//! Bounded support for Mara integration tests that need an on-disk project.
//!
//! `ProjectSandbox` is intentionally not a general fixture, temporary-directory,
//! or command-running framework. It creates one canonical disposable project,
//! configures a test-owned child process, and cleans the project up.

use std::{
    env,
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// The only supported starting states for a [`ProjectSandbox`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSandboxMode {
    /// An empty project directory with no Mara or Git configuration.
    Empty,
    /// A valid, otherwise empty Mara project.
    Configured,
    /// A configured Mara project committed to a new, clean Git repository.
    CleanGit,
    /// A configured Git project with one uncommitted sandbox marker.
    DirtyGit,
}

/// Failure while creating or configuring a [`ProjectSandbox`].
#[derive(Debug)]
pub struct ProjectSandboxError {
    operation: &'static str,
    path: Option<PathBuf>,
    source: io::Error,
    cleanup: Option<ProjectSandboxCleanupError>,
}

impl ProjectSandboxError {
    fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self {
            operation,
            path: Some(path.into()),
            source,
            cleanup: None,
        }
    }

    fn command(operation: &'static str, source: io::Error) -> Self {
        Self {
            operation,
            path: None,
            source,
            cleanup: None,
        }
    }

    fn with_cleanup(mut self, cleanup: ProjectSandboxCleanupError) -> Self {
        self.cleanup = Some(cleanup);
        self
    }

    /// Returns the cleanup failure that also occurred, if sandbox initialization failed.
    pub fn cleanup_error(&self) -> Option<&ProjectSandboxCleanupError> {
        self.cleanup.as_ref()
    }
}

impl fmt::Display for ProjectSandboxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "could not {}", self.operation)?;
        if let Some(path) = &self.path {
            write!(formatter, " at {}", path.display())?;
        }
        write!(formatter, ": {}", self.source)?;
        if let Some(cleanup) = &self.cleanup {
            write!(formatter, "; cleanup also failed: {cleanup}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProjectSandboxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// A reported failure to delete a sandbox that remains available for diagnosis.
#[derive(Debug)]
pub struct ProjectSandboxCleanupError {
    retained_path: PathBuf,
    source: io::Error,
}

impl ProjectSandboxCleanupError {
    /// The canonical sandbox path that could not be deleted.
    pub fn retained_path(&self) -> &Path {
        &self.retained_path
    }
}

impl fmt::Display for ProjectSandboxCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not delete ProjectSandbox at {}: {}",
            self.retained_path.display(),
            self.source
        )
    }
}

impl std::error::Error for ProjectSandboxCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// A canonical, disposable project for one project-oriented integration test.
///
/// The sandbox is always created below the canonical system temporary-directory
/// parent, after proving that its root is outside the source checkout and every
/// registered Git worktree. Its default drop cleanup fails a successful test
/// when deletion fails. Call [`Self::cleanup`] when the test must assert cleanup
/// directly.
pub struct ProjectSandbox {
    root: PathBuf,
    parent: PathBuf,
    active: bool,
    preserve_on_failure: bool,
}

impl ProjectSandbox {
    /// Creates one sandbox in the requested supported mode.
    pub fn new(mode: ProjectSandboxMode) -> Result<Self, ProjectSandboxError> {
        let source_checkout = workspace_root()?;
        let worktrees = worktree_paths(&source_checkout)?;
        let parent = canonical_temp_parent()?;
        let temporary = tempfile::Builder::new()
            .prefix("mara-project-sandbox-")
            .tempdir_in(&parent)
            .map_err(|source| ProjectSandboxError::io("create sandbox", &parent, source))?;
        let raw_root = temporary.path().to_path_buf();

        if fs::symlink_metadata(&raw_root)
            .map_err(|source| ProjectSandboxError::io("inspect sandbox", &raw_root, source))?
            .file_type()
            .is_symlink()
        {
            let _ = fs::remove_dir_all(&raw_root);
            return Err(ProjectSandboxError::io(
                "create sandbox with a non-symlink final component",
                raw_root,
                io::Error::other("sandbox final component is a symlink"),
            ));
        }

        let root = raw_root
            .canonicalize()
            .map_err(|source| ProjectSandboxError::io("canonicalize sandbox", &raw_root, source))?;
        let _ = temporary.keep();

        if root.starts_with(&source_checkout)
            || worktrees.iter().any(|worktree| root.starts_with(worktree))
        {
            let error = ProjectSandboxError::io(
                "create sandbox outside the source checkout and every worktree",
                &root,
                io::Error::other("sandbox is inside a source checkout or worktree"),
            );
            return match remove_sandbox(&root) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(error.with_cleanup(cleanup)),
            };
        }

