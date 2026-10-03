//! Embedded native execution. No daemon routing, session cache or transport policy.
use crate::operations::*;
use crate::{Corpus, ItemCollectionResult, backfill_mids, list_items, load_corpus};
use crate::{GetResult, RelatedFilters, RelatedResult, SearchResult, get, related, search};
use crate::{ItemCreationRequest, create_item};
use crate::{Project, Schema, Template, initialize_project, load_schema, resolve_project};
use std::{env, path::PathBuf};
mod validation;

/// Direct native operation surface, reusable by embedded consumers and a future daemon.
/// Each call loads current project inputs; this type does not cache or route requests.
// @mara implements DES-EXECUTION-BOUNDARIES
#[derive(Debug, Clone)]
pub struct NativeRuntime {
    selected: Option<PathBuf>,
    current_directory: PathBuf,
}

impl NativeRuntime {
    /// Select a project in the caller's context without reading process environment.
    pub fn new(selected: Option<PathBuf>, current_directory: PathBuf) -> Result<Self, String> {
        if !current_directory.is_absolute() {
            return Err("caller current directory must be absolute".into());
        }
        let selected = selected.map(|path| {
            if path.is_absolute() {
                path
            } else {
                current_directory.join(path)
            }
        });
        Ok(Self {
            selected,
            current_directory,
        })
    }

    pub fn trace_matrix(
        &self,
        params: &crate::TraceMatrixParams,
    ) -> Result<crate::TraceMatrixResult, crate::ValidationError> {
        let project = resolve_project(self.selected.as_deref(), &self.current_directory)
            .map_err(|e| crate::ValidationError::new("io_error", e.to_string()))?;
        let schema = load_schema(&project).map_err(|e| match e {
            crate::Error::Io { .. } => crate::ValidationError::new("io_error", e.to_string()),
            _ => crate::ValidationError::invalid_argument(e.to_string()),
        })?;
        crate::native_trace::matrix(&project, &schema, params)
    }

    pub fn from_environment(selected: Option<PathBuf>) -> Result<Self, String> {
        let current_directory = env::current_dir()
            .map_err(|error| format!("could not read current directory: {error}"))?;
        Self::new(selected, current_directory)
    }

