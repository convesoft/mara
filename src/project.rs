//! Deterministic discovery and strict loading of Mara project configuration.

use std::{
    collections::HashSet,
    fmt, fs, io,
    io::Read,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use toml::Spanned;

pub const PROJECT_DIRECTORY: &str = ".mara";
pub const PROJECT_CONFIG_FILE: &str = "project.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    volume: u64,
    file: u64,
}

/// The discovered location of one Mara project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLocation {
    /// Canonical absolute path of the directory containing `.mara`.
    pub root: PathBuf,
    /// Absolute logical path of the discovered `.mara/project.toml` marker.
    pub config_path: PathBuf,
}

/// A project whose v1 configuration and configured paths have been validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedProject {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub format_version: u32,
    pub name: String,
    /// Canonical path of the existing schema input.
    pub schema_path: PathBuf,
    pub content: ContentConfig,
    /// Canonicalized destination path, including a normalized absent suffix.
    pub index_path: PathBuf,
    pub validation: ValidationConfig,
    pub git: GitConfig,
}

/// Strict v1 content-discovery settings.
///
/// This loader validates the settings but does not discover content files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub respect_gitignore: bool,
    pub follow_directory_symlinks: bool,
    pub allow_internal_file_symlinks: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationConfig {
    pub warnings_as_errors: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitConfig {
    pub require_clean_worktree_for_writes: bool,
}

/// One-based source coordinates within `.mara/project.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub byte_offset: usize,
    pub line: usize,
    pub column: usize,
}

/// Stable Mara v1 diagnostic catalogue code for a loading failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectLoadErrorCode {
    ProjectNotFound,
    ProjectPathOutsideRoot,
    ProjectSymlinkRejected,
    ProjectDuplicateFile,
    ConfigIo,
    ConfigSyntax,
    ConfigDuplicateKey,
    ConfigUnknownKey,
    ConfigInvalidValue,
    SchemaIo,
}

impl ProjectLoadErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProjectNotFound => "project.not_found",
            Self::ProjectPathOutsideRoot => "project.path_outside_root",
            Self::ProjectSymlinkRejected => "project.symlink_rejected",
            Self::ProjectDuplicateFile => "project.duplicate_file",
            Self::ConfigIo => "config.io",
            Self::ConfigSyntax => "config.syntax",
            Self::ConfigDuplicateKey => "config.duplicate_key",
            Self::ConfigUnknownKey => "config.unknown_key",
            Self::ConfigInvalidValue => "config.invalid_value",
            Self::SchemaIo => "schema.io",
        }
    }
}

/// Stable Mara v1 command-level operational code for a loading failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectLoadOperationalErrorCode {
    ProjectUnavailable,
    IoFailed,
}

impl ProjectLoadOperationalErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProjectUnavailable => "project.unavailable",
            Self::IoFailed => "io.failed",
        }
    }
}

/// Identifies whether a failure belongs to the diagnostic or operational wire domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectLoadErrorClass {
    Diagnostic(ProjectLoadErrorCode),
    Operational(ProjectLoadOperationalErrorCode),
}

impl ProjectLoadErrorClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Diagnostic(code) => code.as_str(),
            Self::Operational(code) => code.as_str(),
        }
    }
}

/// A structured project discovery or loading failure.
#[derive(Debug)]
pub enum ProjectLoadError {
    ProjectNotFound {
        start: PathBuf,
    },
    Io {
        class: ProjectLoadErrorClass,
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    InvalidConfiguration {
        class: ProjectLoadErrorClass,
        path: PathBuf,
        field: Option<&'static str>,
        message: String,
        location: Option<SourceLocation>,
    },
    UnsafePath {
        class: ProjectLoadErrorClass,
        config_path: Box<Path>,
        field: &'static str,
        configured: Box<str>,
        reason: Box<str>,
        resolved: Option<Box<Path>>,
        location: Option<SourceLocation>,
    },
}

impl ProjectLoadError {
    /// Returns the stable wire-domain classification without parsing display text.
    pub const fn class(&self) -> ProjectLoadErrorClass {
        match self {
            Self::ProjectNotFound { .. } => {
                ProjectLoadErrorClass::Diagnostic(ProjectLoadErrorCode::ProjectNotFound)
            }
            Self::Io { class, .. }
            | Self::InvalidConfiguration { class, .. }
            | Self::UnsafePath { class, .. } => *class,
        }
    }