        let mut sandbox = Self {
            root,
            parent,
            active: true,
            preserve_on_failure: false,
        };
        if let Err(error) = sandbox.initialize(mode) {
            return Err(complete_initialization_cleanup(error, sandbox.cleanup()));
        }
        Ok(sandbox)
    }

    /// Returns the canonical project directory.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Configures a test-owned child to execute from this project with isolated
    /// inherited Git state.
    pub fn configure_command<'command>(
        &self,
        command: &'command mut Command,
    ) -> &'command mut Command {
        clear_git_environment(command);
        let explicit_git_variables: Vec<OsString> = command
            .get_envs()
            .filter(|(name, _)| is_git_variable(name))
            .map(|(name, _)| name.to_os_string())
            .collect();
        for name in explicit_git_variables {
            command.env_remove(name);
        }
        command
            .current_dir(&self.root)
            .env("GIT_CEILING_DIRECTORIES", &self.parent)
    }

    /// Retains this sandbox only when its owning test unwinds with a panic.
    pub fn preserve_on_failure(mut self) -> Self {
        self.preserve_on_failure = true;
        self
    }

    /// Deletes this sandbox now, returning the canonical retained path on error.
    pub fn cleanup(mut self) -> Result<(), ProjectSandboxCleanupError> {
        self.active = false;
        remove_sandbox(&self.root)
    }

    fn initialize(&mut self, mode: ProjectSandboxMode) -> Result<(), ProjectSandboxError> {
        match mode {
            ProjectSandboxMode::Empty => Ok(()),
            ProjectSandboxMode::Configured => self.configure_project(),
            ProjectSandboxMode::CleanGit => {
                self.configure_project()?;
                self.initialize_git()?;
                Ok(())
            }
            ProjectSandboxMode::DirtyGit => {
                self.configure_project()?;
                self.initialize_git()?;
                fs::write(self.root.join(".project-sandbox-dirty"), "dirty\n").map_err(|source| {
                    ProjectSandboxError::io("create dirty Git marker", self.root.clone(), source)
                })
            }
        }
    }

    fn configure_project(&self) -> Result<(), ProjectSandboxError> {
        let mara_directory = self.root.join(".mara");
        fs::create_dir(&mara_directory).map_err(|source| {
            ProjectSandboxError::io(
                "create Mara configuration directory",
                mara_directory,
                source,
            )
        })?;
        fs::write(self.root.join(".mara/project.toml"), initial_project_toml()).map_err(
            |source| {
                ProjectSandboxError::io("write project configuration", self.root.clone(), source)
            },
        )?;
        fs::write(self.root.join(".mara/schema.yaml"), initial_schema_yaml()).map_err(|source| {
            ProjectSandboxError::io("write project schema", self.root.clone(), source)
        })
    }

    fn initialize_git(&self) -> Result<(), ProjectSandboxError> {
        for arguments in [
            &["init", "--quiet"][..],
            &["config", "user.email", "mara-test@example.invalid"],
            &["config", "user.name", "Mara ProjectSandbox"],
            &["config", "commit.gpgSign", "false"],
            &["add", "."],
            &["commit", "--quiet", "-m", "test: initialize ProjectSandbox"],
        ] {
            let mut command = Command::new("git");
            self.configure_command(&mut command).args(arguments);
            let output = command
                .output()
                .map_err(|source| ProjectSandboxError::command("run Git", source))?;
            if !output.status.success() {
                return Err(ProjectSandboxError::command(
                    "initialize Git sandbox",
                    io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
                ));
            }
        }
        Ok(())
    }
}

impl Drop for ProjectSandbox {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        if self.preserve_on_failure && std::thread::panicking() {
            eprintln!("preserved failed ProjectSandbox at {}", self.root.display());
            return;
        }
        if let Err(error) = remove_sandbox(&self.root) {
            report_default_cleanup_failure(error);
        }
    }
}

fn canonical_temp_parent() -> Result<PathBuf, ProjectSandboxError> {
    let mut candidates = vec![env::temp_dir()];
    #[cfg(unix)]
    candidates.push(PathBuf::from("/var/tmp"));

    for candidate in candidates {
        let parent = match candidate.canonicalize() {
            Ok(parent) => parent,
            Err(_) => continue,
        };
        if !has_git_marker_ancestor(&parent) {
            return Ok(parent);
        }
    }

    Err(ProjectSandboxError::io(
        "find a sandbox parent outside Git discovery",
        env::temp_dir(),
        io::Error::other("every available sandbox parent is inside a Git repository"),
    ))
}

