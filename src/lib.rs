use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

mod corpus;

pub use corpus::{
    Corpus, Diagnostic, Document, Item, Mention, MetadataEntry, Relation, SourceLocation,
    SourceSpan, load_corpus, load_corpus_for_validation, load_corpus_syntax_for_validation,
    validate_corpus,
};

pub const PROJECT_FILE: &str = ".mara/project.toml";
pub const SCHEMA_FILE: &str = ".mara/schema.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Minimal,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    root: PathBuf,
    name: String,
    schema_path: PathBuf,
    content_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    format_version: u32,
    flavours: BTreeMap<String, FlavourDefinition>,
    relations: BTreeMap<String, RelationDefinition>,
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

    fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.format_version != 1 {
            errors.push(format!(
                "unsupported schema format version {}",
                self.format_version
            ));
        }

        for (name, flavour) in &self.flavours {
            if !is_snake_name(name) {
                errors.push(format!("invalid flavour name '{name}'"));
            }
            if flavour.description.trim().is_empty() {
                errors.push(format!("flavour '{name}' description must not be empty"));
            }
            if !is_id_prefix(&flavour.id_prefix) {
                errors.push(format!(
                    "flavour '{name}' has invalid ID prefix '{}'",
                    flavour.id_prefix
                ));
            }

            for (field_name, field) in &flavour.fields {
                if is_structural_item_name(field_name) {
                    errors.push(format!(
                        "flavour '{name}' field '{field_name}' is reserved for item structure"
                    ));
                }
                if !is_snake_name(field_name) {
                    errors.push(format!(
                        "flavour '{name}' has invalid field name '{field_name}'"
                    ));
                }
                errors.extend(field.validation_errors(name, field_name));
            }
        }

        for (name, relation) in &self.relations {
            if !is_snake_name(name) {
                errors.push(format!("invalid relation name '{name}'"));
            }
            if is_structural_item_name(name) {
                errors.push(format!("relation '{name}' is reserved for item structure"));
            }
            if relation.description.trim().is_empty() {
                errors.push(format!("relation '{name}' description must not be empty"));
            }
            errors.extend(endpoint_errors(
                name,
                "source",
                &relation.source,
                &self.flavours,
            ));
            errors.extend(endpoint_errors(
                name,
                "target",
                &relation.target,
                &self.flavours,
            ));
            for source in &relation.source {
                if self
                    .flavours
                    .get(source)
                    .is_some_and(|flavour| flavour.fields.contains_key(name))
                {
                    errors.push(format!(
                        "relation '{name}' conflicts with field '{name}' on source flavour '{source}'"
                    ));
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
            }
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BodyRequirement {
    Optional,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Integer,
    Number,
    Boolean,
    Enum,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    let discovery_start = discovery_start.as_ref();
    if let Some(explicit_root) = explicit_root {
        let root = if explicit_root.is_absolute() {
            explicit_root.to_path_buf()
        } else {
            discovery_start.join(explicit_root)
        };
        return load_project_root(&root);
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
            return load_project_root(&candidate);
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
    let schema: Schema =
        serde_saphyr::from_str(&source).map_err(|source| Error::InvalidSchema {
            path: project.schema_path().to_path_buf(),
            message: source.to_string(),
        })?;
    let errors = schema.validation_errors();
    Ok((schema, errors))
}

fn load_project_root(root: &Path) -> Result<Project, Error> {
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
    let configuration: ProjectFile =
        toml::from_str(&source).map_err(|source| Error::InvalidProject {
            path: project_path.clone(),
            message: source.to_string(),
        })?;

    if configuration.format_version != 1 {
        return Err(Error::InvalidProject {
            path: project_path,
            message: format!(
                "unsupported project format version {}",
                configuration.format_version
            ),
        });
    }
    if configuration.project.name.trim().is_empty() {
        return Err(Error::InvalidProject {
            path: project_path,
            message: "project.name must not be empty".into(),
        });
    }
    let configured_schema = Path::new(&configuration.project.schema);
    if !is_project_relative(configured_schema) {
        return Err(Error::InvalidProject {
            path: project_path,
            message: "project.schema must be a project-relative path".into(),
        });
    }
    if configuration
        .content
        .include
        .iter()
        .any(|pattern| !is_project_relative(Path::new(pattern)))
    {
        return Err(Error::InvalidProject {
            path: project_path,
            message: "content.include entries must be project-relative patterns".into(),
        });
    }

    let schema_path = root.join(configured_schema);
    if !schema_path.is_file() {
        return Err(Error::InvalidProject {
            path: project_path,
            message: format!("schema file does not exist at {}", schema_path.display()),
        });
    }

    Ok(Project {
        root,
        name: configuration.project.name,
        schema_path,
        content_patterns: configuration.content.include,
    })
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

fn is_item_id(id: &str) -> bool {
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

fn endpoint_errors(
    relation: &str,
    endpoint: &str,
    flavours: &[String],
    declarations: &BTreeMap<String, FlavourDefinition>,
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
        if !declarations.contains_key(flavour) {
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