    pub const fn diagnostic_code(&self) -> Option<ProjectLoadErrorCode> {
        match self.class() {
            ProjectLoadErrorClass::Diagnostic(code) => Some(code),
            ProjectLoadErrorClass::Operational(_) => None,
        }
    }

    pub const fn operational_code(&self) -> Option<ProjectLoadOperationalErrorCode> {
        match self.class() {
            ProjectLoadErrorClass::Diagnostic(_) => None,
            ProjectLoadErrorClass::Operational(code) => Some(code),
        }
    }
}

impl fmt::Display for ProjectLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectNotFound { start } => write!(
                formatter,
                "no {PROJECT_DIRECTORY}/{PROJECT_CONFIG_FILE} found from {}",
                start.display()
            ),
            Self::Io {
                operation,
                path,
                source,
                ..
            } => write!(
                formatter,
                "could not {operation} {}: {source}",
                path.display()
            ),
            Self::InvalidConfiguration {
                path,
                field,
                message,
                location,
                ..
            } => {
                write!(
                    formatter,
                    "invalid project configuration {}",
                    path.display()
                )?;
                if let Some(location) = location {
                    write!(formatter, ":{}:{}", location.line, location.column)?;
                }
                if let Some(field) = field {
                    write!(formatter, " ({field})")?;
                }
                write!(formatter, ": {message}")
            }
            Self::UnsafePath {
                config_path,
                field,
                configured,
                reason,
                resolved,
                location,
                ..
            } => {
                write!(formatter, "unsafe path in {}", config_path.display())?;
                if let Some(location) = location {
                    write!(formatter, ":{}:{}", location.line, location.column)?;
                }
                write!(formatter, " ({field} = {configured:?}): {reason}")?;
                if let Some(resolved) = resolved {
                    write!(formatter, " ({})", resolved.display())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ProjectLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Finds the nearest ancestor containing `.mara/project.toml`.
///
/// `start` must exist and may name either a directory or a regular file. The
/// search uses its canonical location so results do not depend on the caller's
/// current working directory or a symlinked spelling of the starting path.
pub fn discover_project(start: impl AsRef<Path>) -> Result<ProjectLocation, ProjectLoadError> {
    let requested_start = start.as_ref().to_path_buf();
    let resolved_start =
        fs::canonicalize(&requested_start).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
            operation: "resolve discovery start",
            path: requested_start.clone(),
            source,
        })?;
    let metadata = fs::metadata(&resolved_start).map_err(|source| ProjectLoadError::Io {
        class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
        operation: "inspect discovery start",
        path: resolved_start.clone(),
        source,
    })?;
    let mut current = if metadata.is_file() {
        resolved_start
            .parent()
            .expect("a canonical file path has a parent")
            .to_path_buf()
    } else if metadata.is_dir() {
        resolved_start
    } else {
        return Err(ProjectLoadError::InvalidConfiguration {
            class: ProjectLoadErrorClass::Operational(
                ProjectLoadOperationalErrorCode::ProjectUnavailable,
            ),
            path: requested_start,
            field: None,
            message: "project discovery must start from a directory or regular file".into(),
            location: None,
        });
    };

    loop {
        let config_path = current.join(PROJECT_DIRECTORY).join(PROJECT_CONFIG_FILE);
        match fs::symlink_metadata(&config_path) {
            Ok(_) => {
                return Ok(ProjectLocation {
                    root: current,
                    config_path,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ProjectLoadError::Io {
                    class: ProjectLoadErrorClass::Diagnostic(ProjectLoadErrorCode::ConfigIo),
                    operation: "inspect project marker",
                    path: config_path,
                    source,
                });
            }
        }

        if !current.pop() {
            return Err(ProjectLoadError::ProjectNotFound {
                start: requested_start,
            });
        }
    }
}

/// Discovers and loads the project containing `start`.
pub fn discover_and_load(start: impl AsRef<Path>) -> Result<LoadedProject, ProjectLoadError> {
    let location = discover_project(start)?;
    load_location(location)
}

/// Loads `.mara/project.toml` from an explicit project root.
pub fn load_from_root(root: impl AsRef<Path>) -> Result<LoadedProject, ProjectLoadError> {
    let requested_root = root.as_ref().to_path_buf();
    let resolved_root =
        fs::canonicalize(&requested_root).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
            operation: "resolve project root",
            path: requested_root.clone(),
            source,
        })?;
    let metadata = fs::metadata(&resolved_root).map_err(|source| ProjectLoadError::Io {
        class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
        operation: "inspect project root",
        path: resolved_root.clone(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(ProjectLoadError::InvalidConfiguration {
            class: ProjectLoadErrorClass::Operational(
                ProjectLoadOperationalErrorCode::ProjectUnavailable,
            ),
            path: requested_root,
            field: None,
            message: "explicit project root is not a directory".into(),
            location: None,
        });
    }
    let config_path = resolved_root
        .join(PROJECT_DIRECTORY)
        .join(PROJECT_CONFIG_FILE);
    load_location(ProjectLocation {
        root: resolved_root,
        config_path,
    })
}

fn load_location(location: ProjectLocation) -> Result<LoadedProject, ProjectLoadError> {
    let (mut config_file, resolved_config_path, config_identity) = open_project_input(
        &location.root,
        &location.config_path,
        &location.config_path,
        "project configuration",
        ".mara/project.toml",
        None,
        ProjectLoadErrorCode::ConfigIo,
    )?;
    let mut bytes = Vec::new();
    config_file
        .read_to_end(&mut bytes)
        .map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Diagnostic(ProjectLoadErrorCode::ConfigIo),
            operation: "read project configuration",
            path: resolved_config_path.clone(),
            source,
        })?;
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(ProjectLoadError::InvalidConfiguration {
            class: ProjectLoadErrorClass::Diagnostic(ProjectLoadErrorCode::ConfigSyntax),
            path: location.config_path,
            field: None,
            message: "UTF-8 byte-order marks are not permitted".into(),
            location: Some(SourceLocation {
                byte_offset: 0,
                line: 1,
                column: 1,
            }),
        });
    }
    let source_text =
        std::str::from_utf8(&bytes).map_err(|error| ProjectLoadError::InvalidConfiguration {
            class: ProjectLoadErrorClass::Diagnostic(ProjectLoadErrorCode::ConfigSyntax),
            path: location.config_path.clone(),
            field: None,
            message: "configuration is not valid UTF-8".into(),
            location: Some(source_location(&bytes, error.valid_up_to())),
        })?;
    // Parse the document independently of the v1 shape first, so syntax and
    // duplicate-key failures cannot be conflated with schema-value failures.
    toml::from_str::<toml::Value>(source_text).map_err(|error: toml::de::Error| {
        let code = if error.message().contains("duplicate key") {
            ProjectLoadErrorCode::ConfigDuplicateKey
        } else {
            ProjectLoadErrorCode::ConfigSyntax
        };
        configuration_decode_error(&location.config_path, source_text, error, code)
    })?;
    let version: FormatVersionProbe =
        toml::from_str(source_text).map_err(|error: toml::de::Error| {
            configuration_decode_error(
                &location.config_path,
                source_text,
                error,
                ProjectLoadErrorCode::ConfigInvalidValue,
            )
        })?;
    let format_location = location_for_span(source_text, &version.format_version);
    let format_version = version.format_version.into_inner();
    if format_version != 1 {
        return Err(invalid_value(
            &location.config_path,
            "format_version",
            format!("unsupported format version {format_version}; expected integer 1"),
            format_location,
        ));
    }

    let raw: RawProjectConfig = toml::from_str(source_text).map_err(|error: toml::de::Error| {
        let code = if error.message().starts_with("unknown field ") {
            ProjectLoadErrorCode::ConfigUnknownKey
        } else {
            ProjectLoadErrorCode::ConfigInvalidValue
        };
        configuration_decode_error(&location.config_path, source_text, error, code)
    })?;
    debug_assert_eq!(raw.format_version.into_inner(), 1);

    let name_location = location_for_span(source_text, &raw.project.name);
    let name = raw.project.name.into_inner();
    if !valid_project_name(&name) {
        return Err(invalid_value(
            &location.config_path,
            "project.name",
            format!("{name:?} does not match [a-z][a-z0-9]*(?:-[a-z0-9]+)*"),
            name_location,
        ));
    }

    let include = validate_glob_sequence(
        source_text,
        &location.config_path,
        "content.include",
        raw.content.include,
        true,
    )?;
    let exclude = validate_glob_sequence(
        source_text,
        &location.config_path,
        "content.exclude",
        raw.content.exclude,
        false,
    )?;

    let schema_location = location_for_span(source_text, &raw.project.schema);
    let schema_value = raw.project.schema.into_inner();
    let (schema_path, schema_identity) = resolve_existing_input(
        &location.root,
        &location.config_path,
        "project.schema",
        &schema_value,
        schema_location,
        ProjectLoadErrorCode::SchemaIo,
    )?;

    let index_location = location_for_span(source_text, &raw.index.path);
    let index_value = raw.index.path.into_inner();
    let index_path = resolve_output_path(
        &location.root,
        &location.config_path,
        "index.path",
        &index_value,
        index_location,
    )?;
    let index_aliases_input = existing_file_identity(&index_path)?.is_some_and(|index_identity| {
        config_identity == Some(index_identity) || schema_identity == Some(index_identity)
    });
    if index_path == resolved_config_path || index_path == schema_path || index_aliases_input {
        return Err(unsafe_path(
            ProjectLoadErrorCode::ProjectDuplicateFile,
            &location.config_path,
            "index.path",
            &index_value,
            "index destination resolves to a project input",
            Some(index_path),
            index_location,
        ));
    }

    Ok(LoadedProject {
        root: location.root,
        config_path: location.config_path,
        format_version: 1,
        name,
        schema_path,
        content: ContentConfig {
            include,
            exclude,
            respect_gitignore: raw.content.respect_gitignore,
            follow_directory_symlinks: raw.content.follow_directory_symlinks,
            allow_internal_file_symlinks: raw.content.allow_internal_file_symlinks,
        },
        index_path,
        validation: ValidationConfig {
            warnings_as_errors: raw.validation.warnings_as_errors,
        },
        git: GitConfig {
            require_clean_worktree_for_writes: raw.git.require_clean_worktree_for_writes,
        },
    })
}

