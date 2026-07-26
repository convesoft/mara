use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

use mara_core::{Mid, SourceDocument, SourceSpan, SourceText};
use mara_markdown::parse_document;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    SemanticCompilation,
    validation::{ValidationResult, check_project, validate_documents},
};

const TRANSACTION_FORMAT: &str = "mara.transaction";
const TRANSACTION_VERSION: u32 = 1;
const TRANSACTION_DIRECTORY: &str = ".mara/transactions";

/// Provenance carried by a candidate source edit.
///
/// Only `Authored` occurrences cross the transaction write boundary. The other
/// variants make rejection explicit for callers that are holding graph or
/// projection records alongside source-backed occurrences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditProvenance {
    Authored,
    Inferred,
    Backlink,
    Projection,
}

/// One exact byte replacement requested against a parsed source occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEdit {
    span: SourceSpan,
    expected: Vec<u8>,
    replacement: Vec<u8>,
    provenance: EditProvenance,
}

impl SourceEdit {
    pub fn authored(
        span: SourceSpan,
        expected: impl Into<Vec<u8>>,
        replacement: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            span,
            expected: expected.into(),
            replacement: replacement.into(),
            provenance: EditProvenance::Authored,
        }
    }

    pub fn derived(span: SourceSpan, provenance: EditProvenance) -> Self {
        debug_assert_ne!(provenance, EditProvenance::Authored);
        Self {
            span,
            expected: Vec::new(),
            replacement: Vec::new(),
            provenance,
        }
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub const fn provenance(&self) -> EditProvenance {
        self.provenance
    }
}

/// Fully computed bytes for one destination before any filesystem mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
    path: String,
    original: Vec<u8>,
    replacement: Vec<u8>,
}

impl FilePatch {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn original(&self) -> &[u8] {
        &self.original
    }

    pub fn replacement(&self) -> &[u8] {
        &self.replacement
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    DerivedTarget {
        path: String,
        provenance: EditProvenance,
    },
    MissingSource {
        path: String,
    },
    DuplicateSource {
        path: String,
    },
    InvalidSpan {
        path: String,
    },
    StalePreimage {
        path: String,
        start: u64,
        end: u64,
    },
    OverlappingEdits {
        path: String,
    },
    InvalidRename {
        reason: String,
    },
    ProjectInvalid {
        reason: String,
    },
    DirtyWorktree,
    UnsafePath {
        path: String,
        reason: String,
    },
    Io {
        path: String,
        message: String,
    },
    JournalConflict {
        transaction: String,
        reason: String,
    },
    IncompleteTransaction {
        transaction: String,
    },
    Interrupted {
        point: FaultPoint,
    },
    RollbackFailed {
        cause: String,
        rollback: String,
    },
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DerivedTarget { path, provenance } => {
                write!(formatter, "cannot edit {provenance:?} occurrence in {path}")
            }
            Self::MissingSource { path } => write!(formatter, "source {path} is unavailable"),
            Self::DuplicateSource { path } => {
                write!(formatter, "source {path} was supplied more than once")
            }
            Self::InvalidSpan { path } => write!(formatter, "source span is invalid for {path}"),
            Self::StalePreimage { path, start, end } => {
                write!(
                    formatter,
                    "source preimage changed at {path}:{start}..{end}"
                )
            }
            Self::OverlappingEdits { path } => {
                write!(formatter, "source edits overlap in {path}")
            }
            Self::InvalidRename { reason } => {
                write!(formatter, "invalid display-ID rename: {reason}")
            }
            Self::ProjectInvalid { reason } => {
                write!(formatter, "project validation failed: {reason}")
            }
            Self::DirtyWorktree => write!(formatter, "Git worktree is not clean"),
            Self::UnsafePath { path, reason } => write!(formatter, "unsafe path {path}: {reason}"),
            Self::Io { path, message } => write!(formatter, "I/O failed for {path}: {message}"),
            Self::JournalConflict {
                transaction,
                reason,
            } => {
                write!(
                    formatter,
                    "transaction {transaction} is conflicted: {reason}"
                )
            }
            Self::IncompleteTransaction { transaction } => {
                write!(
                    formatter,
                    "transaction {transaction} requires explicit recovery"
                )
            }
            Self::Interrupted { point } => {
                write!(formatter, "transaction interrupted at {point:?}")
            }
            Self::RollbackFailed { cause, rollback } => {
                write!(
                    formatter,
                    "transaction failed ({cause}) and rollback failed ({rollback})"
                )
            }
        }
    }
}

impl Error for TransactionError {}

