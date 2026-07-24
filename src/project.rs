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

/// A structured project discovery or loading failure.
#[derive(Debug)]
pub enum ProjectLoadError {
    ProjectNotFound {
        start: PathBuf,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    InvalidConfiguration {
        path: PathBuf,
        field: Option<&'static str>,
        message: String,
        location: Option<SourceLocation>,
    },
    UnsafePath {
        config_path: Box<Path>,
        field: &'static str,
        configured: Box<str>,
        reason: Box<str>,
        resolved: Option<Box<Path>>,
        location: Option<SourceLocation>,
    },
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
            operation: "resolve discovery start",
            path: requested_start.clone(),
            source,
        })?;
    let metadata = fs::metadata(&resolved_start).map_err(|source| ProjectLoadError::Io {
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
            operation: "resolve project root",
            path: requested_root.clone(),
            source,
        })?;
    let metadata = fs::metadata(&resolved_root).map_err(|source| ProjectLoadError::Io {
        operation: "inspect project root",
        path: resolved_root.clone(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(ProjectLoadError::InvalidConfiguration {
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
    let (mut config_file, resolved_config_path) = open_project_input(
        &location.root,
        &location.config_path,
        &location.config_path,
        "project configuration",
        ".mara/project.toml",
        None,
    )?;
    let mut bytes = Vec::new();
    config_file
        .read_to_end(&mut bytes)
        .map_err(|source| ProjectLoadError::Io {
            operation: "read project configuration",
            path: resolved_config_path.clone(),
            source,
        })?;
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(ProjectLoadError::InvalidConfiguration {
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
            path: location.config_path.clone(),
            field: None,
            message: "configuration is not valid UTF-8".into(),
            location: Some(source_location(&bytes, error.valid_up_to())),
        })?;
    let raw: RawProjectConfig = toml::from_str(source_text).map_err(|error: toml::de::Error| {
        ProjectLoadError::InvalidConfiguration {
            path: location.config_path.clone(),
            field: None,
            message: error.message().to_owned(),
            location: error
                .span()
                .map(|span| source_location(source_text.as_bytes(), span.start)),
        }
    })?;

    let format_location = location_for_span(source_text, &raw.format_version);
    let format_version = raw.format_version.into_inner();
    if format_version != 1 {
        return Err(invalid_value(
            &location.config_path,
            "format_version",
            format!("unsupported format version {format_version}; expected integer 1"),
            format_location,
        ));
    }

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
    let schema_path = resolve_existing_input(
        &location.root,
        &location.config_path,
        "project.schema",
        &schema_value,
        schema_location,
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
    if index_path == resolved_config_path || index_path == schema_path {
        return Err(unsafe_path(
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
) -> Result<PathBuf, ProjectLoadError> {
    validate_relative_path(config_path, field, configured, location)?;
    let logical_path = root.join(configured);
    let (_, resolved_path) = open_project_input(
        root,
        config_path,
        &logical_path,
        field,
        configured,
        location,
    )?;
    Ok(resolved_path)
}

fn open_project_input(
    root: &Path,
    config_path: &Path,
    logical_path: &Path,
    field: &'static str,
    configured: &str,
    location: Option<SourceLocation>,
) -> Result<(fs::File, PathBuf), ProjectLoadError> {
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
    let file = open_read_no_follow(&resolved_path).map_err(|source| ProjectLoadError::Io {
        operation: "open project input",
        path: resolved_path.clone(),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| ProjectLoadError::Io {
        operation: "inspect opened project input",
        path: resolved_path.clone(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(unsafe_path(
            config_path,
            field,
            configured,
            "resolved input is not a regular file",
            Some(resolved_path),
            location,
        ));
    }
    Ok((file, resolved_path))
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
    let resolved_ancestor =
        fs::canonicalize(&existing_ancestor).map_err(|source| ProjectLoadError::Io {
            operation: "resolve output ancestor",
            path: existing_ancestor,
            source,
        })?;
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
            operation: "inspect output ancestor",
            path: resolved_ancestor.clone(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(unsafe_path(
                config_path,
                field,
                configured,
                "nearest existing output ancestor is not a directory",
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
            operation: "inspect output destination",
            path: resolved_target.clone(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(unsafe_path(
                config_path,
                field,
                configured,
                "existing output destination is not a regular file",
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
    let reason = if configured.is_empty() {
        Some("path must not be empty")
    } else if configured.contains('\0') {
        Some("path must not contain NUL")
    } else if configured.contains('\\') {
        Some("path must use `/`, not backslashes")
    } else if Path::new(configured).is_absolute() || is_windows_drive_path(configured) {
        Some("path must be project-relative and have no drive prefix")
    } else if has_uri_scheme(configured) {
        Some("path must not contain a URI scheme")
    } else if configured
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        Some("path must not contain empty, `.` or `..` segments")
    } else {
        None
    };

    match reason {
        Some(reason) => Err(unsafe_path(
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
        path: path.to_path_buf(),
        field: Some(field),
        message,
        location,
    }
}

fn unsafe_path(
    config_path: &Path,
    field: &'static str,
    configured: &str,
    reason: impl Into<String>,
    resolved: Option<PathBuf>,
    location: Option<SourceLocation>,
) -> ProjectLoadError {
    ProjectLoadError::UnsafePath {
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
fn open_read_no_follow(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
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

#[cfg(not(any(unix, windows)))]
fn open_read_no_follow(path: &Path) -> io::Result<fs::File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other("refusing to follow an input symlink"));
    }
    fs::File::open(path)
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