fn resolve_existing_input(
    root: &Path,
    config_path: &Path,
    field: &'static str,
    configured: &str,
    location: Option<SourceLocation>,
    io_code: ProjectLoadErrorCode,
) -> Result<(PathBuf, Option<FileIdentity>), ProjectLoadError> {
    validate_relative_path(config_path, field, configured, location)?;
    let logical_path = root.join(configured);
    let (_, resolved_path, identity) = open_project_input(
        root,
        config_path,
        &logical_path,
        field,
        configured,
        location,
        io_code,
    )?;
    Ok((resolved_path, identity))
}

fn open_project_input(
    root: &Path,
    config_path: &Path,
    logical_path: &Path,
    field: &'static str,
    configured: &str,
    location: Option<SourceLocation>,
    io_code: ProjectLoadErrorCode,
) -> Result<(fs::File, PathBuf, Option<FileIdentity>), ProjectLoadError> {
    let resolved_path = match fs::canonicalize(logical_path) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound && location.is_some() => {
            return Err(invalid_value(
                config_path,
                field,
                format!("configured input {configured:?} does not exist"),
                location,
            ));
        }
        Err(source) => {
            return Err(ProjectLoadError::Io {
                class: ProjectLoadErrorClass::Diagnostic(io_code),
                operation: "resolve project input",
                path: logical_path.to_path_buf(),
                source,
            });
        }
    };
    ensure_contained(
        root,
        config_path,
        field,
        configured,
        &resolved_path,
        location,
    )?;
    let metadata = fs::metadata(&resolved_path).map_err(|source| ProjectLoadError::Io {
        class: ProjectLoadErrorClass::Diagnostic(io_code),
        operation: "inspect project input before opening",
        path: resolved_path.clone(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(unsafe_path(
            ProjectLoadErrorCode::ConfigInvalidValue,
            config_path,
            field,
            configured,
            "resolved input is not a regular file",
            Some(resolved_path),
            location,
        ));
    }
    let file = open_read_no_follow(&resolved_path).map_err(|source| ProjectLoadError::Io {
        class: ProjectLoadErrorClass::Diagnostic(io_code),
        operation: "open project input",
        path: resolved_path.clone(),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| ProjectLoadError::Io {
        class: ProjectLoadErrorClass::Diagnostic(io_code),
        operation: "inspect opened project input",
        path: resolved_path.clone(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(unsafe_path(
            ProjectLoadErrorCode::ConfigInvalidValue,
            config_path,
            field,
            configured,
            "resolved input is not a regular file",
            Some(resolved_path),
            location,
        ));
    }
    verify_opened_input(
        root,
        config_path,
        field,
        configured,
        &resolved_path,
        &file,
        location,
        io_code,
    )?;
    let identity = file_identity(&file).map_err(|source| ProjectLoadError::Io {
        class: ProjectLoadErrorClass::Diagnostic(io_code),
        operation: "read opened project input identity",
        path: resolved_path.clone(),
        source,
    })?;
    Ok((file, resolved_path, identity))
}

fn existing_file_identity(path: &Path) -> Result<Option<FileIdentity>, ProjectLoadError> {
    match fs::metadata(path) {
        Ok(metadata) => path_identity(path, &metadata).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
            operation: "read existing output metadata identity",
            path: path.to_path_buf(),
            source,
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
            operation: "inspect existing output identity",
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_opened_input(
    root: &Path,
    config_path: &Path,
    field: &'static str,
    configured: &str,
    expected_path: &Path,
    opened_file: &fs::File,
    location: Option<SourceLocation>,
    io_code: ProjectLoadErrorCode,
) -> Result<(), ProjectLoadError> {
    let rechecked_path =
        fs::canonicalize(expected_path).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Diagnostic(io_code),
            operation: "recheck opened project input",
            path: expected_path.to_path_buf(),
            source,
        })?;
    ensure_contained(
        root,
        config_path,
        field,
        configured,
        &rechecked_path,
        location,
    )?;
    let rechecked_file =
        open_read_no_follow(&rechecked_path).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Diagnostic(io_code),
            operation: "reopen project input for identity check",
            path: rechecked_path.clone(),
            source,
        })?;
    let opened_identity = file_identity(opened_file).map_err(|source| ProjectLoadError::Io {
        class: ProjectLoadErrorClass::Diagnostic(io_code),
        operation: "read opened project input identity",
        path: expected_path.to_path_buf(),
        source,
    })?;
    let rechecked_identity =
        file_identity(&rechecked_file).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Diagnostic(io_code),
            operation: "read rechecked project input identity",
            path: rechecked_path.clone(),
            source,
        })?;
    if rechecked_path != expected_path
        || matches!((opened_identity, rechecked_identity), (Some(left), Some(right)) if left != right)
    {
        return Err(unsafe_path(
            ProjectLoadErrorCode::ProjectSymlinkRejected,
            config_path,
            field,
            configured,
            "opened input changed during containment verification",
            Some(rechecked_path),
            location,
        ));
    }
    Ok(())
}