fn workspace_root() -> Result<PathBuf, ProjectSandboxError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|source| {
            ProjectSandboxError::io(
                "canonicalize source checkout",
                env!("CARGO_MANIFEST_DIR"),
                source,
            )
        })
}

fn worktree_paths(source_checkout: &Path) -> Result<Vec<PathBuf>, ProjectSandboxError> {
    let mut command = Command::new("git");
    clear_git_environment(&mut command);
    let output = command
        .current_dir(source_checkout)
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()
        .map_err(|source| ProjectSandboxError::command("list Git worktrees", source))?;
    if !output.status.success() {
        return Err(ProjectSandboxError::command(
            "list Git worktrees",
            io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
        ));
    }

    output
        .stdout
        .split(|byte| *byte == b'\0')
        .filter_map(|record| record.strip_prefix(b"worktree "))
        .map(path_from_git_record)
        .map(|path| {
            path.canonicalize().map_err(|source| {
                ProjectSandboxError::io("canonicalize Git worktree", path, source)
            })
        })
        .collect()
}

fn path_from_git_record(record: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(OsString::from_vec(record.to_vec()))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(record).into_owned())
    }
}

fn clear_git_environment(command: &mut Command) {
    clear_git_environment_from(command, env::vars_os());
}

fn clear_git_environment_from(
    command: &mut Command,
    variables: impl IntoIterator<Item = (OsString, OsString)>,
) {
    for (name, _) in variables {
        if is_git_variable(&name) {
            command.env_remove(name);
        }
    }
}

fn is_git_variable(name: &OsStr) -> bool {
    name.to_string_lossy()
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
}

fn has_git_marker_ancestor(path: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| ancestor.join(".git").symlink_metadata().is_ok())
}

fn remove_sandbox(root: &Path) -> Result<(), ProjectSandboxCleanupError> {
    cleanup_with(root, |path| fs::remove_dir_all(path))
}

fn complete_initialization_cleanup(
    error: ProjectSandboxError,
    cleanup: Result<(), ProjectSandboxCleanupError>,
) -> ProjectSandboxError {
    match cleanup {
        Ok(()) => error,
        Err(cleanup) => error.with_cleanup(cleanup),
    }
}

fn cleanup_with(
    root: &Path,
    remove: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), ProjectSandboxCleanupError> {
    remove(root).map_err(|source| ProjectSandboxCleanupError {
        retained_path: root.to_path_buf(),
        source,
    })
}

fn report_default_cleanup_failure(error: ProjectSandboxCleanupError) {
    if std::thread::panicking() {
        eprintln!("{error}");
    } else {
        panic!("{error}");
    }
}

fn initial_project_toml() -> &'static str {
    "format_version = 1\n[project]\nname = \"project-sandbox\"\nschema = \".mara/schema.yaml\"\n[content]\ninclude = [\"**/*.mara.md\"]\nexclude = []\nrespect_gitignore = true\nfollow_directory_symlinks = false\nallow_internal_file_symlinks = true\n[index]\npath = \".mara/index.json\"\n[validation]\nwarnings_as_errors = false\n[git]\nrequire_clean_worktree_for_writes = true\n"
}

