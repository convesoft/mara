use crate::*;
use ignore::gitignore::GitignoreBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Version-1 caller-owned UTF-8 sources. Loading never discovers or fetches files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub format_version: u32,
    pub files: Vec<SourceFile>,
    pub selection: SelectionInputs,
    pub base_identity: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}
/// The producer declares the complete selected inventory and effective ignore context.
/// An expected document without supplied content is an incomplete input, not absence proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionInputs {
    pub document_paths: Vec<String>,
    pub ignores: Vec<IgnoreInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreInput {
    /// Root-relative directory; empty means the logical project root.
    pub directory: String,
    /// For inherited ignores, the relative path from that ignore origin to the root.
    /// Nonempty requires an empty directory; exact components preserve anchored patterns.
    pub root_prefix: Vec<String>,
    pub content: String,
}

/// Normalize portable paths, rejecting escapes, host-dependent separators and drive prefixes.
pub fn normalize_source_path(path: &str) -> Result<String, Error> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.split('/').any(|p| p == "..")
    {
        return Err(input_error(format!("unsafe source path '{path}'")));
    }
    let normalized = path
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect::<Vec<_>>()
        .join("/");
    if normalized.is_empty() {
        return Err(input_error("source path must name a file"));
    }
    if normalized
        .split('/')
        .next()
        .is_some_and(|part| part.contains(':'))
    {
        return Err(input_error(format!("unsafe source path '{path}'")));
    }
    Ok(normalized)
}
fn input_error(message: impl Into<String>) -> Error {
    Error::InvalidProject {
        path: PathBuf::from(PROJECT_FILE),
        message: message.into(),
    }
}