/// Builds complete per-file postimages while proving every occurrence against
/// the immutable source bytes retained by the parser.
pub fn build_patch_plan(
    documents: &[SourceDocument],
    edits: &[SourceEdit],
) -> Result<Vec<FilePatch>, TransactionError> {
    let mut sources = BTreeMap::new();
    for document in documents {
        let path = document.path().to_owned();
        if sources.insert(path.clone(), document).is_some() {
            return Err(TransactionError::DuplicateSource { path });
        }
    }

    let mut by_path = BTreeMap::<String, Vec<&SourceEdit>>::new();
    for edit in edits {
        if edit.provenance != EditProvenance::Authored {
            return Err(TransactionError::DerivedTarget {
                path: edit.span.path().to_owned(),
                provenance: edit.provenance,
            });
        }
        by_path
            .entry(edit.span.path().to_owned())
            .or_default()
            .push(edit);
    }

    let mut patches = Vec::with_capacity(by_path.len());
    for (path, mut edits) in by_path {
        let document = sources
            .get(&path)
            .ok_or_else(|| TransactionError::MissingSource { path: path.clone() })?;
        let original = document.source().as_str().as_bytes();
        edits.sort_by_key(|edit| (edit.span.start_byte(), edit.span.end_byte()));

        let mut prior_end = 0_u64;
        for (index, edit) in edits.iter().enumerate() {
            let start = usize::try_from(edit.span.start_byte())
                .map_err(|_| TransactionError::InvalidSpan { path: path.clone() })?;
            let end = usize::try_from(edit.span.end_byte())
                .map_err(|_| TransactionError::InvalidSpan { path: path.clone() })?;
            if end > original.len() || start > end {
                return Err(TransactionError::InvalidSpan { path });
            }
            if index != 0 && edit.span.start_byte() < prior_end {
                return Err(TransactionError::OverlappingEdits { path });
            }
            if original.get(start..end) != Some(edit.expected.as_slice()) {
                return Err(TransactionError::StalePreimage {
                    path,
                    start: edit.span.start_byte(),
                    end: edit.span.end_byte(),
                });
            }
            prior_end = edit.span.end_byte();
        }

        let replacement_capacity = edits.iter().fold(original.len(), |size, edit| {
            size - edit.expected.len() + edit.replacement.len()
        });
        let mut replacement = Vec::with_capacity(replacement_capacity);
        let mut cursor = 0;
        for edit in edits {
            let start = edit.span.start_byte() as usize;
            let end = edit.span.end_byte() as usize;
            replacement.extend_from_slice(&original[cursor..start]);
            replacement.extend_from_slice(&edit.replacement);
            cursor = end;
        }
        replacement.extend_from_slice(&original[cursor..]);
        patches.push(FilePatch {
            path,
            original: original.to_vec(),
            replacement,
        });
    }
    Ok(patches)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenameOptions {
    pub allow_dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameResult {
    transaction_id: String,
    files_changed: Vec<String>,
}

impl RenameResult {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    pub fn files_changed(&self) -> &[String] {
        &self.files_changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryMode {
    Complete,
    Rollback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    JournalNextFlushed,
    JournalReplaced,
    StageFlushed,
    BackupFlushed,
    DestinationReplaced,
    PermissionsApplied,
    DestinationFlushed,
    ParentFlushed,
    FileJournaled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalOperation {
    DisplayIdRename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalPhase {
    Preparing,
    Prepared,
    Applying,
    Applied,
    Verifying,
    Verified,
    RollingBack,
    RolledBack,
    Cleaning,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalOutcome {
    Replacement,
    Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalFileState {
    Declared,
    Staged,
    Pending,
    Applied,
    Restored,
    Cleaned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalFile {
    ordinal: usize,
    path: String,
    file_identity: String,
    original_sha256: String,
    replacement_sha256: String,
    original_size: u64,
    replacement_size: u64,
    readonly: bool,
    unix_mode: Option<u32>,
    stage_path: String,
    stage_identity: Option<String>,
    backup_path: String,
    backup_identity: Option<String>,
    state: JournalFileState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionJournal {
    format: String,
    version: u32,
    id: String,
    operation: JournalOperation,
    phase: JournalPhase,
    outcome: Option<JournalOutcome>,
    allow_dirty: bool,
    source_mid: String,
    old_id: String,
    new_id: String,
    files: Vec<JournalFile>,
}

struct RenamePlan {
    root: PathBuf,
    source_mid: Mid,
    old_id: String,
    new_id: String,
    allow_dirty: bool,
    patches: Vec<FilePatch>,
    canonical_endpoints: Vec<(String, String, String)>,
}

/// Plans, validates, journals, and applies one repository-wide display-ID rename.
/// The resulting source changes remain uncommitted in the caller's worktree.
pub fn rename_display_id(
    start: impl AsRef<Path>,
    old_id: &str,
    new_id: &str,
    options: RenameOptions,
) -> Result<RenameResult, TransactionError> {
    let plan = plan_display_id_rename(start.as_ref(), old_id, new_id, options)?;
    execute_rename(plan, &mut |_| Ok(()))
}

/// Recovers the single incomplete v1 transaction beneath a project root.
pub fn recover_transaction(
    start: impl AsRef<Path>,
    mode: RecoveryMode,
) -> Result<RenameResult, TransactionError> {
    let validation =
        check_project(start.as_ref()).map_err(|error| TransactionError::ProjectInvalid {
            reason: error.to_string(),
        })?;
    let root = validation
        .project()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "loaded project metadata is unavailable".to_owned(),
        })?
        .root
        .clone();
    let transaction =
        find_incomplete_transaction(&root)?.ok_or_else(|| TransactionError::JournalConflict {
            transaction: "none".to_owned(),
            reason: "no incomplete transaction exists".to_owned(),
        })?;
    let mut journal = load_reconciled_journal(&root, &transaction)?;
    let canonical_endpoints = if mode == RecoveryMode::Complete
        && matches!(
            journal.phase,
            JournalPhase::Verified | JournalPhase::Cleaning | JournalPhase::Complete
        ) {
        Vec::new()
    } else {
        original_canonical_endpoints(&validation, &root, &journal)?
    };
    let files_changed = journal.files.iter().map(|file| file.path.clone()).collect();
    match mode {
        RecoveryMode::Complete => complete_recovery(
            &root,
            &transaction,
            &mut journal,
            &canonical_endpoints,
            &mut |_| Ok(()),
        )?,
        RecoveryMode::Rollback => rollback_recovery(
            &root,
            &transaction,
            &mut journal,
            &canonical_endpoints,
            &mut |_| Ok(()),
        )?,
    }
    Ok(RenameResult {
        transaction_id: transaction,
        files_changed,
    })
}

fn plan_display_id_rename(
    start: &Path,
    old_id: &str,
    new_id: &str,
    options: RenameOptions,
) -> Result<RenamePlan, TransactionError> {
    if old_id.is_empty() || new_id.is_empty() || old_id == new_id {
        return Err(TransactionError::InvalidRename {
            reason: "old and new IDs must be distinct non-empty values".to_owned(),
        });
    }
    let validation = check_project(start).map_err(|error| TransactionError::ProjectInvalid {
        reason: error.to_string(),
    })?;
    if !validation.is_valid() {
        return Err(TransactionError::ProjectInvalid {
            reason: "the complete source project is not valid before editing".to_owned(),
        });
    }
    let project = validation
        .project()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "loaded project metadata is unavailable".to_owned(),
        })?;
    if let Some(transaction) = find_incomplete_transaction(&project.root)? {
        return Err(TransactionError::IncompleteTransaction { transaction });
    }
    if project.git.require_clean_worktree_for_writes && !options.allow_dirty {
        require_clean_worktree(&project.root)?;
    }
    let schema = validation
        .schema()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "compiled schema is unavailable".to_owned(),
        })?;
    let semantic = validation
        .semantic()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "compiled semantic model is unavailable".to_owned(),
        })?;
    let mut matches = semantic
        .items()
        .iter()
        .filter(|item| item.display_id().is_some_and(|id| id.value() == old_id));
    let source_item = matches
        .next()
        .ok_or_else(|| TransactionError::InvalidRename {
            reason: format!("display ID {old_id:?} does not resolve to an item"),
        })?;
    if matches.next().is_some() {
        return Err(TransactionError::InvalidRename {
            reason: format!("display ID {old_id:?} is ambiguous"),
        });
    }
    if semantic.items().iter().any(|item| {
        item.display_id()
            .is_some_and(|display_id| display_id.value() == new_id)
    }) {
        return Err(TransactionError::InvalidRename {
            reason: format!("display ID {new_id:?} is already in use"),
        });
    }

    let source_mid = source_item.mid().clone();
    let declaration = source_item
        .display_id()
        .expect("the selected item has a display ID");
    let documents = validation
        .documents()
        .iter()
        .map(|document| document.source().clone())
        .collect::<Vec<_>>();
    let document_map = documents
        .iter()
        .map(|document| (document.path(), document))
        .collect::<BTreeMap<_, _>>();
    let mut edits = vec![SourceEdit::authored(
        declaration.source().clone(),
        old_id.as_bytes(),
        new_id.as_bytes(),
    )];
    let references = semantic
        .items()
        .iter()
        .flat_map(|item| item.resolved_references())
        .chain(semantic.narrative_references().iter());
    for reference in references {
        if reference.target() == &source_mid && reference.authored().target() == old_id {
            let span = exact_reference_target_span(
                reference.authored().target_source(),
                old_id,
                &document_map,
            )?;
            edits.push(SourceEdit::authored(
                span,
                old_id.as_bytes(),
                new_id.as_bytes(),
            ));
        }
    }
    edits.sort_by_key(|edit| {
        (
            edit.span().path().to_owned(),
            edit.span().start_byte(),
            edit.span().end_byte(),
        )
    });
    edits.dedup_by(|left, right| left.span() == right.span());
    let patches = build_patch_plan(&documents, &edits)?;
    validate_postimage(
        schema,
        validation.documents(),
        semantic,
        &patches,
        old_id,
        new_id,
        &source_mid,
        project.validation.warnings_as_errors,
    )?;
    Ok(RenamePlan {
        root: project.root.clone(),
        source_mid,
        old_id: old_id.to_owned(),
        new_id: new_id.to_owned(),
        allow_dirty: options.allow_dirty,
        patches,
        canonical_endpoints: canonical_endpoints(semantic),
    })
}

fn exact_reference_target_span(
    target_source: &SourceSpan,
    target: &str,
    documents: &BTreeMap<&str, &SourceDocument>,
) -> Result<SourceSpan, TransactionError> {
    let document =
        documents
            .get(target_source.path())
            .ok_or_else(|| TransactionError::MissingSource {
                path: target_source.path().to_owned(),
            })?;
    let full_start = target_source.start_byte() as usize;
    let full_end = target_source.end_byte() as usize;
    let bytes = document.source().as_str().as_bytes();
    let full = bytes
        .get(full_start..full_end)
        .ok_or_else(|| TransactionError::InvalidSpan {
            path: target_source.path().to_owned(),
        })?;
    if !full.ends_with(target.as_bytes()) {
        return Err(TransactionError::StalePreimage {
            path: target_source.path().to_owned(),
            start: target_source.start_byte(),
            end: target_source.end_byte(),
        });
    }
    let start = (full_end - target.len()) as u64;
    let end = full_end as u64;
    let (start_line, start_column) =
        document.source_index().coordinates_at(start).map_err(|_| {
            TransactionError::InvalidSpan {
                path: target_source.path().to_owned(),
            }
        })?;
    let (end_line, end_column) =
        document
            .source_index()
            .coordinates_at(end)
            .map_err(|_| TransactionError::InvalidSpan {
                path: target_source.path().to_owned(),
            })?;
    document
        .source_index()
        .try_span(start, end, start_line, start_column, end_line, end_column)
        .map_err(|_| TransactionError::InvalidSpan {
            path: target_source.path().to_owned(),
        })
}

#[allow(clippy::too_many_arguments)]
fn validate_postimage(
    schema: &mara_core::SchemaDocument,
    original_documents: &[mara_markdown::ParsedDocument],
    original_semantic: &SemanticCompilation,
    patches: &[FilePatch],
    old_id: &str,
    new_id: &str,
    source_mid: &Mid,
    warnings_as_errors: bool,
) -> Result<(), TransactionError> {
    let patch_map = patches
        .iter()
        .map(|patch| (patch.path(), patch.replacement()))
        .collect::<BTreeMap<_, _>>();
    let documents = original_documents
        .iter()
        .map(|document| {
            let source = match patch_map.get(document.source().path()) {
                Some(bytes) => String::from_utf8((*bytes).to_vec()).map_err(|_| {
                    TransactionError::ProjectInvalid {
                        reason: format!("postimage for {} is not UTF-8", document.source().path()),
                    }
                })?,
                None => document.source().source().as_str().to_owned(),
            };
            let source = SourceDocument::try_new(document.source().path(), SourceText::new(source))
                .map_err(|error| TransactionError::ProjectInvalid {
                    reason: error.to_string(),
                })?;
            Ok(parse_document(
                source,
                schema.identity().value().mid().value(),
            ))
        })
        .collect::<Result<Vec<_>, TransactionError>>()?;
    let result = validate_documents(schema, &documents, warnings_as_errors);
    validate_rename_result(&result, original_semantic, old_id, new_id, source_mid)
}

fn validate_rename_result(
    result: &ValidationResult,
    original_semantic: &SemanticCompilation,
    old_id: &str,
    new_id: &str,
    source_mid: &Mid,
) -> Result<(), TransactionError> {
    if !result.is_valid() {
        return Err(TransactionError::ProjectInvalid {
            reason: "the complete renamed project is invalid".to_owned(),
        });
    }
    let semantic = result
        .semantic()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "the renamed semantic model is unavailable".to_owned(),
        })?;
    if semantic.items().iter().any(|item| {
        item.display_id()
            .is_some_and(|display_id| display_id.value() == old_id)
    }) {
        return Err(TransactionError::ProjectInvalid {
            reason: "the old display ID still resolves".to_owned(),
        });
    }
    let renamed = semantic.items().iter().filter(|item| {
        item.display_id()
            .is_some_and(|display_id| display_id.value() == new_id)
    });
    if renamed.map(|item| item.mid()).collect::<Vec<_>>() != vec![source_mid] {
        return Err(TransactionError::ProjectInvalid {
            reason: "the new display ID does not resolve uniquely to the original MID".to_owned(),
        });
    }
    if canonical_endpoints(semantic) != canonical_endpoints(original_semantic) {
        return Err(TransactionError::ProjectInvalid {
            reason: "canonical relation endpoints changed during rename".to_owned(),
        });
    }
    Ok(())
}