fn resolve_output_path(
    root: &Path,
    config_path: &Path,
    field: &'static str,
    configured: &str,
    location: Option<SourceLocation>,
) -> Result<PathBuf, ProjectLoadError> {
    validate_relative_path(config_path, field, configured, location)?;
    let target = root.join(configured);
    let (existing_ancestor, suffix) = nearest_existing_ancestor(&target)?;
    let resolved_ancestor = match fs::canonicalize(&existing_ancestor) {
        Ok(path) => path,
        Err(_)
            if fs::symlink_metadata(&existing_ancestor)
                .is_ok_and(|metadata| metadata.file_type().is_symlink()) =>
        {
            return Err(unsafe_path(
                ProjectLoadErrorCode::ProjectSymlinkRejected,
                config_path,
                field,
                configured,
                "output path contains a dangling symlink",
                Some(existing_ancestor),
                location,
            ));
        }
        Err(source) => {
            return Err(ProjectLoadError::Io {
                class: ProjectLoadErrorClass::Operational(
                    ProjectLoadOperationalErrorCode::IoFailed,
                ),
                operation: "resolve output ancestor",
                path: existing_ancestor,
                source,
            });
        }
    };
    ensure_contained(
        root,
        config_path,
        field,
        configured,
        &resolved_ancestor,
        location,
    )?;
    if !suffix.is_empty() {
        let metadata = fs::metadata(&resolved_ancestor).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
            operation: "inspect output ancestor",
            path: resolved_ancestor.clone(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(unsafe_path(
                ProjectLoadErrorCode::ConfigInvalidValue,
                config_path,
                field,
                configured,
                "nearest existing output ancestor is not a directory",
                Some(resolved_ancestor),
                location,
            ));
        }
        if check_output_write_access(&resolved_ancestor, true).is_err() {
            return Err(unsafe_path(
                ProjectLoadErrorCode::ConfigInvalidValue,
                config_path,
                field,
                configured,
                "nearest existing output ancestor is not writable",
                Some(resolved_ancestor),
                location,
            ));
        }
    }
    let resolved_target = suffix
        .iter()
        .fold(resolved_ancestor, |path, segment| path.join(segment));
    ensure_contained(
        root,
        config_path,
        field,
        configured,
        &resolved_target,
        location,
    )?;
    if suffix.is_empty() {
        let metadata = fs::metadata(&resolved_target).map_err(|source| ProjectLoadError::Io {
            class: ProjectLoadErrorClass::Operational(ProjectLoadOperationalErrorCode::IoFailed),
            operation: "inspect output destination",
            path: resolved_target.clone(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(unsafe_path(
                ProjectLoadErrorCode::ConfigInvalidValue,
                config_path,
                field,
                configured,
                "existing output destination is not a regular file",
                Some(resolved_target),
                location,
            ));
        }
        if check_output_write_access(&resolved_target, false).is_err() {
            return Err(unsafe_path(
                ProjectLoadErrorCode::ConfigInvalidValue,
                config_path,
                field,
                configured,
                "existing output destination is not writable",
                Some(resolved_target),
                location,
            ));
        }
    }
    Ok(resolved_target)
}

fn nearest_existing_ancestor(path: &Path) -> Result<(PathBuf, Vec<PathBuf>), ProjectLoadError> {
    let mut current = path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let segment = current.file_name().ok_or_else(|| ProjectLoadError::Io {
                    class: ProjectLoadErrorClass::Operational(
                        ProjectLoadOperationalErrorCode::IoFailed,
                    ),
                    operation: "find existing output ancestor",
                    path: path.to_path_buf(),
                    source: io::Error::new(
                        io::ErrorKind::NotFound,
                        "path has no existing ancestor",
                    ),
                })?;
                suffix.push(PathBuf::from(segment));
                current.pop();
            }
            Err(source) => {
                return Err(ProjectLoadError::Io {
                    class: ProjectLoadErrorClass::Operational(
                        ProjectLoadOperationalErrorCode::IoFailed,
                    ),
                    operation: "inspect output ancestor",
                    path: current,
                    source,
                });
            }
        }
    }
    suffix.reverse();
    Ok((current, suffix))
}