    // @mara implements DES-OPERATION-PROJECT-CONTEXT
    pub fn for_project(&self, requested: Option<PathBuf>) -> Result<Self, String> {
        if requested.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err("request project path must be absolute".into());
        }
        if self.selected.is_some() && requested.is_some() {
            return Err(
                "project cannot be selected per operation because the server started with --project"
                    .into(),
            );
        }
        Ok(Self {
            selected: requested.or_else(|| self.selected.clone()),
            current_directory: self.current_directory.clone(),
        })
    }

    pub fn project_initialize(
        &self,
        target: Option<PathBuf>,
        template: Template,
    ) -> Result<ProjectInitializationResult, String> {
        let operation = self.for_project(target)?;
        project_initialize(
            operation
                .selected
                .ok_or_else(|| {
                    "project init requires an absolute project path when the server is not bound with --project"
                        .to_string()
                })?,
            template,
        )
    }

    pub fn item_create(&self, request: ItemCreateParams) -> Result<ItemCreationResult, String> {
        let (project, schema) = self.load_project()?;
        let id = request.id.clone();
        let created = create_item(
            &project,
            &schema,
            ItemCreationRequest {
                flavour: request.flavour,
                id: request.id,
                file: request.file,
                title: request.title,
                fields: request
                    .fields
                    .into_iter()
                    .map(|field| (field.key, field.value))
                    .collect(),
                relations: request.relations,
                body: request.body,
                line: request.line,
            },
        )
        .map_err(|error| error.to_string())?;
        let complete = created.is_complete();
        Ok(ItemCreationResult {
            id,
            mid: created.mid().to_owned(),
            path: created.path().to_path_buf(),
            line: created.line(),
            complete,
            missing: if complete {
                Vec::new()
            } else {
                vec!["body".into()]
            },
        })
    }

    pub fn project_mid_backfill(&self) -> Result<ProjectMidBackfillResult, String> {
        let (project, schema) = self.load_project()?;
        let result = backfill_mids(&project, &schema).map_err(|error| error.to_string())?;
        Ok(ProjectMidBackfillResult {
            project: project.root().to_path_buf(),
            changed: result
                .entries()
                .iter()
                .map(|entry| BackfilledMidResult {
                    id: entry.id().to_owned(),
                    mid: entry.mid().to_owned(),
                    path: entry.path().to_path_buf(),
                    line: entry.line(),
                })
                .collect(),
        })
    }

    pub fn project_transaction_rollback(&self) -> Result<TransactionRollbackResult, String> {
        // Recovery must work even when interrupted writes made the corpus invalid.
        let project = resolve_project(self.selected.as_deref(), &self.current_directory)
            .map_err(|error| error.to_string())?;
        let result = crate::rollback_transaction(&project).map_err(|error| error.to_string())?;
        Ok(TransactionRollbackResult {
            project: project.root().to_path_buf(),
            restored: result.restored,
        })
    }

    // @mara implements REQ-SCHEMA-DISCOVERY
    pub fn schema_get(
        &self,
        kind: Option<SchemaKind>,
        name: Option<String>,
    ) -> Result<SchemaGetResult, String> {
        let (_, schema) = self.load_project()?;
        mara_engine::operations::schema_get(schema, kind, name)
    }

    pub fn schema_list(&self, kind: SchemaKind) -> Result<SchemaListResult, String> {
        let (_, schema) = self.load_project()?;
        Ok(mara_engine::operations::schema_list(&schema, kind))
    }

    pub fn relation_get(
        &self,
        params: RelationParams,
        limit: Option<usize>,
        cursor: Option<String>,
    ) -> Result<crate::RelationInspection, crate::RelationError> {
        let (project, schema) = self.load_project()?;
        let corpus = load_corpus(&project, &schema)?;
        crate::relations::inspect(
            &project,
            &corpus,
            &schema,
            &params,
            limit,
            cursor.as_deref(),
        )
    }

    pub fn item_move(&self, params: ItemMoveParams) -> Result<crate::ItemMove, String> {
        let (project, schema) = self.load_project()?;
        crate::move_item(
            &project,
            &schema,
            &params.reference,
            &params.file,
            params.line,
        )
        .map_err(|error| error.to_string())
    }

    pub fn item_rename(&self, reference: &str, new_id: &str) -> Result<crate::ItemRename, String> {
        let (project, schema) = self.load_project()?;
        crate::rename_item(&project, &schema, reference, new_id).map_err(|error| error.to_string())
    }

    pub fn item_delete(&self, reference: &str) -> Result<crate::ItemDeletion, String> {
        let (project, schema) = self.load_project()?;
        crate::delete_item(&project, &schema, reference).map_err(|error| error.to_string())
    }

    pub fn item_update(&self, params: ItemUpdateParams) -> Result<crate::ItemUpdate, String> {
        let (project, schema) = self.load_project()?;
        crate::update_item(&project, &schema, params).map_err(|error| error.to_string())
    }

    pub fn relation_add(
        &self,
        params: RelationParams,
    ) -> Result<RelationMutationResult, crate::RelationError> {
        let (project, schema) = self.load_project()?;
        crate::mutation::mutate_semantic_relation(&project, &schema, &params, true, None)
    }

    pub fn relation_remove(
        &self,
        params: RelationParams,
    ) -> Result<RelationMutationResult, crate::RelationError> {
        self.relation_remove_occurrence(params, None)
    }

    pub fn relation_remove_occurrence(
        &self,
        params: RelationParams,
        occurrence: Option<String>,
    ) -> Result<RelationMutationResult, crate::RelationError> {
        let (project, schema) = self.load_project()?;
        crate::mutation::mutate_semantic_relation(
            &project,
            &schema,
            &params,
            false,
            occurrence.as_deref(),
        )
    }

    pub fn related(&self, params: RelatedParams) -> Result<RelatedResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        let filters = RelatedFilters::new(params.direction, params.relations, params.flavours)
            .with_page(params.limit, params.cursor);
        related(&corpus, &schema, &params.reference, &filters).map_err(|error| error.to_string())
    }

    pub fn get(&self, params: GetParams) -> Result<GetResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        get(
            &corpus,
            &schema,
            &params.reference,
            params.cursor.as_deref(),
        )
        .map_err(|error| error.to_string())
    }

    pub fn search(&self, params: SearchParams) -> Result<SearchResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        let (query, filters, ids) = params.into_parts();
        search(
            &corpus,
            &schema,
            &query,
            &filters.into_domain().with_ids(ids),
        )
        .map_err(|error| error.to_string())
    }

    pub fn item_list(&self, filters: ItemFilterParams) -> Result<ItemCollectionResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        list_items(&corpus, &schema, &filters.into_domain()).map_err(|error| error.to_string())
    }

    fn load_query_project(&self) -> Result<(Corpus, Schema), String> {
        let (project, schema) = self.load_project()?;
        let corpus = load_corpus(&project, &schema).map_err(|error| error.to_string())?;
        Ok((corpus, schema))
    }

    fn load_project(&self) -> Result<(Project, Schema), String> {
        let project = resolve_project(self.selected.as_deref(), &self.current_directory)
            .map_err(|error| error.to_string())?;
        let schema = load_schema(&project).map_err(|error| error.to_string())?;
        Ok((project, schema))
    }
}

pub fn project_initialize(
    target: PathBuf,
    template: Template,
) -> Result<ProjectInitializationResult, String> {
    let project = initialize_project(target, template).map_err(|error| error.to_string())?;
    Ok(ProjectInitializationResult {
        project: ProjectSummary {
            root: project.root().to_path_buf(),
            name: project.name().to_owned(),
            schema_path: project.schema_path().to_path_buf(),
            content_patterns: project.content_patterns().to_vec(),
        },
        created: vec![crate::PROJECT_FILE.into(), crate::SCHEMA_FILE.into()],
    })
}
