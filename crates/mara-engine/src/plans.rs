//! Validated semantic candidates. Publication, locks and recovery belong to hosts.
use crate::*;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Constructed only by an owning semantic operation. Inspectable, not forgeable.
#[derive(Debug, Clone)]
pub struct ChangePlan<T> {
    original: SourceSnapshot,
    candidate: SourceSnapshot,
    fingerprint: String,
    code_fingerprint: Option<String>,
    code_index: Option<CodeIndex>,
    code_dependencies: BTreeMap<PathBuf, Option<Vec<u8>>>,
    changes: Vec<SourceChange>,
    result: T,
}
impl<T> ChangePlan<T> {
    pub fn changes(&self) -> &[SourceChange] {
        &self.changes
    }
    pub fn result(&self) -> &T {
        &self.result
    }
    pub fn original(&self) -> &SourceSnapshot {
        &self.original
    }
    pub fn candidate(&self) -> &SourceSnapshot {
        &self.candidate
    }
    pub fn source_fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub fn check_preconditions(
        &self,
        current: &SourceEngine,
        code: Option<&CodeIndex>,
    ) -> Result<(), Error> {
        if current.source_fingerprint() != self.fingerprint
            || current.base_identity() != self.original.base_identity.as_deref()
        {
            return invalid("source inputs changed since semantic planning; retry the operation");
        }
        if let Some(expected) = &self.code_fingerprint {
            let planner =
                current.planner_with_code(code.ok_or_else(|| Error::InvalidMutation {
                    message: "required native code inputs are unavailable".into(),
                })?)?;
            if planner.code_fingerprint.as_ref() != Some(expected)
                || Some(&planner.corpus.code) != self.code_index.as_ref()
            {
                return invalid("code inputs changed since semantic planning; retry the operation");
            }
            for (path, bytes) in &self.code_dependencies {
                if planner.corpus.code.file_only_bytes(path) != *bytes {
                    return invalid(
                        "code dependency changed since semantic planning; retry the operation",
                    );
                }
            }
        } else if code.is_some() {
            return invalid("code capability changed since semantic planning; retry the operation");
        }
        Ok(())
    }
}