// @mara implements DES-EXPLICIT-SOURCE-ENGINE
#[derive(Debug, Clone)]
pub struct SourceEngine {
    files: BTreeMap<String, String>,
    selection: SelectionInputs,
    fingerprint: String,
    base_identity: Option<String>,
}
/// Outcomes from a separate rule adapter. These are inputs, not engine-generated proof.
/// They bind exact source state and the validation target to prevent stale reuse.
#[derive(Debug, Clone)]
pub struct RuleChecks {
    pub source_fingerprint: String,
    pub definitions: CheckOutcome,
    pub evaluation: Option<(ValidationTarget, CheckOutcome)>,
}
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub evaluation_complete: bool,
    pub diagnostics: Vec<ValidationDiagnostic>,
}
impl SourceEngine {
    pub fn new(snapshot: SourceSnapshot) -> Result<Self, Error> {
        if snapshot.format_version != 1 {
            return Err(input_error(
                "unsupported source snapshot format; expected 1",
            ));
        }
        let mut files = BTreeMap::new();
        for file in snapshot.files {
            let path = normalize_source_path(&file.path)?;
            if files.insert(path.clone(), file.content).is_some() {
                return Err(input_error(format!(
                    "duplicate normalized source path '{path}'"
                )));
            }
        }
        let mut selection = snapshot.selection;
        let mut paths = BTreeSet::new();
        for path in &mut selection.document_paths {
            *path = normalize_source_path(path)?;
            if !paths.insert(path.clone()) {
                return Err(input_error(format!(
                    "duplicate selected document path '{path}'"
                )));
            }
        }
        selection.document_paths.sort();
        for ignore in &mut selection.ignores {
            if !ignore.root_prefix.is_empty() && !ignore.directory.is_empty() {
                return Err(input_error("invalid ignore origin"));
            }
            for component in &ignore.root_prefix {
                if normalize_source_path(component)? != *component || component.contains('/') {
                    return Err(input_error(
                        "ignore root prefix requires ordinary path components",
                    ));
                }
            }
            if !ignore.directory.is_empty() {
                ignore.directory = normalize_source_path(&ignore.directory)?;
            }
            ignore_matcher(ignore)?;
        }
        if let Some(longest) = selection
            .ignores
            .iter()
            .map(|i| &i.root_prefix)
            .max_by_key(|p| p.len())
            && selection
                .ignores
                .iter()
                .any(|i| !longest.ends_with(&i.root_prefix))
        {
            return Err(input_error("inconsistent inherited ignore origins"));
        }
        selection
            .ignores
            .sort_by(|a, b| (&a.root_prefix, &a.directory).cmp(&(&b.root_prefix, &b.directory)));
        if selection.ignores.windows(2).any(|pair| {
            pair[0].root_prefix == pair[1].root_prefix && pair[0].directory == pair[1].directory
        }) {
            return Err(input_error("duplicate normalized ignore origin"));
        }
        let fingerprint = fingerprint_source_inputs(&files, &selection);
        Ok(Self {
            files,
            selection,
            fingerprint,
            base_identity: snapshot.base_identity,
        })
    }
    pub fn source_fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub fn base_identity(&self) -> Option<&str> {
        self.base_identity.as_deref()
    }
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }
    fn project_validation(&self) -> Result<ProjectValidation, Error> {
        let source = self
            .files
            .get(PROJECT_FILE)
            .ok_or_else(|| input_error("missing required source .mara/project.toml"))?;
        parse_project_for_validation(PathBuf::from("."), source, |path| self.file(path).is_some())
    }
    fn file(&self, path: &Path) -> Option<&str> {
        let path = path.to_str()?;
        let normalized = normalize_source_path(path).ok()?;
        self.files.get(&normalized).map(String::as_str)
    }
    fn strict_project(&self) -> Result<(Project, Schema), Error> {
        let (project, errors, available) = self.project_validation()?.into_parts();
        if let Some(error) = errors.first() {
            return Err(input_error(error.message.clone()));
        }
        if !available {
            return Err(input_error("missing required schema source"));
        }
        let (schema, errors) = parse_schema_for_validation(
            self.file(project.schema_path()).expect("available schema"),
        )?;
        if let Some(error) = errors.first() {
            return Err(Error::InvalidSchema {
                path: project.schema_path().to_owned(),
                message: error.message.clone(),
            });
        }
        Ok((project, schema))
    }
    /// Evaluate supplied Git ignore text against logical paths. No ignore files are read.
    pub fn document_is_selected(&self, project: &Project, path: &str) -> Result<bool, Error> {
        let path = normalize_source_path(path)?;
        let relative = Path::new(&path);
        if !corpus::is_mara_document(relative)
            || !corpus::content_matcher(project)?.is_match(relative)
        {
            return Ok(false);
        }
        let mut origins = self.selection.ignores.iter().collect::<Vec<_>>();
        origins.sort_by_key(|input| {
            (
                std::cmp::Reverse(input.root_prefix.len()),
                input.directory.split('/').filter(|s| !s.is_empty()).count(),
            )
        });
        let matchers = origins
            .into_iter()
            .map(|input| Ok((input, ignore_matcher(input)?)))
            .collect::<Result<Vec<_>, Error>>()?;
        let components = path.split('/').collect::<Vec<_>>();
        // Git cannot re-include a child while an ancestor directory remains ignored.
        for count in 1..=components.len() {
            let candidate = PathBuf::from(components[..count].join("/"));
            let mut ignored = false;
            for (input, matcher) in &matchers {
                let candidate = if input.root_prefix.is_empty() {
                    let Ok(relative) = candidate.strip_prefix(&input.directory) else {
                        continue;
                    };
                    if relative.as_os_str().is_empty() {
                        continue;
                    }
                    relative.to_owned()
                } else {
                    PathBuf::from(input.root_prefix.join("/")).join(&candidate)
                };
                let matched = matcher.matched(&candidate, count < components.len());
                if matched.is_ignore() {
                    ignored = true;
                } else if matched.is_whitelist() {
                    ignored = false;
                }
            }
            if ignored {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn documents(
        &self,
        project: &Project,
        schema: Option<&Schema>,
    ) -> Result<(DocumentSet, Vec<Diagnostic>), Error> {
        let mut diagnostics = Vec::new();
        let expected = self
            .selection
            .document_paths
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut supplied = Vec::new();
        for path in &expected {
            if project.content_discovery_is_complete()
                && !self.document_is_selected(project, path)?
            {
                return Err(input_error(format!(
                    "selected inventory contains excluded document '{path}'"
                )));
            }
            match self.files.get(path) {
                Some(source) => supplied.push((PathBuf::from(path), source.clone())),
                None => diagnostics.push(Diagnostic::unreadable(
                    PathBuf::from(path),
                    "missing required selected document source".into(),
                )),
            }
        }
        for path in self.files.keys() {
            if project.content_discovery_is_complete()
                && self.document_is_selected(project, path)?
                && !expected.contains(path)
            {
                return Err(input_error(format!(
                    "document '{path}' is absent from the supplied selection inventory"
                )));
            }
        }
        Ok(corpus::parse_documents_for_validation(
            project,
            supplied,
            schema,
            diagnostics,
        ))
    }
    pub fn corpus(&self) -> Result<(Project, Schema, Corpus), Error> {
        let (project, schema) = self.strict_project()?;
        let (documents, diagnostics) = self.documents(&project, Some(&schema))?;
        if let Some(diagnostic) = diagnostics.first() {
            return Err(Error::InvalidDocument {
                path: diagnostic.source().path().to_owned(),
                line: diagnostic.source().span().start_line(),
                message: diagnostic.message().into(),
            });
        }
        let code = CodeIndex::empty(&project);
        Ok((
            project,
            schema,
            Corpus {
                documents,
                code,
                source_fingerprint: self.fingerprint.clone(),
            },
        ))
    }
    pub fn schema_get(
        &self,
        kind: Option<SchemaKind>,
        name: Option<String>,
    ) -> Result<SchemaGetResult, String> {
        let (_, schema) = self.strict_project().map_err(|e| e.to_string())?;
        operations::schema_get(schema, kind, name)
    }
    pub fn schema_list(&self, kind: SchemaKind) -> Result<SchemaListResult, String> {
        let (_, schema) = self.strict_project().map_err(|e| e.to_string())?;
        Ok(operations::schema_list(&schema, kind))
    }
    pub fn item_list(&self, params: ItemFilterParams) -> Result<ItemCollectionResult, String> {
        let (_, schema, corpus) = self.corpus().map_err(|e| e.to_string())?;
        list_items(&corpus, &schema, &params.into_domain()).map_err(|e| e.to_string())
    }
    pub fn search(&self, params: SearchParams) -> Result<SearchResult, String> {
        let (_, schema, corpus) = self.corpus().map_err(|e| e.to_string())?;
        let (query, filters, ids) = params.into_parts();
        search(
            &corpus,
            &schema,
            &query,
            &filters.into_domain().with_ids(ids),
        )
        .map_err(|e| e.to_string())
    }
    pub fn get(&self, params: GetParams) -> Result<GetResult, String> {
        let (_, schema, corpus) = self.corpus().map_err(|e| e.to_string())?;
        get(
            &corpus,
            &schema,
            &params.reference,
            params.cursor.as_deref(),
        )
        .map_err(|e| e.to_string())
    }
    pub fn related(&self, params: RelatedParams) -> Result<RelatedResult, String> {
        let (project, schema, corpus) = self.corpus().map_err(|e| e.to_string())?;
        // Native code-source edges can add incoming neighbours only to eligible items.
        // The shared query already checks selected item-authored code endpoints.
        let may_need_index = !project.code_languages().is_empty()
            && params
                .direction
                .is_none_or(|direction| direction == RelationDirection::Incoming)
            && params.flavours.is_empty();
        let code_target_flavours = schema
            .relations()
            .iter()
            .filter(|(canonical, relation)| {
                relation.code_source
                    && (params.relations.is_empty()
                        || params
                            .relations
                            .iter()
                            .filter(|name| !name.starts_with("builtin:"))
                            .any(|name| {
                                schema
                                    .resolve_relation(name.strip_prefix("schema:").unwrap_or(name))
                                    .is_some_and(|(name, _, _)| name == canonical.as_str())
                            }))
            })
            .flat_map(|(_, relation)| relation.target.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        let result = related(
            &corpus,
            &schema,
            &params.reference,
            &RelatedFilters::new(params.direction, params.relations, params.flavours)
                .with_page(params.limit, params.cursor),
        )
        .map_err(|e| e.to_string())?;
        if may_need_index
            && result.node.kind == DiscoveryKind::Item
            && result
                .node
                .flavour
                .as_deref()
                .is_some_and(|flavour| code_target_flavours.contains(flavour))
        {
            return Err("code-dependent related navigation is unavailable".into());
        }
        Ok(result)
    }
    pub fn relation_get(
        &self,
        params: RelationParams,
        limit: Option<usize>,
        cursor: Option<String>,
    ) -> Result<RelationInspection, RelationError> {
        let (project, schema, corpus) = self
            .corpus()
            .map_err(|e| RelationError::new("invalid_argument", e.to_string()))?;
        relations::inspect(
            &project,
            &corpus,
            &schema,
            &params,
            limit,
            cursor.as_deref(),
        )
    }
    fn code_required(&self, project: &Project, schema: &Schema, corpus: &Corpus) -> bool {
        !project.code_languages().is_empty()
            || corpus.items().any(|item| {
                item.relations()
                    .iter()
                    .any(|r| r.target().starts_with("code:"))
            })
            || schema.relations().values().any(|r| {
                r.code_source
                    && r.cardinality.as_ref().is_some_and(|c| c.incoming.is_some())
                    && corpus
                        .items()
                        .any(|item| r.target.iter().any(|f| f == item.flavour()))
            })
    }
    pub fn schema_validate(
        &self,
        options: &ValidationOptions,
        rules: Option<&RuleChecks>,
    ) -> Result<ValidationResult, ValidationError> {
        self.validate(
            ValidationTarget {
                kind: ValidationTargetKind::Schema,
                id: None,
            },
            &[],
            options,
            rules,
        )
    }
    pub fn project_validate(
        &self,
        paths: &[PathBuf],
        options: &ValidationOptions,
        rules: Option<&RuleChecks>,
    ) -> Result<ValidationResult, ValidationError> {
        self.validate(
            ValidationTarget {
                kind: ValidationTargetKind::Project,
                id: None,
            },
            paths,
            options,
            rules,
        )
    }
    pub fn item_validate(
        &self,
        id: &str,
        options: &ValidationOptions,
        rules: Option<&RuleChecks>,
    ) -> Result<ValidationResult, ValidationError> {
        if !is_item_id(id) && !is_mid(id) {
            return Err(ValidationError::invalid_argument(
                "item must be an exact human ID or canonical MID",
            ));
        }
        self.validate(
            ValidationTarget {
                kind: ValidationTargetKind::Item,
                id: Some(id.into()),
            },
            &[],
            options,
            rules,
        )
    }
    fn validate(
        &self,
        target: ValidationTarget,
        paths: &[PathBuf],
        options: &ValidationOptions,
        rules: Option<&RuleChecks>,
    ) -> Result<ValidationResult, ValidationError> {
        let limit = options.limit.unwrap_or(20);
        if !(1..=100).contains(&limit) || options.cursor.as_deref() == Some("") {
            return Err(ValidationError::invalid_argument(
                "invalid validation limit or empty cursor",
            ));
        }
        let paths = query::normalized_paths(paths)
            .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
        let (project, project_errors, schema_available) = self
            .project_validation()
            .map_err(|e| ValidationError::invalid_argument(e.to_string()))?
            .into_parts();
        let schema_only = target.kind == ValidationTargetKind::Schema;
        let mut result = ValidationResult {
            format_version: 1,
            project: project.root().to_owned(),
            target,
            valid: false,
            evaluation_complete: true,
            diagnostics: Vec::new(),
            summary: ValidationSummary {
                errors: 0,
                warnings: 0,
                counts_exact: true,
            },
            selection: None,
            has_more: false,
            next_cursor: None,
            path: schema_only.then(|| project.schema_path().to_owned()),
            flavours: schema_only.then_some(None),
            relations: schema_only.then_some(None),
        };
        for error in project_errors {
            result
                .diagnostics
                .push(validation::configuration_diagnostic(
                    &project,
                    ValidationScope::Project,
                    &project.root().join(PROJECT_FILE),
                    error,
                ));
        }
        let schema = if schema_available {
            let (schema, errors) = parse_schema_for_validation(
                self.file(project.schema_path()).expect("schema available"),
            )
            .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
            for error in errors {
                result
                    .diagnostics
                    .push(validation::configuration_diagnostic(
                        &project,
                        ValidationScope::Schema,
                        project.schema_path(),
                        error,
                    ));
            }
            (schema.format_version() == SCHEMA_FORMAT_VERSION).then_some(schema)
        } else {
            None
        };
        result.evaluation_complete = result.diagnostics.is_empty() && schema.is_some();
        if schema_only && let Some(schema) = &schema {
            result.flavours = Some(Some(schema.flavours().len()));
            result.relations = Some(Some(schema.relations().len()));
        }
        let mut rule_definitions_usable = true;
        if !project.rule_files.is_empty() {
            for path in &project.rule_files {
                if self.file(path).is_none()
                    && rules.is_none_or(|checks| {
                        checks.definitions.evaluation_complete
                            && checks.definitions.diagnostics.is_empty()
                    })
                {
                    result.evaluation_complete = false;
                    rule_definitions_usable = false;
                    result.diagnostics.push(ValidationDiagnostic::new(
                        DiagnosticCode::RuleInvalid,
                        Severity::Error,
                        ValidationScope::Schema,
                        DiagnosticLocation::file(project.root(), path),
                        "missing or unsafe required rule source",
                    ));
                }
            }
            if let Some(rules) = rules {
                if rules.source_fingerprint != self.fingerprint {
                    return Err(ValidationError::invalid_argument(
                        "rule-check inputs belong to a different source snapshot",
                    ));
                }
                result.evaluation_complete &= rules.definitions.evaluation_complete;
                rule_definitions_usable &= rules.definitions.evaluation_complete
                    && rules.definitions.diagnostics.is_empty();
                result
                    .diagnostics
                    .extend(rules.definitions.diagnostics.clone());
            } else {
                rule_definitions_usable = false;
                unavailable(
                    &mut result,
                    ValidationScope::Schema,
                    "configured rule-definition checks are unavailable; supply matching rule-check inputs",
                );
            }
        }
        if !schema_only {
            let (documents, mut source_diagnostics) = self
                .documents(&project, schema.as_ref())
                .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
            let corpus = Corpus {
                documents,
                code: CodeIndex::empty(&project),
                source_fingerprint: self.fingerprint.clone(),
            };
            result.evaluation_complete &= corpus.is_complete()
                && corpus
                    .items()
                    .filter(|item| {
                        result
                            .target
                            .id
                            .as_deref()
                            .is_none_or(|id| id == item.id() || Some(id) == item.mid())
                    })
                    .all(Item::validation_source_is_complete);
            source_diagnostics.extend(match &schema {
                Some(schema) => validate_corpus(&corpus, schema),
                None => validate_corpus_independent(&corpus),
            });
            if let Some(schema) = &schema {
                if self.code_required(&project, schema, &corpus) {
                    unavailable(
                        &mut result,
                        ValidationScope::Project,
                        "required code information is unavailable in a document-only snapshot",
                    );
                }
                if result.diagnostics.is_empty() {
                    graph_constraints::evaluate(
                        &project,
                        &corpus,
                        schema,
                        &source_diagnostics,
                        &mut result,
                    );
                }
            }
            if !project.rule_files.is_empty() && rule_definitions_usable {
                if !source_diagnostics.is_empty()
                    || !corpus.is_complete()
                    || !result.evaluation_complete
                {
                    unavailable(
                        &mut result,
                        ValidationScope::Project,
                        "rule evaluation skipped: source conformance is invalid or incomplete",
                    );
                } else if let Some((target, checks)) = rules.and_then(|r| r.evaluation.as_ref()) {
                    if *target != result.target {
                        return Err(ValidationError::invalid_argument(
                            "rule evaluation inputs belong to a different validation target",
                        ));
                    }
                    result.evaluation_complete &= checks.evaluation_complete;
                    result.diagnostics.extend(checks.diagnostics.clone());
                } else {
                    unavailable(
                        &mut result,
                        ValidationScope::Project,
                        "configured rule evaluation is unavailable; supply matching rule-check inputs",
                    );
                }
            }
            validation::collect_source_diagnostics(&corpus, source_diagnostics, &mut result);
        }
        result.summarize();
        validation::sort_diagnostics(&mut result.diagnostics);
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(&self.fingerprint, &result, &paths, limit))
                    .expect("validation serializes")
            )
        );
        if !paths.is_empty() {
            let total = result.diagnostics.len();
            result.diagnostics.retain(|d| {
                matches!(d.scope, ValidationScope::Project | ValidationScope::Schema)
                    || d.path
                        .as_ref()
                        .is_none_or(|p| paths.iter().any(|s| p.starts_with(s)))
            });
            result.selection = Some(validation::ValidationSelection {
                paths,
                omitted_diagnostics: total - result.diagnostics.len(),
            });
        }
        validation::paginate(result, options.cursor.as_deref(), limit, &fingerprint)
    }
}
fn unavailable(result: &mut ValidationResult, scope: ValidationScope, message: &str) {
    result.evaluation_complete = false;
    result.diagnostics.push(ValidationDiagnostic::new(
        DiagnosticCode::EvaluationUnavailable,
        Severity::Error,
        scope,
        DiagnosticLocation::default(),
        message,
    ));
}

fn ignore_matcher(input: &IgnoreInput) -> Result<ignore::gitignore::Gitignore, Error> {
    let mut builder = GitignoreBuilder::new(".");
    // Match GitignoreBuilder::add without opening a file. Fingerprints retain the BOM bytes.
    for line in input.content.trim_start_matches('\u{feff}').lines() {
        builder
            .add_line(None, line)
            .map_err(|e| input_error(format!("invalid ignore input: {e}")))?;
    }
    builder
        .build()
        .map_err(|e| input_error(format!("invalid ignore input: {e}")))
}

/// Hash caller-normalized source inputs. Native hosting retains its own filename grammar;
/// document snapshots must go through SourceEngine::new before using this identity.
#[doc(hidden)]
pub fn fingerprint_source_inputs(
    files: &BTreeMap<String, String>,
    selection: &SelectionInputs,
) -> String {
    let bytes = serde_json::to_vec(&(1u32, files, selection)).expect("source inputs serialize");
    format!("{:x}", Sha256::digest(bytes))
}
