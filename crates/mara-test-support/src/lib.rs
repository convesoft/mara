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
    process::{Command, ExitCode, Termination},
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
}

/// An opted-in sandbox that can preserve a failed project-oriented test.
///
/// Return [`Self::finish`] from a `#[test]` to let Rust's ordinary test harness
/// observe the final test result before this sandbox is cleaned or retained.
pub struct FailurePreservingProjectSandbox {
    sandbox: ProjectSandbox,
}

/// The final result of a project-oriented test that opted into preservation.
///
/// This is deliberately only a [`Termination`] value, not a test runner: the
/// standard Rust test harness still runs the test and reports its outcome.
pub struct ProjectSandboxTestResult<E> {
    sandbox: ProjectSandbox,
    result: Result<(), E>,
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

        let metadata = match fs::symlink_metadata(&raw_root) {
            Ok(metadata) => metadata,
            Err(source) => {
                return Err(cleanup_temporary_initialization_failure(
                    temporary,
                    ProjectSandboxError::io("inspect sandbox", &raw_root, source),
                ));
            }
        };
        if metadata.file_type().is_symlink() {
            let error = ProjectSandboxError::io(
                "create sandbox with a non-symlink final component",
                &raw_root,
                io::Error::other("sandbox final component is a symlink"),
            );
            return Err(cleanup_temporary_initialization_failure(temporary, error));
        }

        let root = match raw_root.canonicalize() {
            Ok(root) => root,
            Err(source) => {
                let error = ProjectSandboxError::io("canonicalize sandbox", &raw_root, source);
                return Err(cleanup_temporary_initialization_failure(temporary, error));
            }
        };
        if let Err(error) = confirm_sandbox_root(&raw_root, &root) {
            return Err(cleanup_temporary_initialization_failure(temporary, error));
        }
        if let Err(error) = confirm_sandbox_parent(&root, &parent) {
            return Err(cleanup_temporary_initialization_failure(temporary, error));
        }

        if root.starts_with(&source_checkout)
            || worktrees.iter().any(|worktree| root.starts_with(worktree))
        {
            let error = ProjectSandboxError::io(
                "create sandbox outside the source checkout and every worktree",
                &root,
                io::Error::other("sandbox is inside a source checkout or worktree"),
            );
            return Err(cleanup_temporary_initialization_failure(temporary, error));
        }
        let _ = temporary.keep();

        let mut sandbox = Self {
            root,
            parent,
            active: true,
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

    /// Opts this sandbox into retention only when its owning test finally fails.
    ///
    /// Return [`FailurePreservingProjectSandbox::finish`] from the test so the
    /// standard test harness can distinguish a reported failure from a caught
    /// panic that the test ultimately handles successfully.
    pub fn preserve_on_failure(self) -> FailurePreservingProjectSandbox {
        FailurePreservingProjectSandbox { sandbox: self }
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
        let template_directory = self.root.join(".git-template");
        fs::create_dir(&template_directory).map_err(|source| {
            ProjectSandboxError::io(
                "create empty Git template directory",
                &template_directory,
                source,
            )
        })?;
        let mut command = Command::new("git");
        self.configure_command(&mut command)
            .args(["init", "--quiet", "--initial-branch", "main", "--template"])
            .arg(&template_directory);
        run_git_command(command)?;
        fs::remove_dir(&template_directory).map_err(|source| {
            ProjectSandboxError::io(
                "remove empty Git template directory",
                &template_directory,
                source,
            )
        })?;

        let git_directory = self.root.join(".git");
        let hooks_directory = git_directory.join("hooks");
        let excludes_directory = git_directory.join("info");
        let excludes_file = excludes_directory.join("exclude");
        fs::create_dir_all(&hooks_directory).map_err(|source| {
            ProjectSandboxError::io(
                "create isolated Git hooks directory",
                &hooks_directory,
                source,
            )
        })?;
        fs::create_dir_all(&excludes_directory).map_err(|source| {
            ProjectSandboxError::io(
                "create isolated Git excludes directory",
                &excludes_directory,
                source,
            )
        })?;
        fs::write(&excludes_file, "").map_err(|source| {
            ProjectSandboxError::io("create isolated Git excludes file", &excludes_file, source)
        })?;

        for (name, value) in [
            ("core.hooksPath", hooks_directory.as_os_str()),
            ("core.excludesFile", excludes_file.as_os_str()),
            ("user.email", OsStr::new("mara-test@example.invalid")),
            ("user.name", OsStr::new("Mara ProjectSandbox")),
            ("commit.gpgSign", OsStr::new("false")),
        ] {
            let mut command = Command::new("git");
            self.configure_command(&mut command)
                .args(["config", "--local", name])
                .arg(value);
            run_git_command(command)?;
        }
        for arguments in [
            &["add", "."][..],
            &["commit", "--quiet", "-m", "test: initialize ProjectSandbox"],
        ] {
            let mut command = Command::new("git");
            self.configure_command(&mut command).args(arguments);
            run_git_command(command)?;
        }
        Ok(())
    }
}

impl FailurePreservingProjectSandbox {
    /// Returns the canonical project directory.
    pub fn path(&self) -> &Path {
        self.sandbox.path()
    }

