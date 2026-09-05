use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions, Permissions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use super::invalid;
use crate::{Error, Project};

const JOURNAL: &str = ".mara/transaction.json";

pub(super) struct MutationLock {
    _file: File,
}

impl Drop for MutationLock {
    fn drop(&mut self) {
        // Release at the operation boundary even if a concurrent subprocess
        // temporarily inherited a descriptor for the same lock file.
        let _ = self._file.unlock();
    }
}

impl MutationLock {
    pub(super) fn acquire(project: &Project) -> Result<Self, Error> {
        let lock = Self::lock(project)?;
        let journal_path = project.root().join(JOURNAL);
        match fs::symlink_metadata(&journal_path) {
            Ok(_) => {
                return invalid(
                    "pending transaction blocks mutations; stop other Mara writers, then run 'mara project transaction rollback'; preserve .mara/transaction.json if recovery fails",
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return io_at(&journal_path, Err(error)),
        }
        Ok(lock)
    }

    fn lock(project: &Project) -> Result<Self, Error> {
        let path = project.root().join(".mara/mutation.lock");
        safe_path(project, Path::new(".mara/mutation.lock"))?;
        let file = io_at(
            &path,
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path),
        )?;
        file.try_lock().map_err(|error| Error::InvalidMutation {
            message: format!(
                "another Mara mutation or recovery is active; retry after it finishes: {error}"
            ),
        })?;
        Ok(Self { _file: file })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileMode {
    readonly: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unix_mode: Option<u32>,
}

impl FileMode {
    fn from_permissions(permissions: Permissions) -> Self {
        #[cfg(unix)]
        let unix_mode = {
            use std::os::unix::fs::PermissionsExt;
            Some(permissions.mode())
        };
        #[cfg(not(unix))]
        let unix_mode = None;
        Self {
            readonly: permissions.readonly(),
            unix_mode,
        }
    }

    fn matches(&self, permissions: Permissions) -> bool {
        if self.readonly != permissions.readonly() {
            return false;
        }
        #[cfg(unix)]
        if let Some(mode) = self.unix_mode {
            use std::os::unix::fs::PermissionsExt;
            return mode == permissions.mode();
        }
        true
    }

    fn apply(&self, file: &File) -> io::Result<()> {
        let mut permissions = file.metadata()?.permissions();
        #[cfg(unix)]
        if let Some(mode) = self.unix_mode {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(mode);
        } else {
            permissions.set_readonly(self.readonly);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(self.readonly);
        file.set_permissions(permissions)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Change {
    path: PathBuf,
    #[serde(deserialize_with = "Option::deserialize")]
    before: Option<String>,
    after: String,
    #[serde(deserialize_with = "Option::deserialize")]
    mode: Option<FileMode>,
}

impl Change {
    pub(super) fn new(
        project: &Project,
        path: PathBuf,
        before: Option<String>,
        after: String,
    ) -> Result<Self, Error> {
        let absolute = safe_path(project, &path)?;
        let mode = before
            .as_ref()
            .map(|_| {
                io_at(&absolute, fs::metadata(&absolute))
                    .map(|metadata| FileMode::from_permissions(metadata.permissions()))
            })
            .transpose()?;
        let change = Self {
            path,
            before,
            after,
            mode,
        };
        change.verify(project, false)?;
        Ok(change)
    }

    fn verify(&self, project: &Project, allow_after: bool) -> Result<(), Error> {
        let absolute = safe_path(project, &self.path)?;
        let current = read_optional(&absolute)?;
        if current == self.before || (allow_after && current.as_deref() == Some(&self.after)) {
            if let Some(mode) = &self.mode {
                let permissions = io_at(&absolute, fs::metadata(&absolute))?.permissions();
                if !mode.matches(permissions) {
                    return invalid(format!(
                        "permissions of '{}' changed since preflight; restore recorded permissions before retrying rollback",
                        self.path.display()
                    ));
                }
            }
            Ok(())
        } else {
            invalid(format!(
                "file '{}' changed since transaction preflight; preserve its edits and .mara/transaction.json, restore the recorded preimage or candidate before retrying rollback",
                self.path.display()
            ))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    format_version: u32,
    changes: Vec<Change>,
}

#[derive(Debug, Clone)]
pub struct TransactionRollback {
    pub restored: Vec<PathBuf>,
}

pub fn rollback_transaction(project: &Project) -> Result<TransactionRollback, Error> {
    let _lock = MutationLock::lock(project)?;
    let path = safe_path(project, Path::new(JOURNAL))?;
    let Some(source) = read_optional(&path)? else {
        return Ok(TransactionRollback {
            restored: Vec::new(),
        });
    };
    let journal: Journal = serde_json::from_str(&source).map_err(|error| Error::InvalidMutation {
        message: format!("unrecoverable transaction journal: {error}; preserve .mara/transaction.json and restore affected files from a trusted backup before removing it"),
    })?;
    if journal.format_version != 1 || journal.changes.is_empty() {
        return invalid(
            "unsupported or empty transaction journal; preserve .mara/transaction.json and restore affected files from a trusted backup before removing it",
        );
    }
    let mut paths = BTreeSet::new();
    for change in &journal.changes {
        if !paths.insert(&change.path)
            || !change.path.to_string_lossy().ends_with(".mara.md")
            || change.before.is_some() != change.mode.is_some()
        {
            return invalid(
                "invalid transaction entries; preserve .mara/transaction.json and restore affected files from a trusted backup before removing it",
            );
        }
    }
    restore(project, &journal)?;
    clear_journal(project)?;
    Ok(TransactionRollback {
        restored: journal
            .changes
            .into_iter()
            .map(|change| change.path)
            .collect(),
    })
}

pub(super) fn commit(
    project: &Project,
    changes: Vec<Change>,
    verify: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    commit_with_hook(project, changes, verify, |_| Ok(()))
}

fn commit_with_hook(
    project: &Project,
    changes: Vec<Change>,
    verify: impl FnOnce() -> Result<(), Error>,
    mut hook: impl FnMut(Option<usize>) -> Result<(), Error>,
) -> Result<(), Error> {
    let journal = Journal {
        format_version: 1,
        changes,
    };
    let mut staged = Vec::new();
    for change in &journal.changes {
        let path = safe_path(project, &change.path)?;
        staged.push(stage(&path, &change.after, change.mode.as_ref())?);
    }
    verify()?;
    for change in &journal.changes {
        change.verify(project, false)?;
    }
    let journal_path = safe_path(project, Path::new(JOURNAL))?;
    let source = serde_json::to_string(&journal).map_err(|error| Error::InvalidMutation {
        message: format!("could not serialize transaction journal: {error}"),
    })?;
    // No originals can change before the journal's directory entry is durable.
    publish_journal(&journal_path, &source)?;
    let result: Result<(), Error> = (|| {
        hook(None)?;
        for (index, (change, temporary)) in journal.changes.iter().zip(staged).enumerate() {
            change.verify(project, false)?;
            let path = safe_path(project, &change.path)?;
            persist(temporary, &path, change.before.is_some())?;
            sync_parent(&path)?;
            hook(Some(index))?;
        }
        clear_journal(project)
    })();
    if let Err(error) = result {
        // A cleanup sync failure may occur after unlinking the journal. Restore
        // recovery information before attempting rollback of any original.
        let rollback = (|| {
            if read_optional(&journal_path)?.is_none() {
                publish_journal(&journal_path, &source)?;
            }
            restore(project, &journal)?;
            clear_journal(project)
        })();
        if let Err(rollback_error) = rollback {
            return invalid(format!(
                "{error}; rollback incomplete: {rollback_error}; mutations blocked until 'mara project transaction rollback' succeeds"
            ));
        }
        return invalid(format!(
            "{error}; transaction rolled back; originals restored"
        ));
    }
    Ok(())
}

fn publish_journal(path: &Path, source: &str) -> Result<(), Error> {
    let temporary = stage(path, source, None)?;
    io_at(
        path,
        temporary
            .persist_noclobber(path)
            .map(|_| ())
            .map_err(|error| error.error),
    )?;
    sync_parent(path)
}

fn restore(project: &Project, journal: &Journal) -> Result<(), Error> {
    // Check every target before restoring any, including a retry after interrupted rollback.
    for change in &journal.changes {
        change.verify(project, true)?;
    }
    let mut staged = Vec::new();
    for change in &journal.changes {
        let path = safe_path(project, &change.path)?;
        staged.push(
            change
                .before
                .as_ref()
                .map(|source| stage(&path, source, change.mode.as_ref()))
                .transpose()?,
        );
    }
    for (change, temporary) in journal.changes.iter().zip(staged) {
        change.verify(project, true)?;
        let path = safe_path(project, &change.path)?;
        if let Some(temporary) = temporary {
            persist(temporary, &path, true)?;
        } else if read_optional(&path)?.is_some() {
            io_at(&path, fs::remove_file(&path))?;
        }
        sync_parent(&path)?;
    }
    Ok(())
}

fn clear_journal(project: &Project) -> Result<(), Error> {
    let path = project.root().join(JOURNAL);
    io_at(&path, fs::remove_file(&path))?;
    sync_parent(&path)
}

fn stage(path: &Path, source: &str, mode: Option<&FileMode>) -> Result<NamedTempFile, Error> {
    let mut temporary = io_at(
        path,
        NamedTempFile::new_in(path.parent().expect("target has parent")),
    )?;
    io_at(path, temporary.write_all(source.as_bytes()))?;
    if let Some(mode) = mode {
        io_at(path, mode.apply(temporary.as_file()))?;
    }
    io_at(path, temporary.as_file().sync_all())?;
    Ok(temporary)
}

fn persist(temporary: NamedTempFile, path: &Path, existed: bool) -> Result<(), Error> {
    let result = if existed {
        temporary.persist(path)
    } else {
        temporary.persist_noclobber(path)
    };
    io_at(path, result.map(|_| ()).map_err(|error| error.error))
}

fn read_optional(path: &Path) -> Result<Option<String>, Error> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => io_at(path, Err(error)),
    }
}

fn safe_path(project: &Project, relative: &Path) -> Result<PathBuf, Error> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid("transaction paths must be normalized project-relative paths");
    }
    let mut path = project.root().to_path_buf();
    for component in relative.components() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return invalid(format!(
                    "transaction path '{}' must not traverse symlinks",
                    relative.display()
                ));
            }
            Ok(metadata) if path == project.root().join(relative) && !metadata.is_file() => {
                return invalid(format!(
                    "transaction target '{}' must be a regular file",
                    relative.display()
                ));
            }
            Ok(_) => {}
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && path == project.root().join(relative) => {}
            Err(error) => return io_at(&path, Err(error)),
        }
    }
    Ok(path)
}

fn sync_parent(path: &Path) -> Result<(), Error> {
    // Directory fsync is supported on Unix. Windows still flushes every staged file.
    #[cfg(unix)]
    {
        let parent = path.parent().expect("target has parent");
        io_at(parent, File::open(parent).and_then(|file| file.sync_all()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn io_at<T>(path: &Path, result: io::Result<T>) -> Result<T, Error> {
    result.map_err(|source| Error::Io {
        action: "access mutation transaction",
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Template, initialize_project};
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Project) {
        let directory = TempDir::new().unwrap();
        let project = initialize_project(directory.path(), Template::Minimal).unwrap();
        fs::write(directory.path().join("a.mara.md"), "original a\r\n").unwrap();
        fs::write(directory.path().join("b.mara.md"), "original b\n").unwrap();
        (directory, project)
    }

    fn changes(project: &Project, new_destination: bool) -> Vec<Change> {
        let destination = if new_destination {
            "new.mara.md"
        } else {
            "b.mara.md"
        };
        vec![
            Change::new(
                project,
                "a.mara.md".into(),
                Some("original a\r\n".into()),
                "".into(),
            )
            .unwrap(),
            Change::new(
                project,
                destination.into(),
                (!new_destination).then(|| "original b\n".into()),
                "moved a\r\n".into(),
            )
            .unwrap(),
        ]
    }

    #[test]
    fn write_failures_restore_every_original_and_remove_new_destinations() {
        for new_destination in [false, true] {
            for fail_at in [None, Some(0), Some(1)] {
                let (_directory, project) = fixture();
                let _lock = MutationLock::acquire(&project).unwrap();
                let result = commit_with_hook(
                    &project,
                    changes(&project, new_destination),
                    || Ok(()),
                    |phase| {
                        if phase == fail_at {
                            invalid::<()>("injected write failure")
                        } else {
                            Ok(())
                        }
                    },
                );
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("originals restored")
                );
                assert_eq!(
                    fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
                    "original a\r\n"
                );
                assert_eq!(
                    fs::read_to_string(project.root().join("b.mara.md")).unwrap(),
                    "original b\n"
                );
                assert!(!project.root().join("new.mara.md").exists());
                assert!(!project.root().join(JOURNAL).exists());
            }
        }
    }

    #[test]
    fn changed_preimages_abort_before_publication() {
        let (_directory, project) = fixture();
        let result = commit(&project, changes(&project, false), || {
            fs::write(project.root().join("b.mara.md"), "manual edit").unwrap();
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
            "original a\r\n"
        );
        assert_eq!(
            fs::read_to_string(project.root().join("b.mara.md")).unwrap(),
            "manual edit"
        );
        assert!(!project.root().join(JOURNAL).exists());
    }

    #[test]
    fn failed_rollback_keeps_journal_and_later_edits_until_explicit_recovery() {
        let (_directory, project) = fixture();
        let result = commit_with_hook(
            &project,
            changes(&project, false),
            || Ok(()),
            |phase| {
                if phase == Some(0) {
                    fs::write(project.root().join("b.mara.md"), "manual edit").unwrap();
                    invalid::<()>("injected failure")
                } else {
                    Ok(())
                }
            },
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("rollback incomplete")
        );
        assert!(MutationLock::acquire(&project).is_err());
        assert!(rollback_transaction(&project).is_err());
        assert_eq!(
            fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(project.root().join("b.mara.md")).unwrap(),
            "manual edit"
        );
        fs::write(project.root().join("b.mara.md"), "original b\n").unwrap();
        assert_eq!(rollback_transaction(&project).unwrap().restored.len(), 2);
        assert_eq!(
            fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
            "original a\r\n"
        );
        assert!(rollback_transaction(&project).unwrap().restored.is_empty());
    }

    #[test]
    fn malformed_recovery_entries_never_mutate_originals() {
        let (_directory, project) = fixture();
        let journal = Journal {
            format_version: 1,
            changes: changes(&project, false),
        };
        let value = serde_json::to_value(&journal).unwrap();
        for field in ["before", "mode"] {
            let mut damaged = value.clone();
            damaged["changes"][0].as_object_mut().unwrap().remove(field);
            fs::write(project.root().join(JOURNAL), damaged.to_string()).unwrap();
            let error = rollback_transaction(&project).unwrap_err().to_string();
            assert!(error.contains("missing field"), "{error}");
            assert_eq!(
                fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
                "original a\r\n"
            );
            assert!(project.root().join(JOURNAL).exists());
        }
    }

    #[test]
    fn rollback_still_rejects_mismatched_recorded_permissions() {
        let (_directory, project) = fixture();
        let journal = Journal {
            format_version: 1,
            changes: changes(&project, false),
        };
        let original = serde_json::to_value(&journal).unwrap();
        let mut readonly_mismatch = original.clone();
        let mode = readonly_mismatch["changes"][0]["mode"]
            .as_object_mut()
            .unwrap();
        let readonly = mode["readonly"].as_bool().unwrap();
        mode.insert("readonly".into(), serde_json::json!(!readonly));
        mode.remove("unix_mode");
        let mismatches = vec![readonly_mismatch];
        #[cfg(unix)]
        let mismatches = {
            let mut mismatches = mismatches;
            let mut mode_mismatch = original;
            let mode = mode_mismatch["changes"][0]["mode"]["unix_mode"]
                .as_u64()
                .unwrap();
            // Change an execute bit without changing the portable readonly value.
            mode_mismatch["changes"][0]["mode"]["unix_mode"] = serde_json::json!(mode ^ 0o100);
            mismatches.push(mode_mismatch);
            mismatches
        };
        for mismatch in mismatches {
            fs::write(project.root().join(JOURNAL), mismatch.to_string()).unwrap();
            let error = rollback_transaction(&project).unwrap_err().to_string();
            assert!(error.contains("permissions"), "{error}");
            assert!(MutationLock::acquire(&project).is_err());
            assert_eq!(
                fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
                "original a\r\n"
            );
            assert_eq!(
                fs::read_to_string(project.root().join("b.mara.md")).unwrap(),
                "original b\n"
            );
            assert_eq!(
                fs::read_to_string(project.root().join(JOURNAL)).unwrap(),
                mismatch.to_string()
            );
        }
    }

    #[test]
    fn active_mutation_lock_blocks_other_writers_and_recovery() {
        let (_directory, project) = fixture();
        let lock = MutationLock::acquire(&project).unwrap();
        assert!(MutationLock::acquire(&project).is_err());
        assert!(rollback_transaction(&project).is_err());
        drop(lock);
        assert!(MutationLock::acquire(&project).is_ok());
    }

    #[test]
    fn mutation_lock_is_released_even_when_a_descriptor_is_inherited() {
        let (_directory, project) = fixture();
        let lock = MutationLock::acquire(&project).unwrap();
        // A concurrent subprocess launch can inherit this open file description
        // briefly before exec closes it, even though the handle is close-on-exec.
        let inherited = lock._file.try_clone().unwrap();
        assert!(MutationLock::acquire(&project).is_err());
        drop(lock);
        let next = MutationLock::acquire(&project).unwrap();
        drop(inherited);
        drop(next);
    }

    #[test]
    fn interrupted_process_can_be_rolled_back_after_restart() {
        let (_directory, project) = fixture();
        for stop_after in ["prepared", "0", "1"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "mutation::transaction::tests::interruption_child",
                    "--ignored",
                ])
                .env("MARA_TEST_TRANSACTION_PROJECT", project.root())
                .env("MARA_TEST_STOP_AFTER", stop_after)
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(73));
            assert!(MutationLock::acquire(&project).is_err());
            assert_eq!(rollback_transaction(&project).unwrap().restored.len(), 2);
            assert_eq!(
                fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
                "original a\r\n"
            );
            assert!(!project.root().join("new.mara.md").exists());
            assert!(!project.root().join(JOURNAL).exists());
        }
    }

    #[test]
    #[ignore = "subprocess helper for interruption coverage"]
    fn interruption_child() {
        let path = std::env::var_os("MARA_TEST_TRANSACTION_PROJECT").unwrap();
        let project = crate::resolve_project(Some(Path::new(&path)), Path::new(&path)).unwrap();
        let stop_after = std::env::var("MARA_TEST_STOP_AFTER").unwrap();
        let _lock = MutationLock::acquire(&project).unwrap();
        commit_with_hook(
            &project,
            changes(&project, true),
            || Ok(()),
            |phase| {
                let phase = phase.map_or_else(|| "prepared".into(), |index| index.to_string());
                if phase == stop_after {
                    std::process::exit(73);
                }
                Ok(())
            },
        )
        .unwrap();
        panic!("interruption point was not reached");
    }
}