fn canonical_endpoints(semantic: &SemanticCompilation) -> Vec<(String, String, String)> {
    let mut endpoints = semantic
        .relations()
        .edges()
        .iter()
        .map(|edge| {
            let key = edge.key();
            (
                key.relation().to_owned(),
                key.source().as_str().to_owned(),
                key.target().as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    endpoints.sort();
    endpoints
}

fn execute_rename(
    plan: RenamePlan,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<RenameResult, TransactionError> {
    let transaction_id = generate_transaction_id()?;
    let transaction_root = ensure_transaction_root(&plan.root)?;
    let transaction_dir = transaction_root.join(&transaction_id);
    fs::create_dir(&transaction_dir).map_err(|error| io_error(&transaction_dir, error))?;
    sync_directory(&transaction_root)?;

    let mut journal = match initial_journal(&plan, &transaction_id) {
        Ok(journal) => journal,
        Err(error) => {
            fs::remove_dir(&transaction_dir)
                .map_err(|cleanup| io_error(&transaction_dir, cleanup))?;
            sync_directory(&transaction_root)?;
            return Err(error);
        }
    };
    write_journal(&transaction_dir, &journal, checkpoint)?;
    sync_directory(&transaction_root)?;
    let files_changed = journal.files.iter().map(|file| file.path.clone()).collect();
    let result = run_forward(
        &plan.root,
        &transaction_id,
        &mut journal,
        &plan.patches,
        &plan.canonical_endpoints,
        checkpoint,
    );
    match result {
        Ok(()) => Ok(RenameResult {
            transaction_id,
            files_changed,
        }),
        Err(error @ TransactionError::Interrupted { .. }) => Err(error),
        Err(error) => {
            let cause = error.to_string();
            match rollback_recovery(
                &plan.root,
                &transaction_id,
                &mut journal,
                &plan.canonical_endpoints,
                &mut |_| Ok(()),
            ) {
                Ok(()) => Err(error),
                Err(rollback) => Err(TransactionError::RollbackFailed {
                    cause,
                    rollback: rollback.to_string(),
                }),
            }
        }
    }
}

fn initial_journal(
    plan: &RenamePlan,
    transaction_id: &str,
) -> Result<TransactionJournal, TransactionError> {
    let mut files = Vec::with_capacity(plan.patches.len());
    let mut identities = BTreeSet::new();
    for (ordinal, patch) in plan.patches.iter().enumerate() {
        let destination = safe_existing_destination(&plan.root, patch.path())?;
        let mut file = open_read_no_follow(&destination)?;
        let metadata = file
            .metadata()
            .map_err(|error| io_error(&destination, error))?;
        let identity = file_identity(&metadata)?;
        if !identities.insert(identity.clone()) {
            return Err(TransactionError::UnsafePath {
                path: patch.path().to_owned(),
                reason: "duplicate filesystem identity".to_owned(),
            });
        }
        let bytes = read_all(&mut file, &destination)?;
        if bytes != patch.original {
            return Err(TransactionError::StalePreimage {
                path: patch.path().to_owned(),
                start: 0,
                end: patch.original.len() as u64,
            });
        }
        let parent = Path::new(patch.path())
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let prefix = format!(".mara-{transaction_id}-{ordinal:06}");
        let stage_path = normalized_join(parent, &format!("{prefix}.stage"));
        let backup_path = normalized_join(parent, &format!("{prefix}.backup"));
        ensure_absent(&plan.root.join(&stage_path))?;
        ensure_absent(&plan.root.join(&backup_path))?;
        files.push(JournalFile {
            ordinal,
            path: patch.path().to_owned(),
            file_identity: identity,
            original_sha256: sha256(&patch.original),
            replacement_sha256: sha256(&patch.replacement),
            original_size: patch.original.len() as u64,
            replacement_size: patch.replacement.len() as u64,
            readonly: metadata.permissions().readonly(),
            unix_mode: unix_mode(&metadata),
            stage_path,
            stage_identity: None,
            backup_path,
            backup_identity: None,
            state: JournalFileState::Declared,
        });
    }
    if files.is_empty() {
        return Err(TransactionError::InvalidRename {
            reason: "a display-ID rename must affect at least its declaration".to_owned(),
        });
    }
    Ok(TransactionJournal {
        format: TRANSACTION_FORMAT.to_owned(),
        version: TRANSACTION_VERSION,
        id: transaction_id.to_owned(),
        operation: JournalOperation::DisplayIdRename,
        phase: JournalPhase::Preparing,
        outcome: None,
        allow_dirty: plan.allow_dirty,
        source_mid: plan.source_mid.as_str().to_owned(),
        old_id: plan.old_id.clone(),
        new_id: plan.new_id.clone(),
        files,
    })
}

fn run_forward(
    root: &Path,
    transaction_id: &str,
    journal: &mut TransactionJournal,
    patches: &[FilePatch],
    canonical_endpoints: &[(String, String, String)],
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let transaction_dir = transaction_directory(root, transaction_id);
    for (index, patch) in patches.iter().enumerate().take(journal.files.len()) {
        prepare_file(root, &transaction_dir, journal, index, patch, checkpoint)?;
    }
    journal.phase = JournalPhase::Prepared;
    write_journal(&transaction_dir, journal, checkpoint)?;
    journal.phase = JournalPhase::Applying;
    write_journal(&transaction_dir, journal, checkpoint)?;
    for index in 0..journal.files.len() {
        replace_forward(root, &transaction_dir, journal, index, checkpoint)?;
    }
    journal.phase = JournalPhase::Applied;
    write_journal(&transaction_dir, journal, checkpoint)?;
    journal.phase = JournalPhase::Verifying;
    write_journal(&transaction_dir, journal, checkpoint)?;
    postcheck_from_journal(
        root,
        journal,
        JournalOutcome::Replacement,
        canonical_endpoints,
    )?;
    journal.outcome = Some(JournalOutcome::Replacement);
    journal.phase = JournalPhase::Verified;
    write_journal(&transaction_dir, journal, checkpoint)?;
    journal.phase = JournalPhase::Cleaning;
    write_journal(&transaction_dir, journal, checkpoint)?;
    clean_transaction(root, transaction_id, journal, checkpoint)
}

fn prepare_file(
    root: &Path,
    transaction_dir: &Path,
    journal: &mut TransactionJournal,
    index: usize,
    patch: &FilePatch,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let entry = &journal.files[index];
    verify_destination(root, entry, &entry.original_sha256, &entry.file_identity)?;
    let stage = root.join(&entry.stage_path);
    write_exclusive(&stage, patch.replacement())?;
    sync_directory(stage.parent().expect("stage has a parent"))?;
    checkpoint(FaultPoint::StageFlushed)?;
    journal.files[index].stage_identity = Some(path_identity(&stage)?);
    journal.files[index].state = JournalFileState::Staged;
    write_journal(transaction_dir, journal, checkpoint)?;
    checkpoint(FaultPoint::FileJournaled)?;

    let backup = root.join(&journal.files[index].backup_path);
    write_exclusive(&backup, patch.original())?;
    sync_directory(backup.parent().expect("backup has a parent"))?;
    checkpoint(FaultPoint::BackupFlushed)?;
    journal.files[index].backup_identity = Some(path_identity(&backup)?);
    journal.files[index].state = JournalFileState::Pending;
    write_journal(transaction_dir, journal, checkpoint)?;
    checkpoint(FaultPoint::FileJournaled)
}

fn replace_forward(
    root: &Path,
    transaction_dir: &Path,
    journal: &mut TransactionJournal,
    index: usize,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let entry = &journal.files[index];
    verify_destination(root, entry, &entry.original_sha256, &entry.file_identity)?;
    verify_temporary(
        root,
        &entry.stage_path,
        entry.stage_identity.as_deref(),
        &entry.replacement_sha256,
    )?;
    verify_temporary(
        root,
        &entry.backup_path,
        entry.backup_identity.as_deref(),
        &entry.original_sha256,
    )?;
    let destination = root.join(&entry.path);
    let stage = root.join(&entry.stage_path);
    fs::rename(&stage, &destination).map_err(|error| io_error(&destination, error))?;
    checkpoint(FaultPoint::DestinationReplaced)?;
    finish_destination(
        root,
        &journal.files[index],
        &entry.replacement_sha256,
        entry.stage_identity.as_deref(),
        checkpoint,
    )?;
    journal.files[index].state = JournalFileState::Applied;
    write_journal(transaction_dir, journal, checkpoint)?;
    checkpoint(FaultPoint::FileJournaled)
}

fn finish_destination(
    root: &Path,
    entry: &JournalFile,
    digest: &str,
    identity: Option<&str>,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let destination = root.join(&entry.path);
    let mut file = open_read_write_no_follow(&destination)?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error(&destination, error))?;
    if identity
        .is_some_and(|expected| expected != file_identity(&metadata).as_deref().unwrap_or(""))
    {
        return conflict(
            &entry.path,
            "destination identity changed during replacement",
        );
    }
    let bytes = read_all(&mut file, &destination)?;
    if sha256(&bytes) != digest {
        return conflict(&entry.path, "destination digest changed during replacement");
    }
    apply_permissions(&file, entry)?;
    checkpoint(FaultPoint::PermissionsApplied)?;
    file.sync_all()
        .map_err(|error| io_error(&destination, error))?;
    checkpoint(FaultPoint::DestinationFlushed)?;
    sync_directory(destination.parent().expect("destination has a parent"))?;
    checkpoint(FaultPoint::ParentFlushed)
}

fn complete_recovery(
    root: &Path,
    transaction_id: &str,
    journal: &mut TransactionJournal,
    canonical_endpoints: &[(String, String, String)],
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    if matches!(
        journal.phase,
        JournalPhase::RollingBack | JournalPhase::RolledBack
    ) {
        return conflict(
            transaction_id,
            "completion is forbidden after rollback begins",
        );
    }
    let transaction_dir = transaction_directory(root, transaction_id);
    if journal.phase == JournalPhase::Preparing {
        for entry in &journal.files {
            verify_destination(root, entry, &entry.original_sha256, &entry.file_identity)?;
            verify_temporary(
                root,
                &entry.stage_path,
                entry.stage_identity.as_deref(),
                &entry.replacement_sha256,
            )?;
            verify_temporary(
                root,
                &entry.backup_path,
                entry.backup_identity.as_deref(),
                &entry.original_sha256,
            )?;
        }
        for index in 0..journal.files.len() {
            adopt_prepared_file(root, &transaction_dir, journal, index, checkpoint)?;
        }
        journal.phase = JournalPhase::Prepared;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    if journal.phase == JournalPhase::Prepared {
        journal.phase = JournalPhase::Applying;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    if journal.phase == JournalPhase::Applying {
        for index in 0..journal.files.len() {
            reconcile_or_apply_file(root, &transaction_dir, journal, index, checkpoint)?;
        }
        journal.phase = JournalPhase::Applied;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    if journal.phase == JournalPhase::Applied {
        journal.phase = JournalPhase::Verifying;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    if journal.phase == JournalPhase::Verifying {
        postcheck_from_journal(
            root,
            journal,
            JournalOutcome::Replacement,
            canonical_endpoints,
        )?;
        journal.outcome = Some(JournalOutcome::Replacement);
        journal.phase = JournalPhase::Verified;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    if journal.phase == JournalPhase::Verified {
        journal.phase = JournalPhase::Cleaning;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    if journal.phase == JournalPhase::Cleaning {
        return clean_transaction(root, transaction_id, journal, checkpoint);
    }
    if journal.phase == JournalPhase::Complete {
        return remove_complete_transaction(root, transaction_id);
    }
    conflict(transaction_id, "journal phase cannot be completed")
}

fn adopt_prepared_file(
    root: &Path,
    transaction_dir: &Path,
    journal: &mut TransactionJournal,
    index: usize,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let entry = journal.files[index].clone();
    verify_destination(root, &entry, &entry.original_sha256, &entry.file_identity)?;
    match entry.state {
        JournalFileState::Declared => {
            verify_temporary(root, &entry.stage_path, None, &entry.replacement_sha256)?;
            journal.files[index].stage_identity =
                Some(path_identity(&root.join(&entry.stage_path))?);
            journal.files[index].state = JournalFileState::Staged;
            write_journal(transaction_dir, journal, checkpoint)?;
        }
        JournalFileState::Staged | JournalFileState::Pending => {}
        _ => return conflict(&entry.path, "illegal file state while preparing"),
    }
    if journal.files[index].state == JournalFileState::Staged {
        verify_temporary(root, &entry.backup_path, None, &entry.original_sha256)?;
        journal.files[index].backup_identity = Some(path_identity(&root.join(&entry.backup_path))?);
        journal.files[index].state = JournalFileState::Pending;
        write_journal(transaction_dir, journal, checkpoint)?;
    }
    verify_temporary(
        root,
        &entry.stage_path,
        journal.files[index].stage_identity.as_deref(),
        &entry.replacement_sha256,
    )?;
    verify_temporary(
        root,
        &entry.backup_path,
        journal.files[index].backup_identity.as_deref(),
        &entry.original_sha256,
    )
}

fn reconcile_or_apply_file(
    root: &Path,
    transaction_dir: &Path,
    journal: &mut TransactionJournal,
    index: usize,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let entry = journal.files[index].clone();
    if entry.state == JournalFileState::Applied {
        verify_destination(
            root,
            &entry,
            &entry.replacement_sha256,
            entry
                .stage_identity
                .as_deref()
                .ok_or_else(|| TransactionError::JournalConflict {
                    transaction: journal.id.clone(),
                    reason: "applied file has no stage identity".to_owned(),
                })?,
        )?;
        let backup_identity =
            entry
                .backup_identity
                .as_deref()
                .ok_or_else(|| TransactionError::JournalConflict {
                    transaction: journal.id.clone(),
                    reason: "applied file has no backup identity".to_owned(),
                })?;
        return verify_temporary(
            root,
            &entry.backup_path,
            Some(backup_identity),
            &entry.original_sha256,
        );
    }
    if entry.state != JournalFileState::Pending {
        return conflict(&entry.path, "only pending files may be applied");
    }
    let destination = root.join(&entry.path);
    let (identity, digest) = identity_and_digest(&destination)?;
    if digest == entry.original_sha256 && identity == entry.file_identity {
        return replace_forward(root, transaction_dir, journal, index, checkpoint);
    }
    if digest == entry.replacement_sha256
        && entry.stage_identity.as_deref() == Some(identity.as_str())
        && !path_exists(&root.join(&entry.stage_path))?
    {
        finish_destination(
            root,
            &entry,
            &entry.replacement_sha256,
            entry.stage_identity.as_deref(),
            checkpoint,
        )?;
        journal.files[index].state = JournalFileState::Applied;
        write_journal(transaction_dir, journal, checkpoint)?;
        return checkpoint(FaultPoint::FileJournaled);
    }
    conflict(
        &entry.path,
        "pending file does not match a recoverable crash window",
    )
}

fn rollback_recovery(
    root: &Path,
    transaction_id: &str,
    journal: &mut TransactionJournal,
    canonical_endpoints: &[(String, String, String)],
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    if matches!(
        journal.phase,
        JournalPhase::Cleaning | JournalPhase::Complete
    ) {
        return conflict(transaction_id, "rollback is forbidden after cleanup begins");
    }
    let transaction_dir = transaction_directory(root, transaction_id);
    if journal.phase != JournalPhase::RollingBack {
        journal.phase = JournalPhase::RollingBack;
        journal.outcome = None;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    for index in (0..journal.files.len()).rev() {
        restore_file(root, &transaction_dir, journal, index, checkpoint)?;
    }
    postcheck_from_journal(root, journal, JournalOutcome::Original, canonical_endpoints)?;
    journal.outcome = Some(JournalOutcome::Original);
    journal.phase = JournalPhase::RolledBack;
    write_journal(&transaction_dir, journal, checkpoint)?;
    journal.phase = JournalPhase::Cleaning;
    write_journal(&transaction_dir, journal, checkpoint)?;
    clean_transaction(root, transaction_id, journal, checkpoint)
}

fn restore_file(
    root: &Path,
    transaction_dir: &Path,
    journal: &mut TransactionJournal,
    index: usize,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let entry = journal.files[index].clone();
    if entry.state == JournalFileState::Restored {
        let expected_identity = entry
            .backup_identity
            .as_deref()
            .unwrap_or(&entry.file_identity);
        return verify_destination(root, &entry, &entry.original_sha256, expected_identity);
    }
    let destination = root.join(&entry.path);
    let (identity, digest) = identity_and_digest(&destination)?;
    if digest == entry.replacement_sha256 {
        let backup_identity =
            entry
                .backup_identity
                .as_deref()
                .ok_or_else(|| TransactionError::JournalConflict {
                    transaction: journal.id.clone(),
                    reason: format!("{} has no recorded backup", entry.path),
                })?;
        verify_temporary(
            root,
            &entry.backup_path,
            Some(backup_identity),
            &entry.original_sha256,
        )?;
        fs::rename(root.join(&entry.backup_path), &destination)
            .map_err(|error| io_error(&destination, error))?;
        checkpoint(FaultPoint::DestinationReplaced)?;
        finish_destination(
            root,
            &entry,
            &entry.original_sha256,
            Some(backup_identity),
            checkpoint,
        )?;
    } else if digest == entry.original_sha256
        && (identity == entry.file_identity
            || entry.backup_identity.as_deref() == Some(identity.as_str()))
    {
        finish_destination(
            root,
            &entry,
            &entry.original_sha256,
            Some(&identity),
            checkpoint,
        )?;
    } else {
        return conflict(&entry.path, "destination cannot be restored safely");
    }
    journal.files[index].state = JournalFileState::Restored;
    write_journal(transaction_dir, journal, checkpoint)?;
    checkpoint(FaultPoint::FileJournaled)
}

fn clean_transaction(
    root: &Path,
    transaction_id: &str,
    journal: &mut TransactionJournal,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let transaction_dir = transaction_directory(root, transaction_id);
    let outcome = journal
        .outcome
        .ok_or_else(|| TransactionError::JournalConflict {
            transaction: transaction_id.to_owned(),
            reason: "cleaning requires a recorded outcome".to_owned(),
        })?;
    for index in 0..journal.files.len() {
        let entry = journal.files[index].clone();
        let destination_digest = match outcome {
            JournalOutcome::Replacement => &entry.replacement_sha256,
            JournalOutcome::Original => &entry.original_sha256,
        };
        let (_, digest) = identity_and_digest(&root.join(&entry.path))?;
        if &digest != destination_digest {
            return conflict(&entry.path, "destination digest changed before cleanup");
        }
        remove_temporary_if_present(
            root,
            &entry.stage_path,
            entry.stage_identity.as_deref(),
            &entry.replacement_sha256,
        )?;
        remove_temporary_if_present(
            root,
            &entry.backup_path,
            entry.backup_identity.as_deref(),
            &entry.original_sha256,
        )?;
        sync_directory(
            root.join(&entry.path)
                .parent()
                .expect("destination has a parent"),
        )?;
        journal.files[index].state = JournalFileState::Cleaned;
        write_journal(&transaction_dir, journal, checkpoint)?;
    }
    journal.phase = JournalPhase::Complete;
    write_journal(&transaction_dir, journal, checkpoint)?;
    remove_complete_transaction(root, transaction_id)
}

fn postcheck_from_journal(
    root: &Path,
    journal: &TransactionJournal,
    outcome: JournalOutcome,
    expected_endpoints: &[(String, String, String)],
) -> Result<(), TransactionError> {
    let result = check_project(root).map_err(|error| TransactionError::ProjectInvalid {
        reason: error.to_string(),
    })?;
    if !result.is_valid() {
        return Err(TransactionError::ProjectInvalid {
            reason: "the recovered complete project is invalid".to_owned(),
        });
    }
    let semantic = result
        .semantic()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "the recovered semantic model is unavailable".to_owned(),
        })?;
    let expected_id = match outcome {
        JournalOutcome::Replacement => &journal.new_id,
        JournalOutcome::Original => &journal.old_id,
    };
    let forbidden_id = match outcome {
        JournalOutcome::Replacement => &journal.old_id,
        JournalOutcome::Original => &journal.new_id,
    };
    let matches = semantic
        .items()
        .iter()
        .filter(|item| {
            item.display_id()
                .is_some_and(|id| id.value() == expected_id)
        })
        .map(|item| item.mid().as_str())
        .collect::<Vec<_>>();
    if matches != vec![journal.source_mid.as_str()]
        || semantic.items().iter().any(|item| {
            item.display_id()
                .is_some_and(|display_id| display_id.value() == forbidden_id)
        })
    {
        return Err(TransactionError::ProjectInvalid {
            reason: "rename identity postcheck failed".to_owned(),
        });
    }
    if canonical_endpoints(semantic) != expected_endpoints {
        return Err(TransactionError::ProjectInvalid {
            reason: "canonical relation endpoints changed during recovery".to_owned(),
        });
    }
    Ok(())
}

fn original_canonical_endpoints(
    validation: &ValidationResult,
    root: &Path,
    journal: &TransactionJournal,
) -> Result<Vec<(String, String, String)>, TransactionError> {
    let schema = validation
        .schema()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "compiled schema is unavailable during recovery".to_owned(),
        })?;
    let mut originals = BTreeMap::new();
    for entry in &journal.files {
        let destination = root.join(&entry.path);
        let (identity, digest, destination_bytes) = identity_digest_and_bytes(&destination)?;
        let bytes = if digest == entry.original_sha256
            && (identity == entry.file_identity
                || entry.backup_identity.as_deref() == Some(identity.as_str()))
        {
            destination_bytes
        } else {
            let backup_identity = entry.backup_identity.as_deref().ok_or_else(|| {
                TransactionError::JournalConflict {
                    transaction: journal.id.clone(),
                    reason: format!("{} has no recoverable original", entry.path),
                }
            })?;
            let backup = root.join(&entry.backup_path);
            let (actual_identity, actual_digest, bytes) = identity_digest_and_bytes(&backup)?;
            if actual_identity != backup_identity || actual_digest != entry.original_sha256 {
                return conflict(
                    &entry.backup_path,
                    "temporary identity or digest does not match journal",
                );
            }
            bytes
        };
        originals.insert(entry.path.as_str(), bytes);
    }
    let documents = validation
        .documents()
        .iter()
        .map(|document| {
            let path = document.source().path();
            let source = match originals.remove(path) {
                Some(bytes) => {
                    String::from_utf8(bytes).map_err(|_| TransactionError::ProjectInvalid {
                        reason: format!("original recovery source {path} is not UTF-8"),
                    })?
                }
                None => document.source().source().as_str().to_owned(),
            };
            let source =
                SourceDocument::try_new(path, SourceText::new(source)).map_err(|error| {
                    TransactionError::ProjectInvalid {
                        reason: error.to_string(),
                    }
                })?;
            Ok(parse_document(
                source,
                schema.identity().value().mid().value(),
            ))
        })
        .collect::<Result<Vec<_>, TransactionError>>()?;
    if !originals.is_empty() {
        return Err(TransactionError::ProjectInvalid {
            reason: "journal source is absent from the recovered project".to_owned(),
        });
    }
    let result = validate_documents(schema, &documents, validation.warnings_as_errors());
    if !result.is_valid() {
        return Err(TransactionError::ProjectInvalid {
            reason: "the journal-derived original project is invalid".to_owned(),
        });
    }
    let semantic = result
        .semantic()
        .ok_or_else(|| TransactionError::ProjectInvalid {
            reason: "the original recovery semantic model is unavailable".to_owned(),
        })?;
    Ok(canonical_endpoints(semantic))
}

fn find_incomplete_transaction(root: &Path) -> Result<Option<String>, TransactionError> {
    let transactions = root.join(TRANSACTION_DIRECTORY);
    let metadata = match fs::symlink_metadata(&transactions) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(&transactions, error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(TransactionError::UnsafePath {
            path: TRANSACTION_DIRECTORY.to_owned(),
            reason: "transaction control path must be a real directory".to_owned(),
        });
    }
    let mut incomplete = Vec::new();
    let mut entries = fs::read_dir(&transactions)
        .map_err(|error| io_error(&transactions, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(&transactions, error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !valid_transaction_id(&name) {
            return conflict(&name, "invalid transaction directory name");
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|error| io_error(&entry.path(), error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return conflict(&name, "transaction entry must be a real directory");
        }
        let mut files = fs::read_dir(entry.path())
            .map_err(|error| io_error(&entry.path(), error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| io_error(&entry.path(), error))?;
        if files.is_empty() {
            fs::remove_dir(entry.path()).map_err(|error| io_error(&entry.path(), error))?;
            sync_directory(&transactions)?;
            continue;
        }
        files.sort_by_key(std::fs::DirEntry::file_name);
        validate_transaction_directory_entries(&entry.path(), &name, &files)?;
        let journal = load_reconciled_journal(root, &name)?;
        if journal.phase == JournalPhase::Complete {
            remove_complete_transaction(root, &name)?;
        } else {
            incomplete.push(name);
        }
    }
    match incomplete.len() {
        0 => Ok(None),
        1 => Ok(incomplete.pop()),
        _ => Err(TransactionError::JournalConflict {
            transaction: incomplete.join(","),
            reason: "multiple incomplete transactions exist".to_owned(),
        }),
    }
}

fn load_reconciled_journal(
    root: &Path,
    transaction_id: &str,
) -> Result<TransactionJournal, TransactionError> {
    let directory = transaction_directory(root, transaction_id);
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| io_error(&directory, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(&directory, error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    validate_transaction_directory_entries(&directory, transaction_id, &entries)?;
    let current_path = directory.join("journal.json");
    let next_path = directory.join("journal.next");
    let current_exists = path_exists(&current_path)?;
    let next_exists = path_exists(&next_path)?;
    match (current_exists, next_exists) {
        (true, false) => read_journal(&current_path, transaction_id),
        (true, true) => {
            let current = read_journal(&current_path, transaction_id)?;
            let next = read_journal(&next_path, transaction_id)?;
            if !same_immutable_journal(&current, &next)
                || !(current == next || legal_next_journal(&current, &next))
            {
                return conflict(transaction_id, "journal.next is not one legal next state");
            }
            fs::remove_file(&next_path).map_err(|error| io_error(&next_path, error))?;
            sync_directory(&directory)?;
            Ok(current)
        }
        (false, true) => {
            let next = read_journal(&next_path, transaction_id)?;
            validate_initial_next(root, &next)?;
            fs::rename(&next_path, &current_path)
                .map_err(|error| io_error(&current_path, error))?;
            sync_directory(&directory)?;
            Ok(next)
        }
        (false, false) => conflict(transaction_id, "transaction directory has no journal"),
    }
}

fn validate_transaction_directory_entries(
    directory: &Path,
    transaction_id: &str,
    entries: &[fs::DirEntry],
) -> Result<(), TransactionError> {
    for entry in entries {
        let name =
            entry
                .file_name()
                .into_string()
                .map_err(|_| TransactionError::JournalConflict {
                    transaction: transaction_id.to_owned(),
                    reason: "transaction directory contains a non-UTF-8 entry".to_owned(),
                })?;
        if !matches!(name.as_str(), "journal.json" | "journal.next") {
            return conflict(
                transaction_id,
                "transaction directory contains an unknown entry",
            );
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| io_error(&directory.join(&name), error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return conflict(
                transaction_id,
                "transaction journal entry is not a regular file",
            );
        }
    }
    Ok(())
}

fn validate_initial_next(
    root: &Path,
    journal: &TransactionJournal,
) -> Result<(), TransactionError> {
    if journal.phase != JournalPhase::Preparing
        || journal.outcome.is_some()
        || journal.files.iter().any(|file| {
            file.state != JournalFileState::Declared
                || file.stage_identity.is_some()
                || file.backup_identity.is_some()
        })
    {
        return conflict(&journal.id, "next-only journal is not an initial state");
    }
    for file in &journal.files {
        verify_destination(root, file, &file.original_sha256, &file.file_identity)?;
        if path_exists(&root.join(&file.stage_path))? || path_exists(&root.join(&file.backup_path))?
        {
            return conflict(&journal.id, "next-only journal has transaction temporaries");
        }
    }
    Ok(())
}

fn read_journal(path: &Path, transaction_id: &str) -> Result<TransactionJournal, TransactionError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return conflict(transaction_id, "journal path is not a regular file");
    }
    let bytes = fs::read(path).map_err(|error| io_error(path, error))?;
    let journal: TransactionJournal =
        serde_json::from_slice(&bytes).map_err(|error| TransactionError::JournalConflict {
            transaction: transaction_id.to_owned(),
            reason: format!("malformed journal: {error}"),
        })?;
    validate_journal(&journal, transaction_id)?;
    Ok(journal)
}

fn validate_journal(
    journal: &TransactionJournal,
    transaction_id: &str,
) -> Result<(), TransactionError> {
    if journal.format != TRANSACTION_FORMAT
        || journal.version != TRANSACTION_VERSION
        || journal.id != transaction_id
        || journal.files.is_empty()
        || !valid_transaction_id(&journal.id)
    {
        return conflict(transaction_id, "journal identity or format is invalid");
    }
    let mut paths = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut prior_path: Option<&str> = None;
    for (ordinal, file) in journal.files.iter().enumerate() {
        if file.ordinal != ordinal
            || !valid_relative_path(&file.path)
            || !valid_digest(&file.original_sha256)
            || !valid_digest(&file.replacement_sha256)
            || file.file_identity.is_empty()
        {
            return conflict(transaction_id, "journal file entry is invalid");
        }
        if prior_path.is_some_and(|prior| prior.as_bytes() >= file.path.as_bytes())
            || !paths.insert(file.path.clone())
            || !identities.insert(file.file_identity.clone())
        {
            return conflict(
                transaction_id,
                "journal paths and identities must be unique and sorted",
            );
        }
        let parent = Path::new(&file.path)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let prefix = format!(".mara-{}-{:06}", journal.id, file.ordinal);
        if file.stage_path != normalized_join(parent, &format!("{prefix}.stage"))
            || file.backup_path != normalized_join(parent, &format!("{prefix}.backup"))
        {
            return conflict(transaction_id, "journal temporary path is not canonical");
        }
        prior_path = Some(&file.path);
    }
    if !valid_journal_state(journal) {
        return conflict(
            transaction_id,
            "journal phase, outcome, and file states are inconsistent",
        );
    }
    Ok(())
}

fn valid_journal_state(journal: &TransactionJournal) -> bool {
    if journal.files.iter().any(|file| match file.state {
        JournalFileState::Declared => {
            file.stage_identity.is_some() || file.backup_identity.is_some()
        }
        JournalFileState::Staged => file.stage_identity.is_none() || file.backup_identity.is_some(),
        JournalFileState::Pending | JournalFileState::Applied => {
            file.stage_identity.is_none() || file.backup_identity.is_none()
        }
        JournalFileState::Restored | JournalFileState::Cleaned => {
            file.backup_identity.is_some() && file.stage_identity.is_none()
        }
    }) {
        return false;
    }

    let files = journal.files.as_slice();
    match (journal.phase, journal.outcome) {
        (JournalPhase::Preparing, None) => preparing_file_states(files),
        (JournalPhase::Prepared, None) => all_files_are(files, JournalFileState::Pending),
        (JournalPhase::Applying, None) => applying_file_states(files),
        (JournalPhase::Applied | JournalPhase::Verifying, None) => {
            all_files_are(files, JournalFileState::Applied)
        }
        (JournalPhase::Verified, Some(JournalOutcome::Replacement)) => {
            all_files_are(files, JournalFileState::Applied)
        }
        (JournalPhase::RollingBack, None) => rolling_back_file_states(files),
        (JournalPhase::RolledBack, Some(JournalOutcome::Original)) => {
            all_files_are(files, JournalFileState::Restored)
        }
        (JournalPhase::Cleaning, Some(outcome)) => cleaning_file_states(files, outcome),
        (JournalPhase::Complete, Some(_)) => all_files_are(files, JournalFileState::Cleaned),
        _ => false,
    }
}

fn all_files_are(files: &[JournalFile], state: JournalFileState) -> bool {
    files.iter().all(|file| file.state == state)
}

fn preparing_file_states(files: &[JournalFile]) -> bool {
    let mut staged = false;
    let mut declared = false;
    files.iter().all(|file| match file.state {
        JournalFileState::Pending if !staged && !declared => true,
        JournalFileState::Staged if !staged && !declared => {
            staged = true;
            true
        }
        JournalFileState::Declared => {
            declared = true;
            true
        }
        _ => false,
    })
}

fn applying_file_states(files: &[JournalFile]) -> bool {
    let mut pending = false;
    files.iter().all(|file| match file.state {
        JournalFileState::Applied if !pending => true,
        JournalFileState::Pending => {
            pending = true;
            true
        }
        _ => false,
    })
}

fn rolling_back_file_states(files: &[JournalFile]) -> bool {
    let restored_start = files
        .iter()
        .position(|file| file.state == JournalFileState::Restored)
        .unwrap_or(files.len());
    files[restored_start..]
        .iter()
        .all(|file| file.state == JournalFileState::Restored)
        && (preparing_file_states(&files[..restored_start])
            || applying_file_states(&files[..restored_start]))
}

fn cleaning_file_states(files: &[JournalFile], outcome: JournalOutcome) -> bool {
    let remaining = match outcome {
        JournalOutcome::Replacement => JournalFileState::Applied,
        JournalOutcome::Original => JournalFileState::Restored,
    };
    let mut remaining_seen = false;
    files.iter().all(|file| match file.state {
        JournalFileState::Cleaned if !remaining_seen => true,
        state if state == remaining => {
            remaining_seen = true;
            true
        }
        _ => false,
    })
}

fn same_immutable_journal(left: &TransactionJournal, right: &TransactionJournal) -> bool {
    left.format == right.format
        && left.version == right.version
        && left.id == right.id
        && left.operation == right.operation
        && left.allow_dirty == right.allow_dirty
        && left.source_mid == right.source_mid
        && left.old_id == right.old_id
        && left.new_id == right.new_id
        && left.files.len() == right.files.len()
        && left.files.iter().zip(&right.files).all(|(left, right)| {
            left.ordinal == right.ordinal
                && left.path == right.path
                && left.file_identity == right.file_identity
                && left.original_sha256 == right.original_sha256
                && left.replacement_sha256 == right.replacement_sha256
                && left.original_size == right.original_size
                && left.replacement_size == right.replacement_size
                && left.readonly == right.readonly
                && left.unix_mode == right.unix_mode
                && left.stage_path == right.stage_path
                && left.backup_path == right.backup_path
        })
}

fn legal_next_journal(current: &TransactionJournal, next: &TransactionJournal) -> bool {
    let legal_outcome = current.outcome == next.outcome
        || matches!(
            (current.phase, current.outcome, next.phase, next.outcome),
            (
                JournalPhase::Verifying,
                None,
                JournalPhase::Verified,
                Some(JournalOutcome::Replacement)
            ) | (
                JournalPhase::RollingBack,
                None,
                JournalPhase::RolledBack,
                Some(JournalOutcome::Original)
            ) | (
                JournalPhase::Verified,
                Some(JournalOutcome::Replacement),
                JournalPhase::RollingBack,
                None
            )
        );
    if !legal_outcome {
        return false;
    }
    let file_changes = current
        .files
        .iter()
        .zip(&next.files)
        .filter(|(left, right)| left != right)
        .count();
    let phase_changed = current.phase != next.phase || current.outcome != next.outcome;
    if phase_changed {
        file_changes == 0 && legal_phase_transition(current.phase, next.phase)
    } else {
        file_changes == 1
            && current
                .files
                .iter()
                .zip(&next.files)
                .all(|(left, right)| left == right || legal_file_transition(left, right))
    }
}

fn legal_phase_transition(current: JournalPhase, next: JournalPhase) -> bool {
    matches!(
        (current, next),
        (JournalPhase::Preparing, JournalPhase::Prepared)
            | (JournalPhase::Prepared, JournalPhase::Applying)
            | (JournalPhase::Applying, JournalPhase::Applied)
            | (JournalPhase::Applied, JournalPhase::Verifying)
            | (JournalPhase::Verifying, JournalPhase::Verified)
            | (JournalPhase::Verified, JournalPhase::Cleaning)
            | (JournalPhase::Preparing, JournalPhase::RollingBack)
            | (JournalPhase::Prepared, JournalPhase::RollingBack)
            | (JournalPhase::Applying, JournalPhase::RollingBack)
            | (JournalPhase::Applied, JournalPhase::RollingBack)
            | (JournalPhase::Verifying, JournalPhase::RollingBack)
            | (JournalPhase::Verified, JournalPhase::RollingBack)
            | (JournalPhase::RollingBack, JournalPhase::RolledBack)
            | (JournalPhase::RolledBack, JournalPhase::Cleaning)
            | (JournalPhase::Cleaning, JournalPhase::Complete)
    )
}

fn legal_file_transition(current: &JournalFile, next: &JournalFile) -> bool {
    matches!(
        (current.state, next.state),
        (JournalFileState::Declared, JournalFileState::Staged)
            | (JournalFileState::Staged, JournalFileState::Pending)
            | (JournalFileState::Pending, JournalFileState::Applied)
            | (JournalFileState::Declared, JournalFileState::Restored)
            | (JournalFileState::Staged, JournalFileState::Restored)
            | (JournalFileState::Pending, JournalFileState::Restored)
            | (JournalFileState::Applied, JournalFileState::Restored)
            | (_, JournalFileState::Cleaned)
    ) && (current.stage_identity == next.stage_identity
        || current.stage_identity.is_none() && next.stage_identity.is_some())
        && (current.backup_identity == next.backup_identity
            || current.backup_identity.is_none() && next.backup_identity.is_some())
}

fn write_journal(
    transaction_dir: &Path,
    journal: &TransactionJournal,
    checkpoint: &mut dyn FnMut(FaultPoint) -> Result<(), TransactionError>,
) -> Result<(), TransactionError> {
    let next = transaction_dir.join("journal.next");
    ensure_regular_or_absent(&next)?;
    let mut bytes =
        serde_json::to_vec_pretty(journal).map_err(|error| TransactionError::JournalConflict {
            transaction: journal.id.clone(),
            reason: format!("journal serialization failed: {error}"),
        })?;
    bytes.push(b'\n');
    let mut file = open_journal_next(&next)?;
    file.write_all(&bytes)
        .map_err(|error| io_error(&next, error))?;
    file.sync_all().map_err(|error| io_error(&next, error))?;
    checkpoint(FaultPoint::JournalNextFlushed)?;
    let current = transaction_dir.join("journal.json");
    fs::rename(&next, &current).map_err(|error| io_error(&current, error))?;
    checkpoint(FaultPoint::JournalReplaced)?;
    sync_directory(transaction_dir)
}

fn require_clean_worktree(root: &Path) -> Result<(), TransactionError> {
    let inside = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map_err(|error| io_error(root, error))?;
    if !inside.status.success() {
        return Ok(());
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .map_err(|error| io_error(root, error))?;
    if !status.status.success() {
        return Err(TransactionError::ProjectInvalid {
            reason: "Git worktree status could not be determined".to_owned(),
        });
    }
    if status.stdout.is_empty() {
        Ok(())
    } else {
        Err(TransactionError::DirtyWorktree)
    }
}

fn generate_transaction_id() -> Result<String, TransactionError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| TransactionError::ProjectInvalid {
        reason: format!("transaction ID generation failed: {error}"),
    })?;
    Ok(format!("tx_{}", ulid::Ulid::from_bytes(bytes)))
}

fn valid_transaction_id(value: &str) -> bool {
    value
        .strip_prefix("tx_")
        .and_then(|value| ulid::Ulid::from_string(value).ok())
        .is_some()
        && value.bytes().skip(3).all(|byte| !byte.is_ascii_lowercase())
}

fn ensure_transaction_root(root: &Path) -> Result<PathBuf, TransactionError> {
    let mara = root.join(".mara");
    let transactions = root.join(TRANSACTION_DIRECTORY);
    match fs::symlink_metadata(&transactions) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(TransactionError::UnsafePath {
                path: TRANSACTION_DIRECTORY.to_owned(),
                reason: "control path is not a real directory".to_owned(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&transactions).map_err(|error| io_error(&transactions, error))?;
            sync_directory(&mara)?;
        }
        Err(error) => return Err(io_error(&transactions, error)),
    }
    Ok(transactions)
}

fn safe_existing_destination(root: &Path, relative: &str) -> Result<PathBuf, TransactionError> {
    if !valid_relative_path(relative) {
        return Err(TransactionError::UnsafePath {
            path: relative.to_owned(),
            reason: "path is not normalized and project-relative".to_owned(),
        });
    }
    let destination = root.join(relative);
    let metadata =
        fs::symlink_metadata(&destination).map_err(|error| io_error(&destination, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(TransactionError::UnsafePath {
            path: relative.to_owned(),
            reason: "destination must be a non-link regular file".to_owned(),
        });
    }
    let canonical = destination
        .canonicalize()
        .map_err(|error| io_error(&destination, error))?;
    if !canonical.starts_with(root) {
        return Err(TransactionError::UnsafePath {
            path: relative.to_owned(),
            reason: "destination resolves outside the project root".to_owned(),
        });
    }
    Ok(destination)
}

fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && !path.contains('\\')
}

fn normalized_join(parent: &Path, name: &str) -> String {
    let path = parent.join(name);
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn transaction_directory(root: &Path, transaction_id: &str) -> PathBuf {
    root.join(TRANSACTION_DIRECTORY).join(transaction_id)
}

fn remove_complete_transaction(root: &Path, transaction_id: &str) -> Result<(), TransactionError> {
    let directory = transaction_directory(root, transaction_id);
    if path_exists(&directory.join("journal.next"))? {
        return conflict(transaction_id, "complete transaction retains journal.next");
    }
    fs::remove_file(directory.join("journal.json"))
        .map_err(|error| io_error(&directory.join("journal.json"), error))?;
    fs::remove_dir(&directory).map_err(|error| io_error(&directory, error))?;
    sync_directory(&root.join(TRANSACTION_DIRECTORY))
}

fn verify_destination(
    root: &Path,
    entry: &JournalFile,
    digest: &str,
    identity: &str,
) -> Result<(), TransactionError> {
    let destination = safe_existing_destination(root, &entry.path)?;
    let (actual_identity, actual_digest) = identity_and_digest(&destination)?;
    if actual_identity != identity || actual_digest != digest {
        return conflict(
            &entry.path,
            "destination identity or digest does not match journal",
        );
    }
    Ok(())
}

fn verify_temporary(
    root: &Path,
    relative: &str,
    identity: Option<&str>,
    digest: &str,
) -> Result<(), TransactionError> {
    if !valid_relative_path(relative) {
        return Err(TransactionError::UnsafePath {
            path: relative.to_owned(),
            reason: "temporary path is not normalized".to_owned(),
        });
    }
    let path = root.join(relative);
    let (actual_identity, actual_digest) = identity_and_digest(&path)?;
    if identity.is_some_and(|expected| expected != actual_identity) || actual_digest != digest {
        return conflict(
            relative,
            "temporary identity or digest does not match journal",
        );
    }
    Ok(())
}

fn remove_temporary_if_present(
    root: &Path,
    relative: &str,
    identity: Option<&str>,
    digest: &str,
) -> Result<(), TransactionError> {
    let path = root.join(relative);
    if !path_exists(&path)? {
        return Ok(());
    }
    verify_temporary(root, relative, identity, digest)?;
    fs::remove_file(&path).map_err(|error| io_error(&path, error))
}

fn identity_and_digest(path: &Path) -> Result<(String, String), TransactionError> {
    identity_digest_and_bytes(path).map(|(identity, digest, _)| (identity, digest))
}

fn identity_digest_and_bytes(path: &Path) -> Result<(String, String, Vec<u8>), TransactionError> {
    let mut file = open_read_no_follow(path)?;
    let metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !metadata.is_file() {
        return Err(TransactionError::UnsafePath {
            path: path.display().to_string(),
            reason: "path is not a regular file".to_owned(),
        });
    }
    let identity = file_identity(&metadata)?;
    let bytes = read_all(&mut file, path)?;
    let digest = sha256(&bytes);
    Ok((identity, digest, bytes))
}

fn path_identity(path: &Path) -> Result<String, TransactionError> {
    identity_and_digest(path).map(|(identity, _)| identity)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn ensure_absent(path: &Path) -> Result<(), TransactionError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
        Ok(_) => Err(TransactionError::UnsafePath {
            path: path.display().to_string(),
            reason: "transaction temporary already exists".to_owned(),
        }),
    }
}

fn ensure_regular_or_absent(path: &Path) -> Result<(), TransactionError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(TransactionError::UnsafePath {
            path: path.display().to_string(),
            reason: "journal path is not a regular file".to_owned(),
        }),
    }
}

fn path_exists(path: &Path) -> Result<bool, TransactionError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(TransactionError::UnsafePath {
            path: path.display().to_string(),
            reason: "links are forbidden in transaction paths".to_owned(),
        }),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

fn write_exclusive(path: &Path, bytes: &[u8]) -> Result<(), TransactionError> {
    let mut file = open_exclusive(path)?;
    file.write_all(bytes)
        .map_err(|error| io_error(path, error))?;
    file.sync_all().map_err(|error| io_error(path, error))
}

fn read_all(file: &mut File, path: &Path) -> Result<Vec<u8>, TransactionError> {
    use std::io::{Read, Seek};
    file.rewind().map_err(|error| io_error(path, error))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    Ok(bytes)
}

fn sync_directory(path: &Path) -> Result<(), TransactionError> {
    let directory = File::open(path).map_err(|error| io_error(path, error))?;
    directory.sync_all().map_err(|error| io_error(path, error))
}

fn io_error(path: &Path, error: io::Error) -> TransactionError {
    TransactionError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    }
}

fn conflict<T>(transaction: &str, reason: &str) -> Result<T, TransactionError> {
    Err(TransactionError::JournalConflict {
        transaction: transaction.to_owned(),
        reason: reason.to_owned(),
    })
}

#[cfg(unix)]
fn open_read_no_follow(path: &Path) -> Result<File, TransactionError> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| io_error(path, error))
}

#[cfg(unix)]
fn open_read_write_no_follow(path: &Path) -> Result<File, TransactionError> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| io_error(path, error))
}

#[cfg(unix)]
fn open_exclusive(path: &Path) -> Result<File, TransactionError> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| io_error(path, error))
}

#[cfg(unix)]
fn open_journal_next(path: &Path) -> Result<File, TransactionError> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| io_error(path, error))
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> Result<String, TransactionError> {
    use std::os::unix::fs::MetadataExt;
    Ok(format!(
        "device:{};inode:{}",
        metadata.dev(),
        metadata.ino()
    ))
}

