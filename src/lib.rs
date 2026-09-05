use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use globset::GlobBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

mod corpus;
mod mutation;
mod operations;
mod query;

pub use corpus::{
    Corpus, Diagnostic, Document, Item, Mention, MetadataEntry, Relation, SourceLocation,
    SourceSpan, load_corpus, load_corpus_for_validation, load_corpus_syntax_for_validation,
    validate_corpus, validate_corpus_independent,
};
pub use mutation::{
    BackfilledMid, BackfilledMids, InitialRelation, ItemCreation, ItemCreationRequest,
    ItemDeletion, ItemLocation, ItemMove, ItemRename, ItemUpdate, RelationMutation,
    TransactionRollback, add_relation, backfill_mids, create_item, delete_item, move_item,
    remove_relation, rename_item, rollback_transaction, update_item,
};
pub use operations::{
    DeclarationSummary, FieldValue, ItemCreateParams, ItemCreationResult, ItemFilterParams,
    ItemGetParams, ItemIdParams, ItemMoveParams, ItemRelatedParams, ItemSearchParams,
    ItemUpdateParams, OperationContext, ProjectInitializationResult, ProjectMidBackfillResult,
    ProjectSummary, RelationAction, RelationMutationResult, RelationParams, SchemaGetResult,
    SchemaKind, SchemaListResult, SchemaValidationResult, TransactionRollbackResult,
    ValidationDiagnostic, ValidationResult, ValidationScope, ValidationTarget,
    ValidationTargetKind, project_initialize,
};
pub use query::{
    EntryRange, FieldFilter, ItemCollectionResult, ItemFilters, ItemGetResult, ItemSource,
    ItemSummary, MetadataFragment, MetadataValue, QueryError, RelatedFilters, RelatedItem,
    RelatedItemsResult, RelationDirection, RelationSummary, ResolvedItem, SearchExcerpt, TextRange,
    get_item, get_item_page, list_items, related_items, search_items,
};

pub const PROJECT_FILE: &str = ".mara/project.toml";
pub const SCHEMA_FILE: &str = ".mara/schema.yaml";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Template {
    #[default]
    Minimal,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    root: PathBuf,
    name: String,
    schema_path: PathBuf,
    content_patterns: Vec<String>,
    content_discovery_complete: bool,
}

#[derive(Debug)]
pub struct ProjectValidation {
    project: Project,
    errors: Vec<String>,
    schema_available: bool,
}

