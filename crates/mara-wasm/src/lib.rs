//! Thin memory host. Semantic planning/checking stays in mara-engine.
use mara_engine::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf};

mod change_set;
pub use change_set::{FileChange, FileChangeSet};

#[cfg(target_arch = "wasm32")]
mod binding;

pub const OPERATIONS: &[&str] = &[
    "capabilities",
    "load_snapshot",
    "workspace_state",
    "workspace_sources",
    "export_changes",
    "check_change_set_preconditions",
    "project_init",
    "project_mid_backfill",
    "schema_get",
    "schema_list",
    "schema_validate",
    "item_list",
    "search",
    "get",
    "related",
    "item_create",
    "item_update",
    "item_delete",
    "item_move",
    "item_rename",
    "relation_get",
    "relation_add",
    "relation_remove",
    "project_validate",
    "item_validate",
    "trace_matrix",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub operation: String,
    #[serde(default = "empty_object")]
    pub params: Value,
}
fn empty_object() -> Value {
    json!({})
}

/// Retains a normalized base independently of current memory. Export never publishes.
pub struct MemoryWorkspace {
    engine: SourceEngine,
    base: SourceSnapshot,
    base_fingerprint: String,
}
impl MemoryWorkspace {
    pub fn new(snapshot: SourceSnapshot) -> Result<Self, Error> {
        let engine = SourceEngine::new(snapshot)?;
        check_inventory(&engine)?;
        Ok(Self {
            base: engine.snapshot(),
            base_fingerprint: engine.source_fingerprint().into(),
            engine,
        })
    }
    pub fn base_snapshot(&self) -> &SourceSnapshot {
        &self.base
    }
    pub fn current_snapshot(&self) -> SourceSnapshot {
        self.engine.snapshot()
    }

    /// Consolidate original base versus current sources without advancing either state.
    // @mara implements REQ-BROWSER-CHANGE-EXPORT
    pub fn export_changes(&self) -> FileChangeSet {
        change_set::consolidate(&self.base, self.engine.files(), &self.base_fingerprint)
    }