#[cfg(unix)]
fn unix_mode(metadata: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.mode() & 0o7777)
}

#[cfg(unix)]
fn apply_permissions(file: &File, entry: &JournalFile) -> Result<(), TransactionError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = entry
        .unix_mode
        .ok_or_else(|| TransactionError::JournalConflict {
            transaction: entry.path.clone(),
            reason: "Unix journal entry has no mode".to_owned(),
        })?;
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| TransactionError::Io {
            path: entry.path.clone(),
            message: error.to_string(),
        })
}

#[cfg(not(unix))]
compile_error!("CON-20 transaction protocol is currently qualified only for Unix targets");

#[cfg(test)]
mod tests {
    use std::fs;

    use mara_core::{SourceDocument, SourceText};
    use tempfile::TempDir;

    use super::*;

    const PROJECT: &str = r#"format_version = 1
[project]
name = "transaction-fixture"
schema = ".mara/schema.yaml"
[content]
include = ["docs/**/*.mara.md"]
exclude = []
respect_gitignore = true
follow_directory_symlinks = false
allow_internal_file_symlinks = false
[index]
path = ".mara/index.json"
[validation]
warnings_as_errors = false
[git]
require_clean_worktree_for_writes = false
"#;

    const SCHEMA: &str = r#"format_version: 1
schema:
  name: transaction-fixture
  version: 1.0.0
identity:
  mid:
    format: ulid
    prefix: m_
flavours:
  alpha:
    label: Alpha
    description: Fixture source.
    guidance:
      use_when: [Testing source items.]
      avoid_when: [Testing target items.]
    id: {}
    title: {}
    body: {}
    fields: {}
  beta:
    label: Beta
    description: Fixture target.
    guidance:
      use_when: [Testing target items.]
      avoid_when: [Testing source items.]
    id: {}
    title: {}
    body: {}
    fields: {}
relations:
  connects:
    source:
      flavours: [alpha]
    target:
      flavours: [beta]
rules: []
"#;