fn validate_relative_path(
    config_path: &Path,
    field: &'static str,
    configured: &str,
    location: Option<SourceLocation>,
) -> Result<(), ProjectLoadError> {
    let rejection = if configured.is_empty() {
        Some((
            ProjectLoadErrorCode::ConfigInvalidValue,
            "path must not be empty",
        ))
    } else if configured.contains('\0') {
        Some((
            ProjectLoadErrorCode::ConfigInvalidValue,
            "path must not contain NUL",
        ))
    } else if configured.contains('\\') {
        Some((
            ProjectLoadErrorCode::ConfigInvalidValue,
            "path must use `/`, not backslashes",
        ))
    } else if Path::new(configured).is_absolute() || is_windows_drive_path(configured) {
        Some((
            ProjectLoadErrorCode::ProjectPathOutsideRoot,
            "path must be project-relative and have no drive prefix",
        ))
    } else if has_uri_scheme(configured) {
        Some((
            ProjectLoadErrorCode::ConfigInvalidValue,
            "path must not contain a URI scheme",
        ))
    } else if configured.split('/').any(|segment| segment == "..") {
        Some((
            ProjectLoadErrorCode::ProjectPathOutsideRoot,
            "path must not contain `..` segments",
        ))
    } else if configured
        .split('/')
        .any(|segment| segment.is_empty() || segment == ".")
    {
        Some((
            ProjectLoadErrorCode::ConfigInvalidValue,
            "path must not contain empty or `.` segments",
        ))
    } else {
        None
    };

    match rejection {
        Some((code, reason)) => Err(unsafe_path(
            code,
            config_path,
            field,
            configured,
            reason,
            None,
            location,
        )),
        None => Ok(()),
    }
}