fn initial_schema_yaml() -> &'static str {
    "format_version: 1\nschema:\n  name: project-sandbox\n  version: 0.1.0\nidentity:\n  mid:\n    format: ulid\n    prefix: m_\nflavours: {}\nrelations: {}\nrules: []\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn git_status(sandbox: &ProjectSandbox) -> String {
        let mut command = Command::new("git");
        sandbox
            .configure_command(&mut command)
            .args(["status", "--porcelain"]);
        let output = command.output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn modes_create_exactly_the_supported_initial_project_states() {
        let empty = ProjectSandbox::new(ProjectSandboxMode::Empty).unwrap();
        assert!(fs::read_dir(empty.path()).unwrap().next().is_none());

        let configured = ProjectSandbox::new(ProjectSandboxMode::Configured).unwrap();
        assert!(configured.path().join(".mara/project.toml").is_file());
        assert!(configured.path().join(".mara/schema.yaml").is_file());
        assert!(!configured.path().join(".git").exists());
        assert!(!configured.path().join("Cargo.toml").exists());
        assert!(!configured.path().join("target").exists());

        let clean = ProjectSandbox::new(ProjectSandboxMode::CleanGit).unwrap();
        assert_eq!(git_status(&clean), "");
        let mut command = Command::new("git");
        clean
            .configure_command(&mut command)
            .args(["config", "--get", "commit.gpgSign"]);
        let output = command.output().unwrap();
        assert_eq!(output.stdout, b"false\n");

        let dirty = ProjectSandbox::new(ProjectSandboxMode::DirtyGit).unwrap();
        assert_eq!(git_status(&dirty), "?? .project-sandbox-dirty\n");
    }

    #[test]
    fn sandbox_path_is_canonical_and_outside_checkout_and_worktrees() {
        let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty).unwrap();
        let source_checkout = workspace_root().unwrap();
        assert_eq!(sandbox.path(), sandbox.path().canonicalize().unwrap());
        assert!(!sandbox.path().starts_with(&source_checkout));
        for worktree in worktree_paths(&source_checkout).unwrap() {
            assert!(!sandbox.path().starts_with(worktree));
        }
    }

    #[test]
    fn configured_children_use_the_project_and_isolate_git_environment() {
        let sandbox = ProjectSandbox::new(ProjectSandboxMode::CleanGit).unwrap();
        let mut command = Command::new("git");
        command
            .env("GIT_DIR", "must-be-cleared")
            .env("GIT_PROJECT_SANDBOX_TEST", "must-be-cleared");
        sandbox.configure_command(&mut command);
        let environments: BTreeMap<_, _> = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
            .collect();
        assert_eq!(
            environments.get(OsStr::new("GIT_DIR")),
            Some(&None),
            "explicit Git variables are cleared"
        );
        assert_eq!(
            environments.get(OsStr::new("GIT_PROJECT_SANDBOX_TEST")),
            Some(&None),
            "every Git-prefixed variable is cleared"
        );
        assert_eq!(
            environments.get(OsStr::new("GIT_CEILING_DIRECTORIES")),
            Some(&Some(sandbox.parent.clone().into_os_string()))
        );

        let output = command
            .args(["rev-parse", "--show-prefix"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"\n");
    }

    #[test]
    fn inherited_git_variables_are_cleared_before_child_configuration() {
        let mut command = Command::new("git");
        clear_git_environment_from(
            &mut command,
            [(
                OsString::from("git_project_sandbox_inherited"),
                OsString::from("must-be-cleared"),
            )],
        );
        let environments: BTreeMap<_, _> = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
            .collect();
        assert_eq!(
            environments.get(OsStr::new("git_project_sandbox_inherited")),
            Some(&None)
        );
    }

    #[test]
    fn explicit_cleanup_deletes_the_sandbox() {
        let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty).unwrap();
        let root = sandbox.path().to_path_buf();
        sandbox.cleanup().unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn cleanup_error_reports_the_canonical_retained_path() {
        let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty).unwrap();
        let root = sandbox.path().to_path_buf();
        let error = cleanup_with(&root, |_| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected deletion failure",
            ))
        })
        .unwrap_err();
        assert_eq!(error.retained_path(), root);
        assert!(root.exists(), "failed cleanup keeps the sandbox available");
        assert!(error.to_string().contains("injected deletion failure"));
    }

    #[test]
    fn initialization_cleanup_failures_remain_observable() {
        let cleanup = ProjectSandboxCleanupError {
            retained_path: PathBuf::from("/canonical/retained-sandbox"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "injected cleanup failure"),
        };
        let error = complete_initialization_cleanup(
            ProjectSandboxError::command(
                "initialize sandbox",
                io::Error::other("injected initialization failure"),
            ),
            Err(cleanup),
        );
        assert_eq!(
            error.cleanup_error().unwrap().retained_path(),
            Path::new("/canonical/retained-sandbox")
        );
        assert!(error.to_string().contains("injected cleanup failure"));
    }

    #[test]
    fn default_cleanup_failures_fail_successful_tests() {
        let result = std::panic::catch_unwind(|| {
            report_default_cleanup_failure(ProjectSandboxCleanupError {
                retained_path: PathBuf::from("/canonical/retained-sandbox"),
                source: io::Error::new(io::ErrorKind::PermissionDenied, "injected cleanup failure"),
            });
        });
        assert!(result.is_err());
    }

    #[test]
    fn preserve_on_failure_retains_only_an_unwinding_sandbox() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let result = std::thread::spawn(move || {
            let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty)
                .unwrap()
                .preserve_on_failure();
            sender.send(sandbox.path().to_path_buf()).unwrap();
            panic!("exercise failure-only preservation");
        })
        .join();
        assert!(result.is_err());
        let retained = receiver.recv().unwrap();
        assert!(retained.exists());
        fs::remove_dir_all(retained).unwrap();

        let removed = {
            let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty)
                .unwrap()
                .preserve_on_failure();
            sandbox.path().to_path_buf()
        };
        assert!(!removed.exists());
    }
}