    const TARGET: &str = ":::beta m_00000000000000000000000002\r\n:id:\tBETA-B  \r\n:title: Target\r\n\r\nUnrelated BETA-B prose.\r\n:::\r\n";
    const SOURCE: &str = r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:title: Source
:connects: BETA-B

References [[BETA-B|keep label]] and [[connects:BETA-B|typed label]].
MID [[m_00000000000000000000000002]] and unrelated BETA-B prose.
:::
"#;

    fn project() -> TempDir {
        let temp = tempfile::tempdir().expect("create transaction fixture");
        fs::create_dir_all(temp.path().join(".mara")).unwrap();
        fs::create_dir_all(temp.path().join("docs")).unwrap();
        fs::write(temp.path().join(".mara/project.toml"), PROJECT).unwrap();
        fs::write(temp.path().join(".mara/schema.yaml"), SCHEMA).unwrap();
        fs::write(temp.path().join("docs/target.mara.md"), TARGET).unwrap();
        fs::write(temp.path().join("docs/source.mara.md"), SOURCE).unwrap();
        temp
    }

    fn interrupt_once(
        selected: FaultPoint,
    ) -> impl FnMut(FaultPoint) -> Result<(), TransactionError> {
        let mut interrupted = false;
        move |point| {
            if point == selected && !interrupted {
                interrupted = true;
                Err(TransactionError::Interrupted { point })
            } else {
                Ok(())
            }
        }
    }