    /// Entropy/time are supplied by the host, never by portable semantics.
    // @mara implements REQ-BROWSER-LOCAL-WORKSPACE
    // @mara implements REQ-EXPLICIT-BACKEND-CAPABILITIES
    pub fn execute(
        &mut self,
        request: Request,
        mut generate_mid: impl FnMut() -> Result<String, Value>,
    ) -> Result<Value, Value> {
        let p = request.params;
        match request.operation.as_str() {
            "capabilities" => {
                parse::<Empty>(p)?;
                Ok(json!({
                    "format_version": 1, "operations": OPERATIONS,
                    "code": {"indexing": false, "native_index_import": false},
                    "mutation_persistence": "memory", "native_recovery": false
                }))
            }
            "load_snapshot" => {
                // Construct first: even a malformed replacement cannot lose current/base state.
                let replacement = Self::new(parse(p)?).map_err(argument)?;
                *self = replacement;
                Ok(self.state())
            }
            "workspace_state" => {
                parse::<Empty>(p)?;
                Ok(self.state())
            }
            "workspace_sources" => {
                parse::<Empty>(p)?;
                Ok(json!({"base": self.base, "current": self.engine.snapshot()}))
            }
            "export_changes" => {
                parse::<Empty>(p)?;
                value(self.export_changes())
            }
            "check_change_set_preconditions" => {
                let p: CheckChangeSet = parse(p)?;
                p.change_set
                    .check_preconditions(p.snapshot)
                    .map_err(|e| envelope("precondition_failed", e))?;
                Ok(
                    json!({"format_version":1, "source_preconditions_match":true,
                    "external_context_verified":false}),
                )
            }
            "project_init" => {
                let p: Init = parse(p)?;
                let plan = self
                    .engine
                    .plan_initialize(&p.name, p.template)
                    .map_err(argument)?;
                let mut created: Vec<_> = plan.changes().iter().map(|c| c.path.clone()).collect();
                created.sort();
                let candidate = SourceEngine::new(plan.candidate().clone()).map_err(argument)?;
                let (project, _) = project_inputs(&candidate).map_err(argument)?;
                let result = json!({"created": created, "project": {
                    "root": ".", "name": project.name(), "schema_path": project.schema_path(),
                    "content_patterns": project.content_patterns()
                }});
                self.engine.apply_plan(plan).map_err(mutation)?;
                Ok(result)
            }
            "schema_get" => {
                let p: SchemaGet = parse(p)?;
                value(self.engine.schema_get(p.kind, p.name).map_err(argument)?)
            }
            "schema_list" => {
                let p: SchemaList = parse(p)?;
                value(self.engine.schema_list(p.kind).map_err(argument)?)
            }
            "item_list" => value(self.engine.item_list(parse(p)?).map_err(argument)?),
            "search" => value(self.engine.search(parse(p)?).map_err(argument)?),
            "get" => value(self.engine.get(parse(p)?).map_err(argument)?),
            "related" => value(self.engine.related(parse(p)?).map_err(argument)?),
            "relation_get" => {
                let p: RelationRead = parse(p)?;
                value(
                    self.engine
                        .relation_get(p.edge(), p.limit, p.cursor)
                        .map_err(relation_error)?,
                )
            }
            "schema_validate" | "project_validate" | "item_validate" => {
                let p: Validate = parse(p)?;
                let kind = match request.operation.as_str() {
                    "schema_validate" => ValidationTargetKind::Schema,
                    "item_validate" => ValidationTargetKind::Item,
                    _ => ValidationTargetKind::Project,
                };
                if kind == ValidationTargetKind::Item && p.id.is_none() {
                    return Err(argument("item_validate requires id"));
                }
                if kind != ValidationTargetKind::Item && p.id.is_some()
                    || kind != ValidationTargetKind::Project && !p.paths.is_empty()
                {
                    return Err(argument("validation options do not apply to this target"));
                }
                let target = ValidationTarget {
                    kind,
                    id: p.id.clone(),
                };
                // Recovery validation still reports original config/schema errors when compilation
                // cannot start; absent configured outcomes never become a passing check.
                let checks = self.engine.rule_checks(target).ok();
                let options = ValidationOptions {
                    limit: p.limit,
                    cursor: p.cursor,
                };
                let result = match kind {
                    ValidationTargetKind::Schema => {
                        self.engine.schema_validate(&options, checks.as_ref())
                    }
                    ValidationTargetKind::Project => {
                        self.engine
                            .project_validate(&p.paths, &options, checks.as_ref())
                    }
                    ValidationTargetKind::Item => self.engine.item_validate(
                        p.id.as_deref().unwrap(),
                        &options,
                        checks.as_ref(),
                    ),
                }
                .map_err(|e| e.envelope())?;
                value(result)
            }
            "trace_matrix" => value(
                self.engine
                    .trace_matrix(&parse(p)?)
                    .map_err(|e| e.envelope())?,
            ),
            "project_mid_backfill" => {
                parse::<Empty>(p)?;
                // Acquire the same full-source/code gate even if no MIDs are missing.
                let planner = self.engine.planner().map_err(mutation)?;
                let (_, _, corpus) = self.engine.corpus().map_err(mutation)?;
                let mids = corpus
                    .items()
                    .filter(|i| i.mid().is_none())
                    .map(|_| generate_mid())
                    .collect::<Result<Vec<_>, _>>()?;
                let plan = planner.plan_backfill(&mids).map_err(mutation)?;
                let changed: Vec<_> = plan
                    .result()
                    .entries()
                    .iter()
                    .map(|e| json!({"id":e.id(), "mid":e.mid(), "path":e.path(), "line":e.line()}))
                    .collect();
                self.engine.apply_plan(plan).map_err(mutation)?;
                Ok(json!({"project": ".", "changed": changed}))
            }
            "item_create" => {
                let p: Create = parse(p)?;
                let planner = self.engine.planner().map_err(mutation)?;
                let id = p.id.clone();
                let plan = planner
                    .plan_create(p.into_domain(), &generate_mid()?)
                    .map_err(mutation)?;
                let r = plan.result();
                let result = json!({
                    "id": id, "mid": r.mid(), "path": r.path(), "line": r.line(),
                    "complete": r.is_complete(), "missing": if r.is_complete() {vec![]} else {vec!["body"]}
                });
                self.engine.apply_plan(plan).map_err(mutation)?;
                Ok(result)
            }
            "item_update" => {
                let plan = self
                    .engine
                    .planner()
                    .map_err(mutation)?
                    .plan_update(parse(p)?)
                    .map_err(mutation)?;
                self.accept(plan)
            }
            "item_delete" => {
                let p: Reference = parse(p)?;
                let plan = self
                    .engine
                    .planner()
                    .map_err(mutation)?
                    .plan_delete(&p.reference)
                    .map_err(mutation)?;
                self.accept(plan)
            }
            "item_move" => {
                let p: ItemMoveParams = parse(p)?;
                let plan = self
                    .engine
                    .planner()
                    .map_err(mutation)?
                    .plan_move(&p.reference, &p.file, p.line)
                    .map_err(mutation)?;
                self.accept(plan)
            }
            "item_rename" => {
                let p: Rename = parse(p)?;
                let plan = self
                    .engine
                    .planner()
                    .map_err(mutation)?
                    .plan_rename(&p.reference, &p.new_id)
                    .map_err(mutation)?;
                self.accept(plan)
            }
            "relation_add" | "relation_remove" => {
                let p: RelationWrite = parse(p)?;
                if request.operation == "relation_add" && p.occurrence.is_some() {
                    return Err(argument("occurrence applies only to relation_remove"));
                }
                let plan = self
                    .engine
                    .planner()
                    .map_err(mutation)?
                    .plan_relation(
                        &p.edge(),
                        request.operation == "relation_add",
                        p.occurrence.as_deref(),
                    )
                    .map_err(relation_error)?;
                self.accept(plan)
            }
            _ => Err(envelope(
                "unsupported_operation",
                "operation is not supported by the memory workspace",
            )),
        }
    }
    fn state(&self) -> Value {
        json!({"format_version":1, "source_fingerprint":self.engine.source_fingerprint(),
            "base_source_fingerprint":self.base_fingerprint, "base_identity":self.base.base_identity,
            "mutation_persistence":"memory"})
    }
    fn accept<T: Serialize>(&mut self, plan: ChangePlan<T>) -> Result<Value, Value> {
        // Serialize before state replacement; rejected binding output leaves memory untouched too.
        let result = value(plan.result())?;
        self.engine.apply_plan(plan).map_err(mutation)?;
        Ok(result)
    }
}
// Presence checks are not a weaker conformance profile. Invalid source remains available
// for recovering validation; absent required inputs are never manufactured.
fn project_inputs(engine: &SourceEngine) -> Result<(Project, Option<PathBuf>), Error> {
    let source = engine
        .files()
        .get(PROJECT_FILE)
        .ok_or_else(|| Error::InvalidProject {
            path: PROJECT_FILE.into(),
            message: "missing required source .mara/project.toml".into(),
        })?;
    // The recovery parser asks about a schema only when it decoded an actual
    // configured relative path. A fallback path after invalid TOML is not an input.
    let missing_schema = RefCell::new(None);
    let validation = parse_project_for_validation(PathBuf::from("."), source, |path| {
        let exists = path
            .to_str()
            .and_then(|p| normalize_source_path(p).ok())
            .is_some_and(|p| engine.files().contains_key(&p));
        if !exists {
            *missing_schema.borrow_mut() = Some(path.to_path_buf());
        }
        exists
    })?;
    Ok((validation.into_parts().0, missing_schema.into_inner()))
}
fn check_inventory(engine: &SourceEngine) -> Result<(), Error> {
    let snapshot = engine.snapshot();
    let missing = |path: &str| Error::InvalidProject {
        path: path.into(),
        message: format!("missing required source '{path}'"),
    };
    for path in &snapshot.selection.document_paths {
        if !engine.files().contains_key(path) {
            return Err(missing(path));
        }
    }
    if engine.files().is_empty() {
        return Ok(());
    } // Explicit empty initialization.
    let (project, missing_schema) = project_inputs(engine)?;
    if let Some(path) = missing_schema {
        return Err(missing(
            path.to_str().unwrap_or("non-UTF-8 configuration path"),
        ));
    }
    for path in &project.rule_files {
        let path = path
            .to_str()
            .ok_or_else(|| missing("non-UTF-8 configuration path"))?;
        let normalized = normalize_source_path(path)?;
        if !engine.files().contains_key(&normalized) {
            return Err(missing(&normalized));
        }
    }
    if project.content_discovery_is_complete() {
        for path in &snapshot.selection.document_paths {
            if !engine.document_is_selected(&project, path)? {
                return Err(Error::InvalidProject {
                    path: path.into(),
                    message: "selected inventory contains excluded document".into(),
                });
            }
        }
        for path in engine.files().keys() {
            if path.ends_with(".mara.md")
                && engine.document_is_selected(&project, path)?
                && !snapshot.selection.document_paths.contains(path)
            {
                return Err(Error::InvalidProject {
                    path: path.into(),
                    message: "included document is missing from the selected inventory".into(),
                });
            }
        }
    }
    Ok(())
}
fn parse<T: DeserializeOwned>(params: Value) -> Result<T, Value> {
    serde_json::from_value(params).map_err(argument)
}
fn value(value: impl Serialize) -> Result<Value, Value> {
    serde_json::to_value(value).map_err(|e| envelope("binding_error", e))
}
pub fn envelope(code: &str, message: impl ToString) -> Value {
    json!({"format_version":1, "error":{"code":code, "message":message.to_string()}})
}
fn argument(message: impl ToString) -> Value {
    envelope("invalid_argument", message)
}
fn mutation(message: impl ToString) -> Value {
    envelope("invalid_mutation", message)
}
fn relation_error(error: RelationError) -> Value {
    serde_json::to_value(error).expect("relation error is serializable")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckChangeSet {
    change_set: FileChangeSet,
    snapshot: SourceSnapshot,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Init {
    name: String,
    #[serde(default)]
    template: Template,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaGet {
    #[serde(default)]
    kind: Option<SchemaKind>,
    #[serde(default)]
    name: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaList {
    kind: SchemaKind,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    reference: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    reference: String,
    new_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Validate {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    paths: Vec<PathBuf>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationRead {
    source: String,
    relation: String,
    target: String,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    cursor: Option<String>,
}
impl RelationRead {
    fn edge(&self) -> RelationParams {
        RelationParams {
            source: self.source.clone(),
            relation: self.relation.clone(),
            target: self.target.clone(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationWrite {
    source: String,
    relation: String,
    target: String,
    #[serde(default)]
    occurrence: Option<String>,
}
impl RelationWrite {
    fn edge(&self) -> RelationParams {
        RelationParams {
            source: self.source.clone(),
            relation: self.relation.clone(),
            target: self.target.clone(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    flavour: String,
    id: String,
    file: PathBuf,
    title: String,
    #[serde(default)]
    fields: Vec<FieldValue>,
    #[serde(default)]
    relations: Vec<InitialRelation>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    line: Option<usize>,
}
impl Create {
    fn into_domain(self) -> ItemCreationRequest {
        ItemCreationRequest {
            flavour: self.flavour,
            id: self.id,
            file: self.file,
            title: self.title,
            fields: self.fields.into_iter().map(|f| (f.key, f.value)).collect(),
            relations: self.relations,
            body: self.body,
            line: self.line,
        }
    }
}