fn ensure_contained(
    root: &Path,
    config_path: &Path,
    field: &'static str,
    configured: &str,
    resolved: &Path,
    location: Option<SourceLocation>,
) -> Result<(), ProjectLoadError> {
    if resolved.starts_with(root) {
        Ok(())
    } else {
        Err(unsafe_path(
            ProjectLoadErrorCode::ProjectPathOutsideRoot,
            config_path,
            field,
            configured,
            "resolved path is outside the project root",
            Some(resolved.to_path_buf()),
            location,
        ))
    }
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((prefix, _)) = value.split_once(':') else {
        return false;
    };
    let mut characters = prefix.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn is_windows_drive_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn valid_project_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_lowercase) {
        return false;
    }
    let mut previous_hyphen = false;
    for byte in &bytes[1..] {
        if *byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_hyphen = false;
        } else {
            return false;
        }
    }
    !previous_hyphen
}

fn validate_glob_sequence(
    source_text: &str,
    config_path: &Path,
    field: &'static str,
    globs: Spanned<Vec<Spanned<String>>>,
    require_non_empty: bool,
) -> Result<Vec<String>, ProjectLoadError> {
    let sequence_location = location_for_span(source_text, &globs);
    let globs = globs.into_inner();
    if require_non_empty && globs.is_empty() {
        return Err(invalid_value(
            config_path,
            field,
            "sequence must contain at least one glob".into(),
            sequence_location,
        ));
    }
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(globs.len());
    for glob in globs {
        let location = location_for_span(source_text, &glob);
        let value = glob.into_inner();
        if !seen.insert(value.clone()) {
            return Err(invalid_value(
                config_path,
                field,
                format!("duplicate glob {value:?}"),
                location,
            ));
        }
        validate_glob(&value).map_err(|reason| {
            invalid_value(
                config_path,
                field,
                format!("invalid glob {value:?}: {reason}"),
                location,
            )
        })?;
        validated.push(value);
    }
    Ok(validated)
}