    fn only_transaction_dir(fixture: &TempDir) -> PathBuf {
        fs::read_dir(fixture.path().join(TRANSACTION_DIRECTORY))
            .unwrap()
            .next()
            .expect("one transaction")
            .unwrap()
            .path()
    }

    fn write_committed_journal(directory: &Path, journal: &TransactionJournal) {
        let mut bytes = serde_json::to_vec_pretty(journal).unwrap();
        bytes.push(b'\n');
        fs::write(directory.join("journal.json"), bytes).unwrap();
    }

    fn set_applied(journal: &mut TransactionJournal) {
        for file in &mut journal.files {
            file.stage_identity = Some(format!("stage-{}", file.ordinal));
            file.backup_identity = Some(format!("backup-{}", file.ordinal));
            file.state = JournalFileState::Applied;
        }
    }

    fn document(path: &str, source: &str) -> SourceDocument {
        SourceDocument::try_new(path, SourceText::new(source.to_owned())).expect("valid fixture")
    }

    fn edit(document: &SourceDocument, needle: &str, replacement: &str) -> SourceEdit {
        let source = document.source().as_str();
        let start = source.find(needle).expect("fixture occurrence") as u64;
        let end = start + needle.len() as u64;
        let (start_line, start_column) = document
            .source_index()
            .coordinates_at(start)
            .expect("start coordinate");
        let (end_line, end_column) = document
            .source_index()
            .coordinates_at(end)
            .expect("end coordinate");
        SourceEdit::authored(
            document
                .source_index()
                .try_span(start, end, start_line, start_column, end_line, end_column)
                .expect("valid span"),
            needle.as_bytes(),
            replacement.as_bytes(),
        )
    }

