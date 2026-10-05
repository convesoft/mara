use crate::*;
use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};
// @mara implements REQ-PROJECT-INITIALIZATION
// @mara implements REQ-ENGINEERING-TEMPLATE
pub fn initialize_project(target: impl AsRef<Path>, template: Template) -> Result<Project, Error> {
    initialize_project_with_changes(target, template).map(|(project, _)| project)
}

pub(crate) fn initialize_project_with_changes(
    target: impl AsRef<Path>,
    template: Template,
) -> Result<(Project, Vec<PathBuf>), Error> {
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
    if path_exists(&project_path)? {
        return Err(Error::ExistingProject { path: project_path });
    }

    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("mara-project");
    // Inspect every template destination before planning, including non-UTF-8 files.
    let engine = SourceEngine::new(SourceSnapshot {
        format_version: 1,
        files: Vec::new(),
        selection: SelectionInputs {
            document_paths: Vec::new(),
            ignores: Vec::new(),
        },
        base_identity: None,
    })?;
    let plan = engine.plan_initialize(name, template)?;
    for change in plan.changes() {
        let path = root.join(&change.path);
        if path_exists(&path)? {
            return Err(Error::WouldOverwrite { path });
        }
    }

    let mara_directory = root.join(".mara");
    let created_mara_directory = !path_exists(&mara_directory)?;
    fs::create_dir_all(&mara_directory).map_err(|source| Error::Io {
        action: "create project directory",
        path: mara_directory.clone(),
        source,
    })?;

    let mut created = Vec::new();
    for change in plan.changes() {
        let path = root.join(&change.path);
        let source = change
            .after
            .as_deref()
            .expect("initialization adds source files");
        if let Err(error) = write_new(&path, source) {
            for path in &created {
                let _ = fs::remove_file(path);
            }
            remove_empty_directory(created_mara_directory, &mara_directory);
            return Err(error);
        }
        created.push(path);
    }

    match resolve_project(None, &root) {
        Ok(project) => {
            let mut paths: Vec<PathBuf> = plan
                .changes()
                .iter()
                .map(|change| change.path.clone().into())
                .collect();
            paths.sort();
            Ok((project, paths))
        }
        Err(error) => {
            for path in &created {
                let _ = fs::remove_file(path);
            }
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

// @mara implements REQ-PROJECT-DISCOVERY
pub(crate) fn resolve_project_root(
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
    if let Some(error) = errors.first() {
        return Err(Error::InvalidSchema {
            path: project.schema_path().to_owned(),
            message: error.message.clone(),
        });
    }
    Ok(schema)
}
pub fn load_schema_for_validation(
    project: &Project,
) -> Result<(Schema, Vec<ConfigurationDiagnostic>), Error> {
    let source = fs::read_to_string(project.schema_path()).map_err(|source| Error::Io {
        action: "read project schema",
        path: project.schema_path().to_owned(),
        source,
    })?;
    crate::runtime::sessions::record(project.schema_path().to_owned());
    let key = crate::runtime::sessions::key("schema", source.as_bytes());
    crate::runtime::sessions::cached(key, || mara_engine::parse_schema_for_validation(&source))
}
fn load_project_root(root: &Path) -> Result<Project, Error> {
    let validation = load_project_root_for_validation(root)?;
    let (project, errors, _) = validation.into_parts();
    if let Some(message) = errors.into_iter().next() {
        return Err(Error::InvalidProject {
            path: project.root().join(PROJECT_FILE),
            message: message.message,
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
    crate::runtime::sessions::record(project_path.clone());
    let source = fs::read_to_string(&project_path).map_err(|source| Error::Io {
        action: "read project configuration",
        path: project_path.clone(),
        source,
    })?;
    let key = crate::runtime::sessions::key(
        "project",
        &serde_json::to_vec(&(root.as_os_str().as_encoded_bytes(), &source))
            .expect("project input bytes serialize"),
    );
    let value = crate::runtime::sessions::cached(key, || {
        mara_engine::parse_project_for_validation(root, &source, |path| path.is_file())
    })?;
    let (project, _, _) = value.clone().into_parts();
    crate::runtime::sessions::record(project.schema_path().to_owned());
    for path in &project.rule_files {
        crate::runtime::sessions::record(project.root().join(path));
    }
    for language in project.code_languages() {
        for path in [&language.grammar, &language.query].into_iter().flatten() {
            crate::runtime::sessions::record(project.root().join(path));
        }
    }
    Ok(value)
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