fn validate_glob(glob: &str) -> Result<(), &'static str> {
    if glob.is_empty() {
        return Err("glob must not be empty");
    }
    if glob.contains('\\') {
        return Err("backslash escaping and platform separators are unsupported");
    }
    if glob.contains('{') || glob.contains('}') {
        return Err("brace expansion is unsupported");
    }
    if glob.split('/').any(str::is_empty) {
        return Err("glob must not contain an empty path segment");
    }
    if glob
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err("glob must not contain `.` or `..` path segments");
    }
    for segment in glob.split('/') {
        if segment.contains("**") && segment != "**" {
            return Err("`**` must occupy a complete path segment");
        }
        validate_glob_segment(segment)?;
    }
    Ok(())
}

fn validate_glob_segment(segment: &str) -> Result<(), &'static str> {
    let characters: Vec<char> = segment.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        match characters[index] {
            '[' => {
                let close_offset = characters[index + 1..]
                    .iter()
                    .position(|character| *character == ']')
                    .ok_or("glob contains an unclosed character class")?;
                let close = index + 1 + close_offset;
                let mut content = &characters[index + 1..close];
                if content.first() == Some(&'!') {
                    content = &content[1..];
                }
                if content.is_empty() {
                    return Err("glob contains an empty character class");
                }
                for (content_index, character) in content.iter().enumerate() {
                    if *character == '-' {
                        if content_index == 0 || content_index + 1 == content.len() {
                            return Err("character-class ranges require two endpoints");
                        }
                        if content[content_index - 1] > content[content_index + 1] {
                            return Err("character-class range is reversed");
                        }
                    }
                }
                index = close + 1;
            }
            ']' => return Err("glob contains `]` without an opening character class"),
            _ => index += 1,
        }
    }
    Ok(())
}

fn invalid_value(
    path: &Path,
    field: &'static str,
    message: String,
    location: Option<SourceLocation>,
) -> ProjectLoadError {
    ProjectLoadError::InvalidConfiguration {
        class: ProjectLoadErrorClass::Diagnostic(ProjectLoadErrorCode::ConfigInvalidValue),
        path: path.to_path_buf(),
        field: Some(field),
        message,
        location,
    }
}

fn configuration_decode_error(
    path: &Path,
    source_text: &str,
    error: toml::de::Error,
    code: ProjectLoadErrorCode,
) -> ProjectLoadError {
    ProjectLoadError::InvalidConfiguration {
        class: ProjectLoadErrorClass::Diagnostic(code),
        path: path.to_path_buf(),
        field: None,
        message: error.message().to_owned(),
        location: error
            .span()
            .map(|span| source_location(source_text.as_bytes(), span.start)),
    }
}

fn unsafe_path(
    code: ProjectLoadErrorCode,
    config_path: &Path,
    field: &'static str,
    configured: &str,
    reason: impl Into<String>,
    resolved: Option<PathBuf>,
    location: Option<SourceLocation>,
) -> ProjectLoadError {
    ProjectLoadError::UnsafePath {
        class: ProjectLoadErrorClass::Diagnostic(code),
        config_path: config_path.into(),
        field,
        configured: configured.into(),
        reason: reason.into().into_boxed_str(),
        resolved: resolved.map(PathBuf::into_boxed_path),
        location,
    }
}

fn location_for_span<T>(source: &str, value: &Spanned<T>) -> Option<SourceLocation> {
    Some(source_location(source.as_bytes(), value.span().start))
}

