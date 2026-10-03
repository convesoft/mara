use crate::MemoryWorkspace;
use mara_engine::{Error, PROJECT_FILE, SourceSnapshot, normalize_source_path};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Source data only. This is not a publication command or an external revision proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileChangeSet {
    pub format_version: u32,
    pub base_source_fingerprint: String,
    pub base_identity: Option<String>,
    pub changes: Vec<FileChange>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
impl FileChangeSet {
    /// Check the complete freshly supplied effective base and *all* preimages before
    /// the host writes anything. The producer still owns completeness, external-context
    /// verification and preventing concurrent changes between this check and application.
    /// Opaque base identities deliberately do not participate in source equality.
    // @mara implements DES-BROWSER-FILE-CHANGESET
    pub fn check_preconditions(&self, snapshot: SourceSnapshot) -> Result<(), Error> {
        if self.format_version != 1 {
            return Err(invalid("unsupported file-change-set format; expected 1"));
        }
        // Reuse the memory loader's normalization and complete-required-inventory checks.
        let actual = MemoryWorkspace::new(snapshot)?;
        if self.base_source_fingerprint != actual.base_fingerprint {
            return Err(invalid("whole-base source fingerprint does not match"));
        }
        let files = actual.engine.files();
        let mut previous: Option<&str> = None;
        for change in &self.changes {
            if normalize_source_path(&change.path)? != change.path
                || previous.is_some_and(|p| p >= change.path.as_str())
            {
                return Err(invalid(
                    "change paths must be normalized, unique and ordered",
                ));
            }
            previous = Some(&change.path);
            if change.before == change.after {
                return Err(invalid("unchanged files must be omitted"));
            }
            if files.get(&change.path) != change.before.as_ref() {
                return Err(invalid(format!(
                    "changed-file preimage does not match: '{}'",
                    change.path
                )));
            }
        }
        Ok(())
    }
}
fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidProject {
        path: PROJECT_FILE.into(),
        message: message.into(),
    }
}

// @mara implements DES-BROWSER-FILE-CHANGESET
pub(crate) fn consolidate(
    base: &SourceSnapshot,
    current: &BTreeMap<String, String>,
    base_fingerprint: &str,
) -> FileChangeSet {
    let original: BTreeMap<_, _> = base
        .files
        .iter()
        .map(|f| (f.path.clone(), f.content.clone()))
        .collect();
    let mut paths = original.keys().chain(current.keys()).collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let changes = paths
        .into_iter()
        .filter_map(|path| {
            let before = original.get(path);
            let after = current.get(path);
            (before != after).then(|| FileChange {
                path: path.clone(),
                before: before.cloned(),
                after: after.cloned(),
            })
        })
        .collect();
    FileChangeSet {
        format_version: 1,
        base_source_fingerprint: base_fingerprint.into(),
        base_identity: base.base_identity.clone(),
        changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mara_engine::{SelectionInputs, SourceEngine, SourceFile};

    // @mara checks REQ-BROWSER-CHANGE-EXPORT
    #[test]
    fn consolidation_is_a_final_delta_including_deletions_and_exact_utf8() {
        let base = SourceEngine::new(SourceSnapshot {
            format_version: 1,
            files: [
                ("./z//deleted", "café\r\n中文"),
                ("restored", "same"),
                ("modified", "before"),
            ]
            .into_iter()
            .map(|(path, content)| SourceFile {
                path: path.into(),
                content: content.into(),
            })
            .collect(),
            selection: SelectionInputs {
                document_paths: vec![],
                ignores: vec![],
            },
            base_identity: None,
        })
        .unwrap();
        let mut current = base.files().clone();
        current.insert("transient".into(), "added".into());
        current.remove("transient");
        current.insert("restored".into(), "intermediate".into());
        current.insert("restored".into(), "same".into());
        current.remove("z/deleted");
        current.insert("modified".into(), "after\r\n👩🏽‍💻".into());
        current.insert("a/addition".into(), String::new());
        let delta = consolidate(&base.snapshot(), &current, base.source_fingerprint());
        assert_eq!(
            delta.changes,
            vec![
                FileChange {
                    path: "a/addition".into(),
                    before: None,
                    after: Some(String::new())
                },
                FileChange {
                    path: "modified".into(),
                    before: Some("before".into()),
                    after: Some("after\r\n👩🏽‍💻".into())
                },
                FileChange {
                    path: "z/deleted".into(),
                    before: Some("café\r\n中文".into()),
                    after: None
                },
            ]
        );
        assert_eq!(
            serde_json::to_value(&delta).unwrap()["base_identity"],
            serde_json::Value::Null
        );
        assert_eq!(
            delta,
            consolidate(&base.snapshot(), &current, base.source_fingerprint())
        );
    }
}
