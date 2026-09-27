use crate::corpus::parse_document_source;
use crate::{Error, Item, Project, Schema, load_corpus_for_validation, validate_corpus};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
mod transaction;
use transaction::MutationLock;
pub use transaction::{TransactionRollback, rollback_transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfilledMids {
    entries: Vec<BackfilledMid>,
}

impl BackfilledMids {
    pub fn entries(&self) -> &[BackfilledMid] {
        &self.entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfilledMid {
    id: String,
    mid: String,
    path: PathBuf,
    line: usize,
}

impl BackfilledMid {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn mid(&self) -> &str {
        &self.mid
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line(&self) -> usize {
        self.line
    }
}

// @mara implements REQ-MID-BACKFILL
// @mara implements DES-MID-BACKFILL
pub fn backfill_mids(project: &Project, schema: &Schema) -> Result<BackfilledMids, Error> {
    let _lock = MutationLock::acquire(project)?;
    let (corpus, mut diagnostics) = load_corpus_for_validation(project, schema)?;
    diagnostics.extend(validate_corpus(&corpus, schema));
    let blocking = diagnostics
        .into_iter()
        .filter(|diagnostic| !diagnostic.is_missing_mid())
        .collect::<Vec<_>>();
    if let Some(diagnostic) = blocking.first() {
        return invalid(format!(
            "cannot backfill MIDs while validation fails at {}:{}: {}",
            diagnostic.source().path().display(),
            diagnostic.source().span().start_line(),
            diagnostic.message()
        ));
    }

    let mut known_mids = corpus
        .items()
        .filter_map(Item::mid)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut by_path = BTreeMap::<PathBuf, Vec<(usize, BackfilledMid)>>::new();
    for document in corpus.documents() {
        for item in document.items().iter().filter(|item| item.mid().is_none()) {
            let mid = generate_mid(known_mids.iter().map(String::as_str));
            known_mids.push(mid.clone());
            by_path
                .entry(document.path().to_path_buf())
                .or_default()
                .push((
                    end_of_line_containing(document.source(), item.source().span().start_byte()),
                    BackfilledMid {
                        id: item.id().to_owned(),
                        mid,
                        path: document.path().to_path_buf(),
                        line: item.source().span().start_line() + 1,
                    },
                ));
        }
    }

    let mut candidates = BTreeMap::<PathBuf, String>::new();
    let mut entries = Vec::new();
    for (path, mut insertions) in by_path {
        let document = corpus
            .documents()
            .iter()
            .find(|document| document.path() == path)
            .expect("backfill target belongs to the loaded corpus");
        let newline = newline_style(document.source());
        insertions.sort_by_key(|(position, _)| *position);
        for (offset, (_, entry)) in insertions.iter_mut().enumerate() {
            entry.line += offset;
        }
        let mut candidate = document.source().to_owned();
        for (position, entry) in insertions.iter().rev() {
            candidate.insert_str(*position, &format!(":mid: {}{newline}", entry.mid()));
        }
        parse_document_source(&path, &candidate, schema)?;
        entries.extend(insertions.into_iter().map(|(_, entry)| entry));
        candidates.insert(path, candidate);
    }

    for (path, candidate) in candidates {
        atomic_replace(&project.root().join(&path), &candidate, true)?;
    }

    Ok(BackfilledMids { entries })
}

fn newline_style(source: &str) -> &'static str {
    source
        .find('\n')
        .filter(|index| *index > 0 && source.as_bytes()[index - 1] == b'\r')
        .map_or("\n", |_| "\r\n")
}

fn end_of_line_containing(source: &str, start: usize) -> usize {
    source[start..]
        .find('\n')
        .map_or(source.len(), |offset| start + offset + 1)
}

fn generate_mid<'a>(existing: impl IntoIterator<Item = &'a str>) -> String {
    let existing = existing.into_iter().collect::<Vec<_>>();
    loop {
        let mid = ulid::Ulid::new().to_string();
        if !existing.iter().any(|value| **value == mid) {
            return mid;
        }
    }
}

fn atomic_replace(path: &Path, source: &str, existed: bool) -> Result<(), Error> {
    let parent = path.parent().expect("a destination file has a parent");
    let permissions = existed
        .then(|| fs::metadata(path).map(|metadata| metadata.permissions()))
        .transpose()
        .map_err(|source| Error::Io {
            action: "inspect destination permissions",
            path: path.to_path_buf(),
            source,
        })?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|source| Error::Io {
        action: "create temporary Mara document",
        path: parent.to_path_buf(),
        source,
    })?;
    temporary
        .as_file_mut()
        .write_all(source.as_bytes())
        .map_err(|source| Error::Io {
            action: "write temporary Mara document",
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .as_file_mut()
        .flush()
        .map_err(|source| Error::Io {
            action: "flush temporary Mara document",
            path: path.to_path_buf(),
            source,
        })?;
    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .map_err(|source| Error::Io {
                action: "preserve destination permissions",
                path: path.to_path_buf(),
                source,
            })?;
    }
    temporary.as_file().sync_all().map_err(|source| Error::Io {
        action: "sync temporary Mara document",
        path: path.to_path_buf(),
        source,
    })?;
    let persisted = if existed {
        temporary.persist(path)
    } else {
        temporary.persist_noclobber(path)
    };
    persisted.map_err(|error| Error::Io {
        action: "atomically replace Mara document",
        path: path.to_path_buf(),
        source: error.error,
    })?;
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidMutation {
        message: message.into(),
    })
}