fn source_location(source: &[u8], offset: usize) -> SourceLocation {
    let offset = offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
    let line_start = prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position + 1);
    let column = std::str::from_utf8(&source[line_start..offset])
        .map_or(offset - line_start, |line| line.chars().count())
        + 1;
    SourceLocation {
        byte_offset: offset,
        line,
        column,
    }
}

#[cfg(unix)]
fn check_output_write_access(path: &Path, directory: bool) -> io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let mode = libc::W_OK | if directory { libc::X_OK } else { 0 };
    // SAFETY: `path` is NUL-terminated and remains valid for the call.
    let result = unsafe { libc::faccessat(libc::AT_FDCWD, path.as_ptr(), mode, libc::AT_EACCESS) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn check_output_write_access(path: &Path, directory: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, FILE_ADD_FILE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_DATA, OPEN_EXISTING,
        },
    };

    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let access = if directory {
        FILE_ADD_FILE
    } else {
        FILE_WRITE_DATA
    };
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        };
    // SAFETY: `path` is NUL-terminated, the optional pointer arguments are
    // null, and the returned handle is closed before returning.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `handle` was returned by `CreateFileW` and is still owned here.
    if unsafe { CloseHandle(handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn check_output_write_access(path: &Path, _directory: bool) -> io::Result<()> {
    if fs::metadata(path)?.permissions().readonly() {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "output location is read-only",
        ))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn open_read_no_follow(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(unix)]
fn file_identity(file: &fs::File) -> io::Result<Option<FileIdentity>> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    Ok(Some(FileIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    }))
}

#[cfg(unix)]
fn path_identity(_path: &Path, metadata: &fs::Metadata) -> io::Result<Option<FileIdentity>> {
    use std::os::unix::fs::MetadataExt;

    Ok(Some(FileIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    }))
}

#[cfg(windows)]
fn open_read_no_follow(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(windows)]
fn file_identity(file: &fs::File) -> io::Result<Option<FileIdentity>> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` owns a valid handle and `information` is writable for the
    // duration of the call.
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut information) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Some(FileIdentity {
        volume: u64::from(information.dwVolumeSerialNumber),
        file: (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    }))
}

#[cfg(windows)]
fn path_identity(path: &Path, _metadata: &fs::Metadata) -> io::Result<Option<FileIdentity>> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    file_identity(&file)
}

#[cfg(not(any(unix, windows)))]
fn open_read_no_follow(path: &Path) -> io::Result<fs::File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other("refusing to follow an input symlink"));
    }
    fs::File::open(path)
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_file: &fs::File) -> io::Result<Option<FileIdentity>> {
    Ok(None)
}

#[cfg(not(any(unix, windows)))]
fn path_identity(_path: &Path, _metadata: &fs::Metadata) -> io::Result<Option<FileIdentity>> {
    Ok(None)
}

#[derive(Debug, Deserialize)]
struct FormatVersionProbe {
    format_version: Spanned<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjectConfig {
    format_version: Spanned<i64>,
    project: RawProject,
    content: RawContent,
    index: RawIndex,
    validation: RawValidation,
    git: RawGit,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProject {
    name: Spanned<String>,
    schema: Spanned<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContent {
    include: Spanned<Vec<Spanned<String>>>,
    exclude: Spanned<Vec<Spanned<String>>>,
    respect_gitignore: bool,
    follow_directory_symlinks: bool,
    allow_internal_file_symlinks: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIndex {
    path: Spanned<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawValidation {
    warnings_as_errors: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGit {
    require_clean_worktree_for_writes: bool,
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_open_handle_that_differs_from_the_rechecked_contained_path() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("project");
        fs::create_dir_all(root.join(".mara")).unwrap();
        let inside = root.join(".mara/schema.yaml");
        let outside = fixture.path().join("outside-schema.yaml");
        fs::write(&inside, "inside").unwrap();
        fs::write(&outside, "outside").unwrap();
        let opened_outside = fs::File::open(&outside).unwrap();
        let expected_path = inside.canonicalize().unwrap();

        let error = verify_opened_input(
            &root.canonicalize().unwrap(),
            &root.join(".mara/project.toml"),
            "project.schema",
            ".mara/schema.yaml",
            &expected_path,
            &opened_outside,
            None,
            ProjectLoadErrorCode::SchemaIo,
        )
        .unwrap_err();

        assert_eq!(
            error.diagnostic_code(),
            Some(ProjectLoadErrorCode::ProjectSymlinkRejected)
        );
        assert!(matches!(error, ProjectLoadError::UnsafePath { .. }));
    }
}