/// One immutable explicit source state, optionally enriched by trusted native code inputs.
/// No rule evaluation: mutations retain their owning source-conformance profile.
pub struct ChangePlanner<'a> {
    pub(crate) engine: &'a SourceEngine,
    pub(crate) project: Project,
    pub(crate) schema: Schema,
    pub(crate) corpus: Corpus,
    code_fingerprint: Option<String>,
}
impl SourceEngine {
    /// Capture explicit state. Native host-only filenames require the native constructor
    /// when rebuilding; they are not portable version-1 browser imports.
    pub fn snapshot(&self) -> SourceSnapshot {
        SourceSnapshot {
            format_version: 1,
            files: self
                .files()
                .iter()
                .map(|(path, content)| SourceFile {
                    path: path.clone(),
                    content: content.clone(),
                })
                .collect(),
            selection: self.selection().clone(),
            base_identity: self.base_identity().map(str::to_owned),
        }
    }
    pub fn planner(&self) -> Result<ChangePlanner<'_>, Error> {
        self.make_planner(None)
    }
    /// Native host supplies an already loaded index; no indexers or code-file discovery run here.
    pub fn planner_with_code(&self, code: &CodeIndex) -> Result<ChangePlanner<'_>, Error> {
        self.make_planner(Some(code))
    }
    fn make_planner(&self, code: Option<&CodeIndex>) -> Result<ChangePlanner<'_>, Error> {
        let (project, schema, mut corpus) = self.corpus()?;
        if let Some(code) = code {
            corpus.code = code.clone();
        }
        if self.code_required(&project, &schema, &corpus) && corpus.code.inputs.is_none() {
            return invalid(
                "required code information is unavailable; cannot validate mutation source conformance",
            );
        }
        let code_fingerprint = if corpus.code.inputs.is_some() {
            Some(
                crate::query::page::fingerprint(&corpus, &schema, &"semantic-plan-inputs-v1")
                    .map_err(|error| Error::InvalidMutation {
                        message: error.to_string(),
                    })?,
            )
        } else {
            None
        };
        Ok(ChangePlanner {
            engine: self,
            project,
            schema,
            corpus,
            code_fingerprint,
        })
    }
    /// Atomically accept a document-only candidate in memory. Never publishes filesystem data.
    pub fn apply_plan<T>(&mut self, plan: ChangePlan<T>) -> Result<T, Error> {
        plan.check_preconditions(self, None)?;
        let candidate = self.rebuild(plan.candidate)?;
        *self = candidate;
        Ok(plan.result)
    }
    pub fn plan_initialize(&self, name: &str, template: Template) -> Result<ChangePlan<()>, Error> {
        let files = crate::templates::initial_sources(name, template)?;
        if self.files().contains_key(PROJECT_FILE) {
            return Err(Error::ExistingProject {
                path: PROJECT_FILE.into(),
            });
        }
        let mut candidate = self.snapshot();
        let mut changes = Vec::new();
        for file in files {
            if self.files().contains_key(&file.path) {
                return Err(Error::WouldOverwrite {
                    path: file.path.into(),
                });
            }
            changes.push(SourceChange {
                path: file.path.clone(),
                before: None,
                after: Some(file.content.clone()),
            });
            candidate.files.push(file);
        }
        let engine = self.rebuild(candidate.clone())?;
        engine.strict_project()?;
        Ok(ChangePlan {
            original: self.snapshot(),
            candidate,
            fingerprint: self.source_fingerprint().to_owned(),
            code_fingerprint: None,
            code_index: None,
            code_dependencies: BTreeMap::new(),
            changes,
            result: (),
        })
    }
}
impl ChangePlanner<'_> {
    pub fn plan_create(
        &self,
        request: ItemCreationRequest,
        mid: &str,
    ) -> Result<ChangePlan<ItemCreation>, Error> {
        crate::mutation::create_item(self, request, mid)
    }
    pub fn plan_backfill(&self, mids: &[String]) -> Result<ChangePlan<BackfilledMids>, Error> {
        crate::mutation::backfill_mids(self, mids)
    }
    pub fn plan_update(&self, request: ItemUpdateParams) -> Result<ChangePlan<ItemUpdate>, Error> {
        crate::mutation::update::update_item(self, request)
    }
    pub fn plan_delete(&self, reference: &str) -> Result<ChangePlan<ItemDeletion>, Error> {
        crate::mutation::delete::delete_item(self, reference)
    }
    pub fn plan_move(
        &self,
        reference: &str,
        file: &Path,
        line: Option<usize>,
    ) -> Result<ChangePlan<ItemMove>, Error> {
        crate::mutation::move_item(self, reference, file, line)
    }
    pub fn plan_rename(
        &self,
        reference: &str,
        new_id: &str,
    ) -> Result<ChangePlan<ItemRename>, Error> {
        crate::mutation::rename::rename_item(self, reference, new_id)
    }
    pub fn plan_relation(
        &self,
        params: &RelationParams,
        add: bool,
        occurrence: Option<&str>,
    ) -> Result<ChangePlan<RelationMutationResult>, RelationError> {
        crate::mutation::mutate_semantic_relation(self, params, add, occurrence)
    }
    pub(crate) fn finish<T>(
        &self,
        replacements: BTreeMap<PathBuf, String>,
        result: T,
    ) -> Result<ChangePlan<T>, Error> {
        let mut candidate = self.engine.snapshot();
        let mut changes = Vec::new();
        for (path, after) in replacements {
            let path = self.engine.normalize_path(path.to_str().ok_or_else(|| {
                Error::InvalidMutation {
                    message: "source path must be UTF-8".into(),
                }
            })?)?;
            let before = self.engine.files().get(&path).cloned();
            if before.as_ref() == Some(&after) {
                continue;
            }
            if !self.engine.document_is_selected(&self.project, &path)? {
                return invalid(format!(
                    "destination file '{path}' is excluded by project content discovery"
                ));
            }
            if let Some(file) = candidate.files.iter_mut().find(|file| file.path == path) {
                file.content = after.clone();
            } else {
                candidate.files.push(SourceFile {
                    path: path.clone(),
                    content: after.clone(),
                });
                candidate.selection.document_paths.push(path.clone());
            }
            changes.push(SourceChange {
                path,
                before,
                after: Some(after),
            });
        }
        let rebuilt = self.engine.rebuild(candidate.clone())?;
        let (_, _, mut corpus) = rebuilt.corpus()?;
        corpus.code = self.corpus.code.clone();
        if rebuilt.code_required(&self.project, &self.schema, &corpus)
            && corpus.code.inputs.is_none()
        {
            return invalid(
                "required code information is unavailable; cannot validate mutation source conformance",
            );
        }
        let code_dependencies = if self.code_fingerprint.is_some() {
            let original_paths = self.corpus.file_only_code_paths();
            corpus
                .file_only_code_paths()
                .into_iter()
                .map(|path| {
                    let bytes = self.corpus.code.file_only_bytes(&path);
                    if bytes.is_none() && !original_paths.contains(&path) {
                        return invalid("candidate-introduced code dependency is unavailable");
                    }
                    Ok((path, bytes))
                })
                .collect::<Result<_, Error>>()?
        } else {
            BTreeMap::new()
        };
        Ok(ChangePlan {
            original: self.engine.snapshot(),
            candidate,
            fingerprint: self.engine.source_fingerprint().to_owned(),
            code_fingerprint: self.code_fingerprint.clone(),
            code_index: self
                .code_fingerprint
                .as_ref()
                .map(|_| self.corpus.code.clone()),
            code_dependencies,
            changes,
            result,
        })
    }
}
fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidMutation {
        message: message.into(),
    })
}