impl ProjectValidation {
    pub fn into_parts(self) -> (Project, Vec<String>, bool) {
        (self.project, self.errors, self.schema_available)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    format_version: u32,
    flavours: BTreeMap<String, FlavourDefinition>,
    relations: BTreeMap<String, RelationDefinition>,
    #[serde(skip)]
    #[schemars(skip)]
    validation: SchemaValidationState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum SchemaValue {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Sequence(Vec<SchemaValue>),
    Mapping(BTreeMap<String, SchemaValue>),
    Null,
}

#[derive(Debug, Deserialize)]
struct SchemaFileForValidation {
    format_version: Option<SchemaValue>,
    flavours: Option<SchemaValue>,
    relations: Option<SchemaValue>,
    #[serde(flatten)]
    unknown: BTreeMap<String, SchemaValue>,
}

#[derive(Debug, Deserialize)]
struct FlavourFileForValidation {
    description: Option<SchemaValue>,
    id_prefix: Option<SchemaValue>,
    body: Option<SchemaValue>,
    fields: Option<SchemaValue>,
    #[serde(flatten)]
    unknown: BTreeMap<String, SchemaValue>,
}

#[derive(Debug)]
struct RecoveredFlavour {
    definition: FlavourDefinition,
    description_valid: bool,
    id_prefix_valid: bool,
    body_valid: bool,
    fields_valid: bool,
    invalid_fields: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SchemaValidationState {
    format_version_invalid: bool,
    flavours_section_invalid: bool,
    relations_section_invalid: bool,
    invalid_flavours: HashSet<String>,
    invalid_flavour_descriptions: HashSet<String>,
    invalid_id_prefixes: HashSet<String>,
    invalid_bodies: HashSet<String>,
    invalid_field_sections: HashSet<String>,
    invalid_fields: HashSet<(String, String)>,
    invalid_field_values: HashSet<(String, String)>,
    invalid_relations: HashSet<String>,
    invalid_relation_sources: HashSet<String>,
    invalid_relation_targets: HashSet<String>,
    invalid_same_flavour: HashSet<String>,
}

impl Schema {
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    pub fn flavours(&self) -> &BTreeMap<String, FlavourDefinition> {
        &self.flavours
    }

    pub fn relations(&self) -> &BTreeMap<String, RelationDefinition> {
        &self.relations
    }

    fn flavour_for_validation(&self, name: &str) -> Option<&FlavourDefinition> {
        (!self.validation.invalid_flavours.contains(name))
            .then(|| self.flavours.get(name))
            .flatten()
    }

    fn flavour_is_declared(&self, name: &str) -> bool {
        self.validation.flavours_section_invalid
            || self.flavours.contains_key(name)
            || self.validation.invalid_flavours.contains(name)
    }

    fn id_prefix_is_valid(&self, flavour: &str) -> bool {
        !self.validation.invalid_id_prefixes.contains(flavour)
    }

    fn body_is_valid(&self, flavour: &str) -> bool {
        !self.validation.invalid_bodies.contains(flavour)
    }

    fn field_is_declared(&self, flavour: &str, field: &str) -> bool {
        self.validation.invalid_field_sections.contains(flavour)
            || self
                .flavours
                .get(flavour)
                .is_some_and(|definition| definition.fields.contains_key(field))
            || self
                .validation
                .invalid_fields
                .contains(&(flavour.to_owned(), field.to_owned()))
    }

    fn field_is_valid(&self, flavour: &str, field: &str) -> bool {
        !self
            .validation
            .invalid_fields
            .contains(&(flavour.to_owned(), field.to_owned()))
    }

    fn field_values_are_valid(&self, flavour: &str, field: &str) -> bool {
        !self
            .validation
            .invalid_field_values
            .contains(&(flavour.to_owned(), field.to_owned()))
    }

    fn relation_is_valid(&self, relation: &str) -> bool {
        !self.validation.relations_section_invalid
            && !self.validation.invalid_relations.contains(relation)
    }

    fn relation_source_is_valid(&self, relation: &str) -> bool {
        !self.validation.invalid_relation_sources.contains(relation)
    }

    fn relation_target_is_valid(&self, relation: &str) -> bool {
        !self.validation.invalid_relation_targets.contains(relation)
    }

    fn same_flavour_is_valid(&self, relation: &str) -> bool {
        !self.validation.invalid_same_flavour.contains(relation)
    }

    fn validation_errors(&mut self) -> Vec<String> {
        let recovered = std::mem::take(&mut self.validation);
        self.validation = SchemaValidationState {
            format_version_invalid: recovered.format_version_invalid,
            flavours_section_invalid: recovered.flavours_section_invalid,
            relations_section_invalid: recovered.relations_section_invalid,
            invalid_flavours: recovered.invalid_flavours,
            invalid_flavour_descriptions: recovered.invalid_flavour_descriptions,
            invalid_id_prefixes: recovered.invalid_id_prefixes,
            invalid_bodies: recovered.invalid_bodies,
            invalid_field_sections: recovered.invalid_field_sections,
            invalid_fields: recovered.invalid_fields,
            invalid_relations: recovered.invalid_relations,
            ..SchemaValidationState::default()
        };
        let mut errors = Vec::new();
        if !self.validation.format_version_invalid && self.format_version != 1 {
            errors.push(format!(
                "unsupported schema format version {}",
                self.format_version
            ));
        }

        for (name, flavour) in &self.flavours {
            if !is_snake_name(name) {
                errors.push(format!("invalid flavour name '{name}'"));
                self.validation.invalid_flavours.insert(name.clone());
            }
            if !self.validation.invalid_flavour_descriptions.contains(name)
                && flavour.description.trim().is_empty()
            {
                errors.push(format!("flavour '{name}' description must not be empty"));
            }
            if !self.validation.invalid_id_prefixes.contains(name)
                && !is_id_prefix(&flavour.id_prefix)
            {
                errors.push(format!(
                    "flavour '{name}' has invalid ID prefix '{}'",
                    flavour.id_prefix
                ));
                self.validation.invalid_id_prefixes.insert(name.clone());
            }

            for (field_name, field) in &flavour.fields {
                let mut field_is_valid = true;
                if is_structural_item_name(field_name) {
                    errors.push(format!(
                        "flavour '{name}' field '{field_name}' is reserved for item structure"
                    ));
                    field_is_valid = false;
                }
                if !is_snake_name(field_name) {
                    errors.push(format!(
                        "flavour '{name}' has invalid field name '{field_name}'"
                    ));
                    field_is_valid = false;
                }
                let field_errors = field.validation_errors(name, field_name);
                if !field_errors.is_empty() {
                    self.validation
                        .invalid_field_values
                        .insert((name.clone(), field_name.clone()));
                    errors.extend(field_errors);
                }
                if !field_is_valid {
                    self.validation
                        .invalid_fields
                        .insert((name.clone(), field_name.clone()));
                }
            }
        }

        for (name, relation) in &self.relations {
            if !is_snake_name(name) {
                errors.push(format!("invalid relation name '{name}'"));
                self.validation.invalid_relations.insert(name.clone());
            }
            if is_structural_item_name(name) {
                errors.push(format!("relation '{name}' is reserved for item structure"));
                self.validation.invalid_relations.insert(name.clone());
            }
            if relation.description.trim().is_empty() {
                errors.push(format!("relation '{name}' description must not be empty"));
            }
            if !self.validation.flavours_section_invalid {
                errors.extend(endpoint_errors(
                    name,
                    "source",
                    &relation.source,
                    &self.flavours,
                    &self.validation.invalid_flavours,
                ));
            }
            if self.validation.flavours_section_invalid
                || !endpoints_are_usable(
                    &relation.source,
                    &self.flavours,
                    &self.validation.invalid_flavours,
                )
            {
                self.validation
                    .invalid_relation_sources
                    .insert(name.clone());
            }
            if !self.validation.flavours_section_invalid {
                errors.extend(endpoint_errors(
                    name,
                    "target",
                    &relation.target,
                    &self.flavours,
                    &self.validation.invalid_flavours,
                ));
            }
            if self.validation.flavours_section_invalid
                || !endpoints_are_usable(
                    &relation.target,
                    &self.flavours,
                    &self.validation.invalid_flavours,
                )
            {
                self.validation
                    .invalid_relation_targets
                    .insert(name.clone());
            }
            for source in &relation.source {
                if self
                    .flavours
                    .get(source)
                    .is_some_and(|flavour| flavour.fields.contains_key(name))
                {
                    errors.push(format!(
                        "relation '{name}' conflicts with field '{name}' on source flavour '{source}'"
                    ));
                    self.validation.invalid_relations.insert(name.clone());
                }
            }
            if relation.same_flavour
                && !relation
                    .source
                    .iter()
                    .any(|source| relation.target.contains(source))
            {
                errors.push(format!(
                    "relation '{name}' requires a shared source and target flavour when same_flavour is true"
                ));
                self.validation.invalid_same_flavour.insert(name.clone());
            }
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlavourDefinition {
    description: String,
    id_prefix: String,
    body: BodyRequirement,
    #[serde(default)]
    fields: BTreeMap<String, FieldDefinition>,
}

impl FlavourDefinition {
    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn fields(&self) -> &BTreeMap<String, FieldDefinition> {
        &self.fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BodyRequirement {
    Optional,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldDefinition {
    #[serde(rename = "type")]
    field_type: FieldType,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    repeatable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    values: Option<Vec<String>>,
}

impl FieldDefinition {
    fn validation_errors(&self, flavour: &str, field: &str) -> Vec<String> {
        let mut errors = Vec::new();
        match (self.field_type, &self.values) {
            (FieldType::Enum, Some(values)) if values.is_empty() => errors.push(format!(
                "flavour '{flavour}' enum field '{field}' must declare at least one value"
            )),
            (FieldType::Enum, Some(values)) => {
                let mut unique = HashSet::new();
                let mut reported_surrounding_whitespace = false;
                for value in values {
                    if value.trim() != value && !reported_surrounding_whitespace {
                        errors.push(format!(
                            "flavour '{flavour}' enum field '{field}' values must not have surrounding whitespace"
                        ));
                        reported_surrounding_whitespace = true;
                    }
                    if value.is_empty() {
                        errors.push(format!(
                            "flavour '{flavour}' enum field '{field}' contains an empty value"
                        ));
                    }
                    if !unique.insert(value) {
                        errors.push(format!(
                            "flavour '{flavour}' enum field '{field}' contains duplicate value '{value}'"
                        ));
                    }
                }
            }
            (FieldType::Enum, None) => errors.push(format!(
                "flavour '{flavour}' enum field '{field}' must declare values"
            )),
            (_, Some(_)) => errors.push(format!(
                "flavour '{flavour}' field '{field}' may declare values only when its type is enum"
            )),
            (_, None) => {}
        }
        errors
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Integer,
    Number,
    Boolean,
    Enum,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelationDefinition {
    description: String,
    source: Vec<String>,
    target: Vec<String>,
    #[serde(default)]
    same_flavour: bool,
}

impl RelationDefinition {
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl Project {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schema_path(&self) -> &Path {
        &self.schema_path
    }

    pub fn content_patterns(&self) -> &[String] {
        &self.content_patterns
    }

    pub(crate) fn content_discovery_is_complete(&self) -> bool {
        self.content_discovery_complete
    }
}

#[derive(Debug)]
pub enum Error {
    ExistingProject {
        path: PathBuf,
    },
    WouldOverwrite {
        path: PathBuf,
    },
    ProjectNotFound {
        start: PathBuf,
    },
    InvalidProject {
        path: PathBuf,
        message: String,
    },
    InvalidSchema {
        path: PathBuf,
        message: String,
    },
    InvalidDocument {
        path: PathBuf,
        line: usize,
        message: String,
    },
    InvalidMutation {
        message: String,
    },
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExistingProject { path } => {
                write!(
                    formatter,
                    "a Mara project already exists at {}",
                    path.display()
                )
            }
            Self::WouldOverwrite { path } => write!(
                formatter,
                "refusing to overwrite existing content at {}",
                path.display()
            ),
            Self::ProjectNotFound { start } => write!(
                formatter,
                "no {PROJECT_FILE} was found from {}",
                start.display()
            ),
            Self::InvalidProject { path, message } => {
                write!(
                    formatter,
                    "invalid Mara project at {}: {message}",
                    path.display()
                )
            }
            Self::InvalidSchema { path, message } => {
                write!(
                    formatter,
                    "invalid Mara schema at {}: {message}",
                    path.display()
                )
            }
            Self::InvalidDocument {
                path,
                line,
                message,
            } => write!(
                formatter,
                "invalid Mara document at {}:{line}: {message}",
                path.display()
            ),
            Self::InvalidMutation { message } => write!(formatter, "{message}"),
            Self::Io {
                action,
                path,
                source,
            } => write!(formatter, "could not {action} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    format_version: u32,
    project: ProjectSection,
    content: ContentSection,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectSection {
    name: String,
    schema: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentSection {
    include: Vec<String>,
}

pub fn initialize_project(target: impl AsRef<Path>, template: Template) -> Result<Project, Error> {
    let target = target.as_ref();
    fs::create_dir_all(target).map_err(|source| Error::Io {
        action: "create target directory",
        path: target.to_path_buf(),
        source,
    })?;

    let root = fs::canonicalize(target).map_err(|source| Error::Io {
        action: "resolve target directory",
        path: target.to_path_buf(),
        source,
    })?;
    if !root.is_dir() {
        return Err(Error::InvalidProject {
            path: root,
            message: "the project root is not a directory".into(),
        });
    }

    let project_path = root.join(PROJECT_FILE);
    let schema_path = root.join(SCHEMA_FILE);
    if path_exists(&project_path)? {
        return Err(Error::ExistingProject { path: project_path });
    }
    if path_exists(&schema_path)? {
        return Err(Error::WouldOverwrite { path: schema_path });
    }

    let mara_directory = root.join(".mara");
    let created_mara_directory = !path_exists(&mara_directory)?;
    fs::create_dir_all(&mara_directory).map_err(|source| Error::Io {
        action: "create project directory",
        path: mara_directory.clone(),
        source,
    })?;

    if let Err(error) = write_new(&schema_path, schema_template(template)) {
        remove_empty_directory(created_mara_directory, &mara_directory);
        return Err(error);
    }

    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("mara-project");
    let project_source = match project_template(name) {
        Ok(source) => source,
        Err(error) => {
            let _ = fs::remove_file(&schema_path);
            remove_empty_directory(created_mara_directory, &mara_directory);
            return Err(error);
        }
    };
    if let Err(error) = write_new(&project_path, &project_source) {
        let _ = fs::remove_file(&schema_path);
        remove_empty_directory(created_mara_directory, &mara_directory);
        return Err(error);
    }

    match resolve_project(None, &root) {
        Ok(project) => Ok(project),
        Err(error) => {
            let _ = fs::remove_file(&project_path);
            let _ = fs::remove_file(&schema_path);
            remove_empty_directory(created_mara_directory, &mara_directory);
            Err(error)
        }
    }
}

pub fn resolve_project(
    explicit_root: Option<&Path>,
    discovery_start: impl AsRef<Path>,
) -> Result<Project, Error> {
    let root = resolve_project_root(explicit_root, discovery_start.as_ref())?;
    load_project_root(&root)
}

pub fn resolve_project_for_validation(
    explicit_root: Option<&Path>,
    discovery_start: impl AsRef<Path>,
) -> Result<ProjectValidation, Error> {
    let root = resolve_project_root(explicit_root, discovery_start.as_ref())?;
    load_project_root_for_validation(&root)
}

fn resolve_project_root(
    explicit_root: Option<&Path>,
    discovery_start: &Path,
) -> Result<PathBuf, Error> {
    if let Some(explicit_root) = explicit_root {
        let root = if explicit_root.is_absolute() {
            explicit_root.to_path_buf()
        } else {
            discovery_start.join(explicit_root)
        };
        return Ok(root);
    }

    let resolved_start = fs::canonicalize(discovery_start).map_err(|source| Error::Io {
        action: "resolve project discovery start",
        path: discovery_start.to_path_buf(),
        source,
    })?;
    let mut candidate = if resolved_start.is_file() {
        resolved_start
            .parent()
            .expect("a file path has a parent")
            .to_path_buf()
    } else {
        resolved_start.clone()
    };

    loop {
        if path_exists(&candidate.join(PROJECT_FILE))? {
            return Ok(candidate);
        }
        if !candidate.pop() {
            return Err(Error::ProjectNotFound {
                start: resolved_start,
            });
        }
    }
}

pub fn load_schema(project: &Project) -> Result<Schema, Error> {
    let (schema, errors) = load_schema_for_validation(project)?;
    if let Some(message) = errors.into_iter().next() {
        return Err(Error::InvalidSchema {
            path: project.schema_path().to_path_buf(),
            message,
        });
    }
    Ok(schema)
}

pub fn load_schema_for_validation(project: &Project) -> Result<(Schema, Vec<String>), Error> {
    let source = fs::read_to_string(project.schema_path()).map_err(|source| Error::Io {
        action: "read project schema",
        path: project.schema_path().to_path_buf(),
        source,
    })?;
    let configuration: SchemaFileForValidation =
        serde_saphyr::from_str(&source).map_err(|source| Error::InvalidSchema {
            path: project.schema_path().to_path_buf(),
            message: source.to_string(),
        })?;
    let SchemaFileForValidation {
        format_version,
        flavours,
        relations,
        unknown,
    } = configuration;
    let mut errors = unknown
        .keys()
        .map(|key| format!("unknown schema configuration key '{key}'"))
        .collect::<Vec<_>>();
    let format_version =
        decode_schema_configuration_value(format_version, "format_version", &mut errors);
    let format_version_invalid = format_version.is_none();
    let format_version = format_version.unwrap_or_default();
    let flavour_values: Option<BTreeMap<String, SchemaValue>> =
        decode_schema_configuration_value(flavours, "flavours", &mut errors);
    let flavours_section_invalid = flavour_values.is_none();
    let mut validation = SchemaValidationState {
        format_version_invalid,
        flavours_section_invalid,
        ..SchemaValidationState::default()
    };
    let mut flavours = BTreeMap::new();
    for (name, value) in flavour_values.unwrap_or_default() {
        match recover_flavour(&name, &value) {
            Ok(recovered) => {
                errors.extend(recovered.errors);
                if !recovered.description_valid {
                    validation.invalid_flavour_descriptions.insert(name.clone());
                }
                if !recovered.id_prefix_valid {
                    validation.invalid_id_prefixes.insert(name.clone());
                }
                if !recovered.body_valid {
                    validation.invalid_bodies.insert(name.clone());
                }
                if !recovered.fields_valid {
                    validation.invalid_field_sections.insert(name.clone());
                }
                validation.invalid_fields.extend(
                    recovered
                        .invalid_fields
                        .into_iter()
                        .map(|field| (name.clone(), field)),
                );
                flavours.insert(name, recovered.definition);
            }
            Err(message) => {
                errors.push(format!("flavour '{name}' is invalid: {message}"));
                validation.invalid_flavours.insert(name);
            }
        }
    }
    let relation_values: Option<BTreeMap<String, SchemaValue>> =
        decode_schema_configuration_value(relations, "relations", &mut errors);
    let relations_section_invalid = relation_values.is_none();
    validation.relations_section_invalid = relations_section_invalid;
    let mut relations = BTreeMap::new();
    for (name, value) in relation_values.unwrap_or_default() {
        match decode_schema_declaration(&value) {
            Ok(relation) => {
                relations.insert(name, relation);
            }
            Err(message) => {
                errors.push(format!("relation '{name}' is invalid: {message}"));
                validation.invalid_relations.insert(name);
            }
        }
    }
    let mut schema = Schema {
        format_version,
        flavours,
        relations,
        validation,
    };
    errors.extend(schema.validation_errors());
    Ok((schema, errors))
}

fn recover_flavour(name: &str, value: &SchemaValue) -> Result<RecoveredFlavour, String> {
    let configuration: FlavourFileForValidation = decode_schema_declaration(value)?;
    let mut errors = configuration
        .unknown
        .keys()
        .map(|key| format!("flavour '{name}' is invalid: unknown configuration key '{key}'"))
        .collect::<Vec<_>>();
    let description =
        decode_flavour_property(name, "description", configuration.description, &mut errors);
    let id_prefix =
        decode_flavour_property(name, "id_prefix", configuration.id_prefix, &mut errors);
    let body = decode_flavour_property(name, "body", configuration.body, &mut errors);
    let field_values = match configuration.fields {
        Some(value) => match decode_schema_declaration(&value) {
            Ok(fields) => Some(fields),
            Err(error) => {
                errors.push(format!(
                    "flavour '{name}' is invalid: property 'fields': {error}"
                ));
                None
            }
        },
        None => Some(BTreeMap::new()),
    };
    let fields_valid = field_values.is_some();
    let mut invalid_fields = Vec::new();
    let mut fields = BTreeMap::new();
    for (field_name, value) in field_values.unwrap_or_default() {
        match decode_schema_declaration(&value) {
            Ok(field) => {
                fields.insert(field_name, field);
            }
            Err(error) => {
                errors.push(format!(
                    "flavour '{name}' field '{field_name}' is invalid: {error}"
                ));
                invalid_fields.push(field_name);
            }
        }
    }
    Ok(RecoveredFlavour {
        definition: FlavourDefinition {
            description: description.clone().unwrap_or_default(),
            id_prefix: id_prefix.clone().unwrap_or_default(),
            body: body.unwrap_or(BodyRequirement::Optional),
            fields,
        },
        description_valid: description.is_some(),
        id_prefix_valid: id_prefix.is_some(),
        body_valid: body.is_some(),
        fields_valid,
        invalid_fields,
        errors,
    })
}

fn decode_flavour_property<T>(
    flavour: &str,
    property: &str,
    value: Option<SchemaValue>,
    errors: &mut Vec<String>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    match value {
        Some(value) => match decode_schema_declaration(&value) {
            Ok(value) => Some(value),
            Err(error) => {
                errors.push(format!(
                    "flavour '{flavour}' is invalid: property '{property}': {error}"
                ));
                None
            }
        },
        None => {
            errors.push(format!(
                "flavour '{flavour}' is invalid: property '{property}' is required"
            ));
            None
        }
    }
}

fn decode_schema_configuration_value<T>(
    value: Option<SchemaValue>,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    match value {
        Some(value) => match decode_schema_declaration(&value) {
            Ok(value) => Some(value),
            Err(error) => {
                errors.push(format!(
                    "invalid schema configuration value '{path}': {error}"
                ));
                None
            }
        },
        None => {
            errors.push(format!("schema configuration key '{path}' is required"));
            None
        }
    }
}

fn decode_schema_declaration<T>(value: &SchemaValue) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let source = serde_saphyr::to_string(value).map_err(|error| error.to_string())?;
    serde_saphyr::from_str(&source).map_err(|error| error.to_string())
}

fn load_project_root(root: &Path) -> Result<Project, Error> {
    let validation = load_project_root_for_validation(root)?;
    let (project, errors, _) = validation.into_parts();
    if let Some(message) = errors.into_iter().next() {
        return Err(Error::InvalidProject {
            path: project.root().join(PROJECT_FILE),
            message,
        });
    }
    Ok(project)
}

fn load_project_root_for_validation(root: &Path) -> Result<ProjectValidation, Error> {
    let root = fs::canonicalize(root).map_err(|source| Error::Io {
        action: "resolve project root",
        path: root.to_path_buf(),
        source,
    })?;
    if !root.is_dir() {
        return Err(Error::InvalidProject {
            path: root,
            message: "the project root is not a directory".into(),
        });
    }

    let project_path = root.join(PROJECT_FILE);
    let source = fs::read_to_string(&project_path).map_err(|source| Error::Io {
        action: "read project configuration",
        path: project_path.clone(),
        source,
    })?;
    let mut configuration: toml::Table =
        toml::from_str(&source).map_err(|source| Error::InvalidProject {
            path: project_path.clone(),
            message: source.to_string(),
        })?;

    let mut errors = Vec::new();
    let format_version: Option<u32> = take_project_value(
        &mut configuration,
        "format_version",
        "format_version",
        &mut errors,
    );
    let (name, schema): (Option<String>, Option<String>) =
        match take_project_table(&mut configuration, "project", "project", &mut errors) {
            Some(mut project) => {
                let name = take_project_value(&mut project, "name", "project.name", &mut errors);
                let schema =
                    take_project_value(&mut project, "schema", "project.schema", &mut errors);
                unknown_project_keys(&project, "project", &mut errors);
                (name, schema)
            }
            None => (None, None),
        };
    let include_values: Option<Vec<toml::Value>> =
        match take_project_table(&mut configuration, "content", "content", &mut errors) {
            Some(mut content) => {
                let include =
                    take_project_value(&mut content, "include", "content.include", &mut errors);
                unknown_project_keys(&content, "content", &mut errors);
                include
            }
            None => None,
        };
    unknown_project_keys(&configuration, "", &mut errors);

    if format_version.is_some_and(|version| version != 1) {
        errors.push(format!(
            "unsupported project format version {}",
            format_version.expect("format version is present")
        ));
    }
    if name.as_deref().is_some_and(|name| name.trim().is_empty()) {
        errors.push("project.name must not be empty".into());
    }
    let configured_schema = schema.as_deref().map(Path::new);
    let schema_is_relative = configured_schema.is_some_and(is_project_relative);
    if configured_schema.is_some() && !schema_is_relative {
        errors.push("project.schema must be a project-relative path".into());
    }
    let mut content_discovery_complete = include_values.is_some();
    let mut include: Vec<String> = Vec::new();
    for (index, value) in include_values.unwrap_or_default().into_iter().enumerate() {
        let decoded: Result<String, _> = value.try_into();
        match decoded {
            Ok(pattern) => include.push(pattern),
            Err(error) => {
                errors.push(format!(
                    "invalid project configuration value 'content.include[{index}]': {error}"
                ));
                content_discovery_complete = false;
            }
        }
    }
    let has_non_relative_content = include
        .iter()
        .any(|pattern| !is_project_relative(Path::new(pattern)));
    if has_non_relative_content {
        errors.push("content.include entries must be project-relative patterns".into());
    }

    let mut content_patterns = Vec::new();
    for pattern in include {
        let pattern_is_relative = is_project_relative(Path::new(&pattern));
        let effective_pattern = if pattern_is_relative {
            normalize_project_relative_pattern(&pattern)
        } else {
            pattern.clone()
        };
        let valid_glob = match GlobBuilder::new(&effective_pattern)
            .literal_separator(true)
            .build()
        {
            Ok(_) => true,
            Err(error) => {
                errors.push(format!(
                    "invalid content.include pattern '{pattern}': {error}"
                ));
                false
            }
        };
        if pattern_is_relative && valid_glob {
            content_patterns.push(effective_pattern);
        } else {
            content_discovery_complete = false;
        }
    }

    let schema_path =
        if let Some(configured_schema) = configured_schema.filter(|_| schema_is_relative) {
            root.join(configured_schema)
        } else {
            root.join(SCHEMA_FILE)
        };
    let schema_available = schema_is_relative && schema_path.is_file();
    if schema_is_relative && !schema_available {
        errors.push(format!(
            "schema file does not exist at {}",
            schema_path.display()
        ));
    }

    Ok(ProjectValidation {
        project: Project {
            root,
            name: name.unwrap_or_default(),
            schema_path,
            content_patterns,
            content_discovery_complete,
        },
        errors,
        schema_available,
    })
}

fn take_project_table(
    configuration: &mut toml::Table,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<toml::Table> {
    match configuration.remove(key) {
        Some(toml::Value::Table(table)) => Some(table),
        Some(value) => {
            errors.push(format!(
                "invalid project configuration value '{path}': expected a table, found {}",
                value.type_str()
            ));
            None
        }
        None => {
            errors.push(format!("project configuration key '{path}' is required"));
            None
        }
    }
}

fn take_project_value<T>(
    configuration: &mut toml::Table,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    match configuration.remove(key) {
        Some(value) => match value.try_into() {
            Ok(value) => Some(value),
            Err(error) => {
                errors.push(format!(
                    "invalid project configuration value '{path}': {error}"
                ));
                None
            }
        },
        None => {
            errors.push(format!("project configuration key '{path}' is required"));
            None
        }
    }
}

fn unknown_project_keys(configuration: &toml::Table, prefix: &str, errors: &mut Vec<String>) {
    errors.extend(configuration.keys().map(|key| {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        format!("unknown project configuration key '{path}'")
    }));
}

fn normalize_project_relative_pattern(pattern: &str) -> String {
    let mut normalized = PathBuf::new();
    for component in Path::new(pattern).components() {
        if component != Component::CurDir {
            normalized.push(component.as_os_str());
        }
    }
    normalized.to_string_lossy().into_owned()
}

fn is_project_relative(path: &Path) -> bool {
    !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn is_snake_name(name: &str) -> bool {
    let mut parts = name.split('_');
    parts.next().is_some_and(is_lower_alphanumeric_name) && parts.all(is_lower_alphanumeric_name)
}

fn is_lower_alphanumeric_name(part: &str) -> bool {
    let mut characters = part.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
        && characters.all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
}

fn is_structural_item_name(name: &str) -> bool {
    matches!(name, "mid" | "flavour" | "id" | "title" | "body")
}

fn is_id_prefix(prefix: &str) -> bool {
    let Some(stem) = prefix.strip_suffix('-') else {
        return false;
    };
    let mut segments = stem.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    let mut characters = first.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
        && characters.all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        && segments.all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        })
}

pub(crate) fn is_item_id(id: &str) -> bool {
    let mut segments = id.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    let mut characters = first.chars();
    let valid_first = characters
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
        && characters.all(|character| character.is_ascii_uppercase() || character.is_ascii_digit());
    valid_first
        && segments.clone().next().is_some()
        && segments.all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        })
}

pub(crate) fn is_mid(value: &str) -> bool {
    value
        .parse::<ulid::Ulid>()
        .is_ok_and(|mid| mid.to_string() == value)
}

fn endpoint_errors(
    relation: &str,
    endpoint: &str,
    flavours: &[String],
    declarations: &BTreeMap<String, FlavourDefinition>,
    invalid_declarations: &HashSet<String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if flavours.is_empty() {
        errors.push(format!(
            "relation '{relation}' {endpoint} must declare at least one flavour"
        ));
        return errors;
    }
    let mut unique = HashSet::new();
    for flavour in flavours {
        if !declarations.contains_key(flavour) && !invalid_declarations.contains(flavour) {
            errors.push(format!(
                "relation '{relation}' {endpoint} references unknown flavour '{flavour}'"
            ));
        }
        if !unique.insert(flavour) {
            errors.push(format!(
                "relation '{relation}' {endpoint} repeats flavour '{flavour}'"
            ));
        }
    }
    errors
}

fn endpoints_are_usable(
    flavours: &[String],
    declarations: &BTreeMap<String, FlavourDefinition>,
    invalid_flavours: &HashSet<String>,
) -> bool {
    !flavours.is_empty()
        && flavours.iter().all(|flavour| {
            declarations.contains_key(flavour) && !invalid_flavours.contains(flavour)
        })
}

fn path_exists(path: &Path) -> Result<bool, Error> {
    path.try_exists().map_err(|source| Error::Io {
        action: "inspect path",
        path: path.to_path_buf(),
        source,
    })
}

fn write_new(path: &Path, source: &str) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| Error::Io {
            action: "create project file",
            path: path.to_path_buf(),
            source,
        })?;
    if let Err(source) = file.write_all(source.as_bytes()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(Error::Io {
            action: "write project file",
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}

fn remove_empty_directory(created: bool, directory: &Path) {
    if created {
        let _ = fs::remove_dir(directory);
    }
}

fn project_template(name: &str) -> Result<String, Error> {
    toml::to_string_pretty(&ProjectFile {
        format_version: 1,
        project: ProjectSection {
            name: name.into(),
            schema: SCHEMA_FILE.into(),
        },
        content: ContentSection {
            include: vec!["**/*.mara.md".into()],
        },
    })
    .map_err(|source| Error::InvalidProject {
        path: PathBuf::from(PROJECT_FILE),
        message: source.to_string(),
    })
}

fn schema_template(template: Template) -> &'static str {
    match template {
        Template::Minimal => MINIMAL_SCHEMA,
        Template::Empty => EMPTY_SCHEMA,
    }
}

const EMPTY_SCHEMA: &str = "format_version: 1\nflavours: {}\nrelations: {}\n";

const MINIMAL_SCHEMA: &str = r#"format_version: 1
flavours:
  scenario:
    description: A concrete behavioural flow with an observable outcome.
    id_prefix: SCN-
    body: required
    fields: {}
  requirement:
    description: An independently verifiable obligation.
    id_prefix: REQ-
    body: required
    fields: {}
  design:
    description: A solution or interface contract that satisfies requirements.
    id_prefix: DES-
    body: required
    fields: {}
  decision:
    description: A consequential choice and its durable rationale.
    id_prefix: ADR-
    body: required
    fields: {}
relations:
  derives_from:
    description: The source originates from or refines the target intent.
    source: [requirement, design]
    target: [scenario, requirement]
  depends_on:
    description: The source cannot be satisfied or understood without the target.
    source: [scenario, requirement, design, decision]
    target: [scenario, requirement, design, decision]
  satisfies:
    description: The source design provides a solution contract for the target requirement.
    source: [design]
    target: [requirement]
  justifies:
    description: The source decision preserves the reasoning for the target.
    source: [decision]
    target: [requirement, design]
  supersedes:
    description: The source replaces an older target of the same flavour.
    source: [scenario, requirement, design, decision]
    target: [scenario, requirement, design, decision]
    same_flavour: true
"#;