    #[test]
    fn patches_only_selected_bytes_across_multiple_files() {
        let first = document("docs/one.mara.md", "before OLD after\r\n");
        let second = document("docs/two.mara.md", "OLD and unrelated OLD prose\n");
        let patches = build_patch_plan(
            &[first.clone(), second.clone()],
            &[
                edit(&first, "OLD", "NEW-ID"),
                edit(&second, "OLD", "NEW-ID"),
            ],
        )
        .expect("valid authored edits");

        assert_eq!(patches[0].replacement(), b"before NEW-ID after\r\n");
        assert_eq!(
            patches[1].replacement(),
            b"NEW-ID and unrelated OLD prose\n"
        );
        assert_eq!(&patches[0].replacement()[..7], &patches[0].original()[..7]);
        assert_eq!(
            &patches[0].replacement()[13..],
            &patches[0].original()[10..]
        );
    }

    #[test]
    fn stale_preimages_fail_before_any_postimage_is_returned() {
        let source = document("docs/item.mara.md", "OLD");
        let mut stale = edit(&source, "OLD", "NEW");
        stale.expected = b"old".to_vec();

        assert!(matches!(
            build_patch_plan(&[source], &[stale]),
            Err(TransactionError::StalePreimage { .. })
        ));
    }

    #[test]
    fn derived_occurrences_are_rejected_at_the_write_boundary() {
        let source = document("docs/item.mara.md", "OLD");
        let authored = edit(&source, "OLD", "NEW");
        let derived = SourceEdit::derived(authored.span().clone(), EditProvenance::Backlink);

        assert_eq!(
            build_patch_plan(&[source], &[derived]),
            Err(TransactionError::DerivedTarget {
                path: "docs/item.mara.md".to_owned(),
                provenance: EditProvenance::Backlink,
            })
        );
    }