    /// Configures a test-owned child to execute from this project with isolated
    /// inherited Git state.
    pub fn configure_command<'command>(
        &self,
        command: &'command mut Command,
    ) -> &'command mut Command {
        self.sandbox.configure_command(command)
    }

    /// Deletes this sandbox now, returning the canonical retained path on error.
    pub fn cleanup(self) -> Result<(), ProjectSandboxCleanupError> {
        self.sandbox.cleanup()
    }

    /// Completes the opted-in sandbox as this test's final result.
    pub fn finish<E>(self, result: Result<(), E>) -> ProjectSandboxTestResult<E> {
        ProjectSandboxTestResult {
            sandbox: self.sandbox,
            result,
        }
    }
}

impl<E: fmt::Debug> Termination for ProjectSandboxTestResult<E> {
    fn report(mut self) -> ExitCode {
        match self.result {
            Ok(()) => match self.sandbox.cleanup() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            },
            Err(error) => {
                self.sandbox.active = false;
                eprintln!(
                    "preserved failed ProjectSandbox at {}",
                    self.sandbox.root.display()
                );
                <Result<(), E> as Termination>::report(Err(error))
            }
        }
    }
}

impl Drop for ProjectSandbox {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        if let Err(error) = remove_sandbox(&self.root) {
            report_default_cleanup_failure(error);
        }
    }
}

fn canonical_temp_parent() -> Result<PathBuf, ProjectSandboxError> {
    let mut candidates = vec![env::temp_dir()];
    #[cfg(unix)]
    candidates.push(PathBuf::from("/var/tmp"));

    canonical_temp_parent_from(candidates)
}

