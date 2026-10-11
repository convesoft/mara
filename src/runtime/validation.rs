use super::*;
use crate::{ValidationError, ValidationOptions, ValidationSummary};
use crate::{
    load_corpus_for_validation, load_corpus_syntax_for_validation, resolve_project_for_validation,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

pub use mara_engine::validation::*;
impl NativeRuntime {
    pub fn project_validate(&self, paths: &[PathBuf]) -> Result<ValidationResult, String> {
        self.project_validate_with_options(paths, &ValidationOptions::default())
            .map_err(|e| e.to_string())
    }
    pub fn item_validate(&self, id: &str) -> Result<ValidationResult, String> {
        self.item_validate_with_options(id, &ValidationOptions::default())
            .map_err(|e| e.to_string())
    }
    pub fn schema_validate(&self) -> Result<ValidationResult, String> {
        self.schema_validate_with_options(&ValidationOptions::default())
            .map_err(|e| e.to_string())
    }
    pub fn project_validate_with_options(
        &self,
        paths: &[PathBuf],
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        let paths = crate::query::normalized_paths(paths)
            .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
        self.validate_target(
            ValidationTarget {
                kind: ValidationTargetKind::Project,
                id: None,
            },
            paths,
            options,
        )
    }
    pub fn item_validate_with_options(
        &self,
        id: &str,
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        if !crate::is_item_id(id) && !crate::is_mid(id) {
            return Err(ValidationError::invalid_argument(
                "item must be an exact human ID or canonical MID",
            ));
        }
        self.validate_target(
            ValidationTarget {
                kind: ValidationTargetKind::Item,
                id: Some(id.into()),
            },
            vec![],
            options,
        )
    }
    // @mara implements DES-SCHEMA-VALIDATION
    pub fn schema_validate_with_options(
        &self,
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        // @mara implements REQ-SCHEMA-DISCOVERY
        self.validate_target(
            ValidationTarget {
                kind: ValidationTargetKind::Schema,
                id: None,
            },
            vec![],
            options,
        )
    }

    fn validate_target(
        &self,
        target: ValidationTarget,
        paths: Vec<PathBuf>,
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        self.with_session(false, || {
            self.validate_target_current(target, paths, options)
        })
    }

    fn validate_target_current(
        &self,
        target: ValidationTarget,
        paths: Vec<PathBuf>,
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        // @mara implements REQ-PROJECT-VALIDATION
        // @mara implements DES-TRACE-DIAGNOSTIC-INTERFACE
        let limit = options.limit.unwrap_or(20);
        if !(1..=100).contains(&limit) {
            return Err(ValidationError::invalid_argument(
                "limit must be 1 through 100",
            ));
        }
        if options.cursor.as_deref() == Some("") {
            return Err(ValidationError::invalid_argument(
                "cursor must not be empty",
            ));
        }
        let (project, project_errors, schema_available) =
            match resolve_project_for_validation(self.selected.as_deref(), &self.current_directory)
            {
                Ok(value) => value.into_parts(),
                Err(error) => return Err(operation_error(error)),
            };
        let schema_only = target.kind == ValidationTargetKind::Schema;
        if !options.fact_snapshots.is_empty() || !options.mappings.is_empty() {
            if schema_only {
                return Err(ValidationError::invalid_argument(
                    "query facts and mappings apply only to project or item validation",
                ));
            }
            if project.query_policy().is_none() {
                return Err(ValidationError::invalid_argument(
                    "query facts and mappings require an enabled query policy",
                ));
            }
        }
        let mut result = ValidationResult {
            format_version: 1,
            project: project.root().to_owned(),
            target,
            valid: false,
            evaluation_complete: true,
            diagnostics: vec![],
            query_policies: None,
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
            result.diagnostics.push(configuration_diagnostic(
                &project,
                ValidationScope::Project,
                &project.root().join(crate::PROJECT_FILE),
                error,
            ));
        }
        let schema = if schema_available {
            match crate::load_schema_for_validation(&project) {
                Ok((schema, errors)) => {
                    for error in errors {
                        result.diagnostics.push(configuration_diagnostic(
                            &project,
                            ValidationScope::Schema,
                            project.schema_path(),
                            error,
                        ));
                    }
                    if schema.format_version() == crate::SCHEMA_FORMAT_VERSION {
                        Some(schema)
                    } else {
                        None
                    }
                }
                Err(error) => return Err(operation_error(error)),
            }
        } else {
            None
        };
        result.evaluation_complete = result.diagnostics.is_empty() && schema.is_some();
        let mut snapshot = Sha256::new();
        snapshot.update(b"validation-1-complete-evaluation-1");
        hash_file(&mut snapshot, &project.root().join(crate::PROJECT_FILE));
        hash_file(&mut snapshot, project.schema_path());
        snapshot.update(b"yaml-shacl-binding-1-registry-0.3.21-adapter-10");
        let rules = schema
            .as_ref()
            .map(|schema| crate::native_rules::load(&project, schema));
        if let Some(rules) = &rules {
            for path in &rules.files {
                hash_file(&mut snapshot, &project.root().join(path));
            }
        }
        if let Some(schema) = &schema
            && schema_only
        {
            result.flavours = Some(Some(schema.flavours().len()));
            result.relations = Some(Some(schema.relations().len()));
        }
        let mut policy_documents = None;
        let mut policy_code = None;
        if !schema_only {
            let (corpus, source_diagnostics) = match &schema {
                Some(schema) => load_corpus_for_validation(&project, schema),
                None => load_corpus_syntax_for_validation(&project),
            }
            .map_err(operation_error)?;
            let mut source_paths = BTreeSet::new();
            for document in corpus.documents() {
                source_paths.insert(document.path().to_owned());
            }
            for diagnostic in &source_diagnostics {
                source_paths.insert(diagnostic.source().path().to_owned());
            }
            for file in corpus.code().files() {
                source_paths.insert(file.path.clone());
            }
            for path in corpus.code().assets() {
                source_paths.insert(path.clone());
            }
            for path in source_paths {
                hash_file(&mut snapshot, &project.root().join(path));
            }
            for path in corpus.file_only_code_paths() {
                snapshot.update(b"file-only-code");
                let identity = path.as_os_str().as_encoded_bytes();
                snapshot.update(identity.len().to_le_bytes());
                snapshot.update(identity);
                match corpus.code().file_only_bytes(&path) {
                    Some(bytes) => {
                        snapshot.update([1]);
                        snapshot.update(bytes.len().to_le_bytes());
                        snapshot.update(bytes);
                    }
                    None => snapshot.update([0]),
                }
            }
            policy_code = Some(corpus.code().clone());
            policy_documents = Some(mara_engine::DocumentSet {
                documents: corpus.documents().to_vec(),
                complete: corpus.is_complete(),
            });
            let checks = rules.as_ref().zip(schema.as_ref()).map(|(rules, schema)| {
                crate::rule_bridge::checks_for_loaded(
                    &project,
                    schema,
                    rules,
                    result.target.clone(),
                    Some((&corpus, &source_diagnostics)),
                    &corpus.source_fingerprint,
                )
            });
            mara_engine::validation::evaluate_loaded(
                &project,
                schema.as_ref(),
                Some((&corpus, source_diagnostics)),
                checks.as_ref(),
                &corpus.source_fingerprint,
                &mut result,
            )?;
        }
        if schema_only {
            let checks = rules.as_ref().zip(schema.as_ref()).map(|(rules, schema)| {
                crate::rule_bridge::checks_for_loaded(
                    &project,
                    schema,
                    rules,
                    result.target.clone(),
                    None,
                    "",
                )
            });
            mara_engine::validation::evaluate_loaded(
                &project,
                schema.as_ref(),
                None,
                checks.as_ref(),
                "",
                &mut result,
            )?;
        }
        if project.query_policy().is_some() {
            match crate::source_bridge::load_query_policy_engine(
                &project,
                policy_documents.as_ref(),
                policy_code.as_ref(),
            ) {
                Ok(source_engine) => mara_engine::query_consumers::append_query_policy_result(
                    &mut result,
                    &source_engine,
                    &options.fact_snapshots,
                    &options.mappings,
                )
                .map_err(|error| ValidationError::new(&error.code, error.message))?,
                Err(error) => mara_engine::query_consumers::append_query_policy_unavailable(
                    &mut result,
                    format!("query policy inputs unavailable: {error}"),
                ),
            }
        }
        result.summarize();
        sort_diagnostics(&mut result.diagnostics);
        // Include diagnostics for unreadable sources and options as well as source bytes.
        snapshot
            .update(serde_json::to_vec(&(&result, &paths, limit)).expect("validation serializes"));
        let fingerprint = format!("{:x}", snapshot.finalize());
        if !paths.is_empty() {
            let total = result.diagnostics.len();
            result.diagnostics.retain(|diagnostic| {
                matches!(
                    diagnostic.scope,
                    ValidationScope::Project | ValidationScope::Schema
                ) || diagnostic
                    .path
                    .as_ref()
                    .is_none_or(|source| paths.iter().any(|path| source.starts_with(path)))
            });
            result.selection = Some(ValidationSelection {
                paths,
                omitted_diagnostics: total - result.diagnostics.len(),
            });
        }
        paginate(result, options.cursor.as_deref(), limit, &fingerprint)
    }
}

fn operation_error(error: crate::Error) -> ValidationError {
    let code = if matches!(error, crate::Error::Io { .. }) {
        "io_error"
    } else {
        "invalid_argument"
    };
    ValidationError::new(code, error.to_string())
}

fn hash_file(hash: &mut Sha256, path: &Path) {
    let identity = path.to_string_lossy();
    hash.update(identity.len().to_le_bytes());
    hash.update(identity.as_bytes());
    match fs::read(path) {
        Ok(bytes) => {
            hash.update([1]);
            hash.update(bytes.len().to_le_bytes());
            hash.update(bytes);
        }
        Err(error) => {
            hash.update([0]);
            hash.update(format!("{:?}", error.kind()).as_bytes());
        }
    }
}