    #[test]
    fn display_id_rename_preserves_every_unselected_byte_and_file_mode() {
        let fixture = project();
        let target_path = fixture.path().join("docs/target.mara.md");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target_path, fs::Permissions::from_mode(0o640)).unwrap();
        }

        let result = rename_display_id(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .expect("rename succeeds");

        assert_eq!(
            result.files_changed(),
            &["docs/source.mara.md", "docs/target.mara.md"]
        );
        assert_eq!(
            fs::read(&target_path).unwrap(),
            TARGET.replacen("BETA-B", "BETA-RENAMED", 1).as_bytes()
        );
        assert_eq!(
            fs::read(fixture.path().join("docs/source.mara.md")).unwrap(),
            SOURCE
                .replacen(":connects: BETA-B", ":connects: BETA-RENAMED", 1)
                .replacen("[[BETA-B|", "[[BETA-RENAMED|", 1)
                .replacen("[[connects:BETA-B|", "[[connects:BETA-RENAMED|", 1)
                .as_bytes()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&target_path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
        assert!(
            fs::read_dir(fixture.path().join(TRANSACTION_DIRECTORY))
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn interrupted_replacement_can_complete_from_the_durable_journal() {
        let fixture = project();
        let plan = plan_display_id_rename(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .unwrap();
        let error =
            execute_rename(plan, &mut interrupt_once(FaultPoint::DestinationReplaced)).unwrap_err();
        assert!(matches!(error, TransactionError::Interrupted { .. }));

        recover_transaction(fixture.path(), RecoveryMode::Complete)
            .expect("complete interrupted transaction");
        assert!(
            fs::read_to_string(fixture.path().join("docs/target.mara.md"))
                .unwrap()
                .contains(":id:\tBETA-RENAMED  ")
        );
        assert!(check_project(fixture.path()).unwrap().is_valid());
    }

    #[test]
    fn interrupted_replacement_can_roll_back_to_the_original_tree() {
        let fixture = project();
        let plan = plan_display_id_rename(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .unwrap();
        execute_rename(plan, &mut interrupt_once(FaultPoint::DestinationReplaced)).unwrap_err();

        recover_transaction(fixture.path(), RecoveryMode::Rollback)
            .expect("roll back interrupted transaction");
        assert_eq!(
            fs::read(fixture.path().join("docs/target.mara.md")).unwrap(),
            TARGET.as_bytes()
        );
        assert_eq!(
            fs::read(fixture.path().join("docs/source.mara.md")).unwrap(),
            SOURCE.as_bytes()
        );
        assert!(check_project(fixture.path()).unwrap().is_valid());
    }

    #[test]
    fn initial_journal_is_canonical_and_next_only_recovery_is_safe() {
        let fixture = project();
        let plan = plan_display_id_rename(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .unwrap();
        let error =
            execute_rename(plan, &mut interrupt_once(FaultPoint::JournalNextFlushed)).unwrap_err();
        assert!(matches!(error, TransactionError::Interrupted { .. }));

        let transactions = fs::read_dir(fixture.path().join(TRANSACTION_DIRECTORY))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(transactions.len(), 1);
        let bytes = fs::read(transactions[0].path().join("journal.next")).unwrap();
        assert!(bytes.ends_with(b"\n"));
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["format"], TRANSACTION_FORMAT);
        assert_eq!(value["version"], TRANSACTION_VERSION);
        assert_eq!(value["operation"], "display_id_rename");
        assert_eq!(value["phase"], "preparing");
        assert!(value["files"].as_array().unwrap().iter().all(|file| {
            file["state"] == "declared"
                && file["stage_identity"].is_null()
                && file["backup_identity"].is_null()
        }));

        recover_transaction(fixture.path(), RecoveryMode::Rollback)
            .expect("adopt and roll back initial next-only journal");
        assert_eq!(
            fs::read(fixture.path().join("docs/target.mara.md")).unwrap(),
            TARGET.as_bytes()
        );
    }

    #[test]
    fn disk_changes_after_planning_fail_without_replacing_any_source() {
        let fixture = project();
        let plan = plan_display_id_rename(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .unwrap();
        let source_path = fixture.path().join("docs/source.mara.md");
        let changed = SOURCE.replace("Source", "Changed source");
        fs::write(&source_path, &changed).unwrap();

        assert!(matches!(
            execute_rename(plan, &mut |_| Ok(())),
            Err(TransactionError::StalePreimage { .. })
        ));
        assert_eq!(fs::read(&source_path).unwrap(), changed.as_bytes());
        assert_eq!(
            fs::read(fixture.path().join("docs/target.mara.md")).unwrap(),
            TARGET.as_bytes()
        );
    }

    #[test]
    fn applied_reconciliation_requires_the_recorded_original_backup() {
        for case in ["missing", "mismatched"] {
            let fixture = project();
            let plan = plan_display_id_rename(
                fixture.path(),
                "BETA-B",
                "BETA-RENAMED",
                RenameOptions::default(),
            )
            .unwrap();
            execute_rename(plan, &mut interrupt_once(FaultPoint::DestinationReplaced)).unwrap_err();

            let directory = only_transaction_dir(&fixture);
            let transaction_id = directory.file_name().unwrap().to_str().unwrap().to_owned();
            let mut journal =
                read_journal(&directory.join("journal.json"), &transaction_id).unwrap();
            journal.files[0].state = JournalFileState::Applied;
            let backup = fixture.path().join(&journal.files[0].backup_path);
            write_committed_journal(&directory, &journal);
            if case == "missing" {
                fs::remove_file(backup).unwrap();
            } else {
                fs::write(backup, b"not the original").unwrap();
            }

            assert!(
                reconcile_or_apply_file(fixture.path(), &directory, &mut journal, 0, &mut |_| Ok(
                    ()
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn verified_replacement_may_durably_begin_rollback_with_no_outcome() {
        let fixture = project();
        let plan = plan_display_id_rename(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .unwrap();
        let transaction_id = "tx_01JX0TV1P2V1N0VJ3M3J6W9Y7R";
        let mut current = initial_journal(&plan, transaction_id).unwrap();
        set_applied(&mut current);
        current.phase = JournalPhase::Verified;
        current.outcome = Some(JournalOutcome::Replacement);
        let mut next = current.clone();
        next.phase = JournalPhase::RollingBack;
        next.outcome = None;

        validate_journal(&current, transaction_id).unwrap();
        validate_journal(&next, transaction_id).unwrap();
        assert!(legal_next_journal(&current, &next));
    }

    #[test]
    fn journal_validation_rejects_impossible_phase_outcome_and_file_states() {
        let fixture = project();
        let plan = plan_display_id_rename(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .unwrap();
        let transaction_id = "tx_01JX0TV1P2V1N0VJ3M3J6W9Y7R";
        let initial = initial_journal(&plan, transaction_id).unwrap();

        let mut prepared_with_declared = initial.clone();
        prepared_with_declared.phase = JournalPhase::Prepared;
        let mut preparing_with_outcome = initial.clone();
        preparing_with_outcome.outcome = Some(JournalOutcome::Replacement);
        let mut staged_without_identity = initial.clone();
        staged_without_identity.files[0].state = JournalFileState::Staged;
        let mut preparation_out_of_order = initial.clone();
        preparation_out_of_order.files[1].stage_identity = Some("stage-1".to_owned());
        preparation_out_of_order.files[1].backup_identity = Some("backup-1".to_owned());
        preparation_out_of_order.files[1].state = JournalFileState::Pending;

        for journal in [
            prepared_with_declared,
            preparing_with_outcome,
            staged_without_identity,
            preparation_out_of_order,
        ] {
            assert!(matches!(
                validate_journal(&journal, transaction_id),
                Err(TransactionError::JournalConflict { .. })
            ));
        }
    }

    #[test]
    fn recovery_rejects_every_unrecognized_transaction_directory_entry() {
        for case in ["unknown", "directory", "link"] {
            let fixture = project();
            let plan = plan_display_id_rename(
                fixture.path(),
                "BETA-B",
                "BETA-RENAMED",
                RenameOptions::default(),
            )
            .unwrap();
            execute_rename(plan, &mut interrupt_once(FaultPoint::JournalReplaced)).unwrap_err();
            let directory = only_transaction_dir(&fixture);
            match case {
                "unknown" => fs::write(directory.join("unexpected"), b"unknown").unwrap(),
                "directory" => fs::create_dir(directory.join("journal.next")).unwrap(),
                "link" => {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink("journal.json", directory.join("journal.next"))
                        .unwrap();
                }
                _ => unreachable!(),
            }

            assert!(matches!(
                recover_transaction(fixture.path(), RecoveryMode::Rollback),
                Err(TransactionError::JournalConflict { .. })
            ));
        }

        let fixture = project();
        let empty = fixture
            .path()
            .join(TRANSACTION_DIRECTORY)
            .join("tx_01JX0TV1P2V1N0VJ3M3J6W9Y7R");
        fs::create_dir_all(&empty).unwrap();
        assert_eq!(find_incomplete_transaction(fixture.path()).unwrap(), None);
        assert!(!empty.exists());
    }

    #[test]
    fn recovered_postcheck_preserves_preflight_canonical_relation_endpoints() {
        let fixture = project();
        let second_target = r#"
:::beta m_00000000000000000000000003
:id: BETA-ALTERED
:title: Alternate target

:::
"#;
        fs::write(fixture.path().join("docs/alternate.mara.md"), second_target).unwrap();
        let plan = plan_display_id_rename(
            fixture.path(),
            "BETA-B",
            "BETA-RENAMED",
            RenameOptions::default(),
        )
        .unwrap();
        execute_rename(plan, &mut interrupt_once(FaultPoint::DestinationReplaced)).unwrap_err();

        let directory = only_transaction_dir(&fixture);
        let transaction_id = directory.file_name().unwrap().to_str().unwrap().to_owned();
        let mut journal = read_journal(&directory.join("journal.json"), &transaction_id).unwrap();
        let source_path = fixture.path().join("docs/source.mara.md");
        let changed = fs::read_to_string(&source_path).unwrap().replacen(
            ":connects: BETA-RENAMED",
            ":connects: BETA-ALTERED",
            1,
        );
        fs::write(&source_path, &changed).unwrap();
        let source = journal
            .files
            .iter_mut()
            .find(|file| file.path == "docs/source.mara.md")
            .unwrap();
        source.replacement_sha256 = sha256(changed.as_bytes());
        write_committed_journal(&directory, &journal);

        assert!(matches!(
            recover_transaction(fixture.path(), RecoveryMode::Complete),
            Err(TransactionError::ProjectInvalid { reason })
                if reason.contains("canonical relation endpoints")
        ));
    }
}