fn canonical_temp_parent_from(
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Result<PathBuf, ProjectSandboxError> {
    for candidate in candidates {
        let candidate: PathBuf = candidate.components().collect();
        let metadata = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
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

fn confirm_sandbox_root(raw_root: &Path, root: &Path) -> Result<(), ProjectSandboxError> {
    let metadata = fs::symlink_metadata(raw_root)
        .map_err(|source| ProjectSandboxError::io("reinspect sandbox", raw_root, source))?;
    if metadata.file_type().is_symlink() {
        return Err(ProjectSandboxError::io(
            "confirm sandbox with a non-symlink final component",
            raw_root,
            io::Error::other("sandbox final component changed to a symlink"),
        ));
    }
    let confirmed = raw_root
        .canonicalize()
        .map_err(|source| ProjectSandboxError::io("recanonicalize sandbox", raw_root, source))?;
    if confirmed != root {
        return Err(ProjectSandboxError::io(
            "confirm stable sandbox path",
            raw_root,
            io::Error::other("sandbox path changed during initialization"),
        ));
    }
    Ok(())
}

fn confirm_sandbox_parent(root: &Path, parent: &Path) -> Result<(), ProjectSandboxError> {
    if root.parent() == Some(parent) {
        Ok(())
    } else {
        Err(ProjectSandboxError::io(
            "confirm sandbox parent",
            root,
            io::Error::other("sandbox parent changed during initialization"),
        ))
    }
}

fn remove_sandbox(root: &Path) -> Result<(), ProjectSandboxCleanupError> {
    cleanup_with(root, |path| match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::remove_file(path),
        Ok(_) => fs::remove_dir_all(path),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(source),
    })
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

fn cleanup_temporary_initialization_failure(
    temporary: tempfile::TempDir,
    error: ProjectSandboxError,
) -> ProjectSandboxError {
    let retained = temporary.keep();
    complete_initialization_cleanup(error, remove_sandbox(&retained))
}

fn run_git_command(mut command: Command) -> Result<(), ProjectSandboxError> {
    let output = command
        .output()
        .map_err(|source| ProjectSandboxError::command("run Git", source))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ProjectSandboxError::command(
            "initialize Git sandbox",
            io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
        ))
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
            .args(["branch", "--show-current"]);
        let output = command.output().unwrap();
        assert_eq!(output.stdout, b"main\n");
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
    fn clean_git_sandbox_overrides_host_hook_and_exclusion_configuration() {
        let sandbox = ProjectSandbox::new(ProjectSandboxMode::CleanGit).unwrap();
        for (name, expected) in [
            ("core.hooksPath", sandbox.path().join(".git/hooks")),
            (
                "core.excludesFile",
                sandbox.path().join(".git/info/exclude"),
            ),
        ] {
            let mut command = Command::new("git");
            sandbox
                .configure_command(&mut command)
                .args(["config", "--local", "--get", name]);
            let output = command.output().unwrap();
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim_end(),
                expected.to_str().unwrap()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn clean_git_sandbox_ignores_hostile_global_git_configuration() {
        const CHILD_MARKER: &str = "MARA_TEST_SUPPORT_HOSTILE_GIT_CONFIG_CHILD";
        if env::var_os(CHILD_MARKER).is_some() {
            let sandbox = ProjectSandbox::new(ProjectSandboxMode::CleanGit).unwrap();
            assert_eq!(git_status(&sandbox), "");
            return;
        }

        let hostile = tempfile::tempdir().unwrap();
        let home = hostile.path().join("home");
        let template = hostile.path().join("template");
        let hooks = hostile.path().join("hooks");
        let excludes = hostile.path().join("excludes");
        let template_hook = template.join("hooks/pre-commit");
        let hostile_hook = hooks.join("pre-commit");
        fs::create_dir_all(template.join("hooks")).unwrap();
        fs::create_dir_all(&hooks).unwrap();
        fs::write(&template_hook, "#!/bin/sh\nexit 1\n").unwrap();
        fs::write(&hostile_hook, "#!/bin/sh\nexit 1\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&template_hook, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&hostile_hook, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&excludes, "*\n").unwrap();
        fs::create_dir(&home).unwrap();
        fs::write(
            home.join(".gitconfig"),
            format!(
                "[init]\n\ttemplateDir = {}\n[core]\n\thooksPath = {}\n\texcludesFile = {}\n[commit]\n\tgpgSign = true\n",
                template.display(),
                hooks.display(),
                excludes.display(),
            ),
        )
        .unwrap();

        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::clean_git_sandbox_ignores_hostile_global_git_configuration",
                "--nocapture",
            ])
            .env(CHILD_MARKER, "1")
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", hostile.path().join("xdg"))
            .env_remove("GIT_CONFIG_GLOBAL")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "hostile global Git configuration broke sandbox initialization:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
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

    #[cfg(unix)]
    #[test]
    fn temporary_parent_rejects_a_symlink_final_component() {
        let parent = canonical_temp_parent().unwrap();
        let temporary = tempfile::Builder::new()
            .prefix("mara-project-sandbox-parent-test-")
            .tempdir_in(&parent)
            .unwrap();
        let real = temporary.path().join("real-parent");
        let symlink = temporary.path().join("symlink-parent");
        fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &symlink).unwrap();
        let symlink_with_trailing_separator = PathBuf::from(format!("{}/", symlink.display()));

        assert_eq!(
            canonical_temp_parent_from([symlink_with_trailing_separator, real.clone()]).unwrap(),
            real.canonicalize().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn swapped_sandbox_symlinks_are_rejected_and_unlinked_without_touching_the_target() {
        let parent = canonical_temp_parent().unwrap();
        let temporary = tempfile::Builder::new()
            .prefix("mara-project-sandbox-swap-test-")
            .tempdir_in(&parent)
            .unwrap();
        let raw_root = temporary.path().join("sandbox");
        let target = temporary.path().join("target");
        fs::create_dir(&raw_root).unwrap();
        fs::create_dir(&target).unwrap();
        let root = raw_root.canonicalize().unwrap();
        fs::remove_dir(&raw_root).unwrap();
        std::os::unix::fs::symlink(&target, &raw_root).unwrap();

        let error = confirm_sandbox_root(&raw_root, &root).unwrap_err();
        assert!(error.to_string().contains("changed to a symlink"));
        remove_sandbox(&raw_root).unwrap();
        assert!(!raw_root.exists());
        assert!(target.exists());
    }

    #[test]
    fn sandbox_parent_must_remain_the_validated_parent() {
        let validated = tempfile::tempdir().unwrap();
        let swapped = tempfile::tempdir().unwrap();
        let error =
            confirm_sandbox_parent(&swapped.path().join("sandbox"), validated.path()).unwrap_err();
        assert!(error.to_string().contains("sandbox parent changed"));
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
    fn failed_temporary_initialization_cleans_up_before_returning_the_error() {
        let parent = canonical_temp_parent().unwrap();
        let temporary = tempfile::Builder::new().tempdir_in(parent).unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let error = cleanup_temporary_initialization_failure(
            temporary,
            ProjectSandboxError::io(
                "inspect sandbox",
                &root,
                io::Error::other("injected inspection failure"),
            ),
        );
        assert!(error.cleanup_error().is_none());
        assert!(!root.exists());
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
    fn preserve_on_failure_tracks_the_final_test_result() {
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
        let removed_after_caught_worker_panic = receiver.recv().unwrap();
        assert!(
            !removed_after_caught_worker_panic.exists(),
            "a caught worker panic must not retain a sandbox after this test succeeds"
        );

        let removed_after_success = {
            let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty)
                .unwrap()
                .preserve_on_failure();
            sandbox.path().to_path_buf()
        };
        assert!(!removed_after_success.exists());

        let removed_after_caught_panic = {
            let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty)
                .unwrap()
                .preserve_on_failure();
            let path = sandbox.path().to_path_buf();
            assert!(std::panic::catch_unwind(|| panic!("caught by the test")).is_err());
            assert_eq!(
                sandbox.finish::<PathBuf>(Ok(())).report(),
                ExitCode::SUCCESS
            );
            path
        };
        assert!(!removed_after_caught_panic.exists());

        let retained_after_reported_error = {
            let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty)
                .unwrap()
                .preserve_on_failure();
            let path = sandbox.path().to_path_buf();
            assert_eq!(
                sandbox.finish(Err(path.clone())).report(),
                ExitCode::FAILURE
            );
            path
        };
        assert!(retained_after_reported_error.exists());
        fs::remove_dir_all(retained_after_reported_error).unwrap();
    }

    #[test]
    fn preserve_on_failure_reports_through_the_standard_test_harness()
    -> ProjectSandboxTestResult<PathBuf> {
        const CHILD_MARKER: &str = "MARA_TEST_SUPPORT_FAILURE_RESULT_CHILD";
        if env::var_os(CHILD_MARKER).is_some() {
            let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty)
                .unwrap()
                .preserve_on_failure();
            let path = sandbox.path().to_path_buf();
            return sandbox.finish(Err(path));
        }

        let sandbox = ProjectSandbox::new(ProjectSandboxMode::Empty)
            .unwrap()
            .preserve_on_failure();
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::preserve_on_failure_reports_through_the_standard_test_harness",
                "--nocapture",
            ])
            .env(CHILD_MARKER, "1")
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "a reported failure must fail the ordinary test harness"
        );
        let output = String::from_utf8_lossy(&output.stderr);
        let retained = output
            .lines()
            .find_map(|line| line.strip_prefix("preserved failed ProjectSandbox at "))
            .map(PathBuf::from)
            .expect("the harness must report the retained sandbox path");
        assert!(retained.exists());
        fs::remove_dir_all(retained).unwrap();
        sandbox.finish(Ok(()))
    }
}
