//! Code markers at the basic level (DES-CODE-MARKERS and DES-CODE-READERS in
//! `docs/code/index.mara.md`): the scan of one code file, keyed by its contents and its language's
//! comment prefixes, the code layer of every scanned file, and the code links that its markers
//! make with a snapshot's items.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Language;
use crate::diagnostic::Diagnostic;
use crate::hash::{Hash, KeyHasher};
use crate::schema::CODE_SOURCE;
use crate::snapshot::Snapshot;
use crate::validation::{is_id, list};
use crate::work::WorkCounts;

/// The version of the scan, part of every [`MarkerKey`].
pub const SCANNER_VERSION: u32 = 1;
/// The version of the code layer's key.
const LAYER_VERSION: u32 = 1;

/// The diagnostic codes of markers (DES-CODE-MARKERS, DES-SCHEMA-CHECKS).
pub const INVALID_MARKER: &str = "invalid-marker";
pub const UNRESOLVED_MARKER: &str = "unresolved-marker";
pub const DISALLOWED_TARGET: &str = "disallowed-target";

/// What starts a marker after its comment start.
const MARA: &[u8] = b"@mara";

/// The key of one file's markers: scanner version, the language's comment prefixes and the
/// file's hash (DES-INPUTS-AND-KEYS). The basic level has no grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MarkerKey(pub Hash);

/// The hash of what a language's scan depends on: its comment prefixes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LanguageKey(Hash);

impl LanguageKey {
    pub fn new(language: &Language) -> LanguageKey {
        let mut key = KeyHasher::new("mara code language", SCANNER_VERSION);
        key.u32(language.prefixes.len() as u32);
        for prefix in &language.prefixes {
            key.bytes(prefix.as_bytes());
        }
        LanguageKey(key.finish())
    }
}

impl MarkerKey {
    pub fn new(language: LanguageKey, file: Hash) -> MarkerKey {
        let mut key = KeyHasher::new("mara code markers", SCANNER_VERSION);
        key.hash(&language.0);
        key.hash(&file);
        MarkerKey(key.finish())
    }
}

/// A marker: a comment whose text starts with `@mara`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    /// 1-based.
    pub line: usize,
    /// The relation and the ID it names, or `None` when its words are not one name and one
    /// word of the form of an ID.
    pub link: Option<MarkerLink>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkerLink {
    /// The forward name as written.
    pub relation: String,
    pub id: String,
    /// The byte offset of the ID in the file.
    pub id_start: usize,
}

/// The markers of one file's contents, in line order. A file with markers keeps its contents,
/// which a rename rewrites.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScannedFile {
    pub markers: Vec<Marker>,
    pub contents: Option<Arc<[u8]>>,
    /// How many lines the file has: a last line without a line feed counts.
    pub lines: usize,
}

impl ScannedFile {
    /// Scans `contents` for markers with the comment prefixes `prefixes`, and counts one code
    /// scan in `work`. The file is read as bytes in lines that end at a line feed.
    pub fn scan(contents: &[u8], prefixes: &[String], work: &mut WorkCounts) -> ScannedFile {
        work.code_scans += 1;
        let mut markers = Vec::new();
        if find(contents, MARA).is_some() {
            let mut start = 0;
            for (index, line) in contents.split(|&byte| byte == b'\n').enumerate() {
                if find(line, MARA).is_some()
                    && let Some(link) = marker(line, prefixes)
                {
                    let link = link.map(|(relation, id, offset)| MarkerLink {
                        relation,
                        id,
                        id_start: start + offset,
                    });
                    markers.push(Marker {
                        line: index + 1,
                        link,
                    });
                }
                start += line.len() + 1;
            }
        }
        let feeds = contents.iter().filter(|&&byte| byte == b'\n').count();
        let lines = feeds + usize::from(!contents.is_empty() && !contents.ends_with(b"\n"));
        let contents = (!markers.is_empty()).then(|| Arc::from(contents));
        ScannedFile {
            markers,
            contents,
            lines,
        }
    }
}

/// The marker on `line`, if its comment is one: `None` when it holds none, `Some(None)` when its
/// words after `@mara` are not one name and one ID, else the relation, the ID and the ID's
/// offset in the line.
#[allow(clippy::type_complexity)]
fn marker(line: &[u8], prefixes: &[String]) -> Option<Option<(String, String, usize)>> {
    let after = comment_start(line, prefixes)?;
    let mut at = after;
    while matches!(line.get(at), Some(b' ' | b'\t')) {
        at += 1;
    }
    let rest = line[at..].strip_prefix(MARA)?;
    if !matches!(rest.first(), None | Some(b' ' | b'\t' | b'\r')) {
        return None;
    }
    let base = at + MARA.len();
    let mut words = Vec::new();
    let mut index = 0;
    let end = rest.len()
        - rest
            .iter()
            .rev()
            .take_while(|&&byte| matches!(byte, b' ' | b'\t' | b'\r'))
            .count();
    while index < end {
        if matches!(rest[index], b' ' | b'\t') {
            index += 1;
            continue;
        }
        let start = index;
        while index < end && !matches!(rest[index], b' ' | b'\t') {
            index += 1;
        }
        words.push((start, &rest[start..index]));
    }
    let [(_, relation), (offset, id)] = words[..] else {
        return Some(None);
    };
    let (Ok(relation), Ok(id)) = (std::str::from_utf8(relation), std::str::from_utf8(id)) else {
        return Some(None);
    };
    if !is_id(id) {
        return Some(None);
    }
    Some(Some((relation.to_owned(), id.to_owned(), base + offset)))
}

/// Where the comment on `line` starts its text: after the first of `prefixes` that stands at
/// the line's start or after a space or tab, the longest one where several start there.
fn comment_start(line: &[u8], prefixes: &[String]) -> Option<usize> {
    (0..line.len()).find_map(|at| {
        if at > 0 && !matches!(line[at - 1], b' ' | b'\t') {
            return None;
        }
        prefixes
            .iter()
            .map(|prefix| prefix.as_bytes())
            .filter(|prefix| line[at..].starts_with(prefix))
            .map(|prefix| at + prefix.len())
            .max()
    })
}

/// The first offset of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// One scanned code file of the layer.
#[derive(Clone, Debug)]
pub struct CodeFile {
    /// Relative to the project root.
    pub path: String,
    pub key: MarkerKey,
    /// The hash of its contents.
    pub hash: Hash,
    pub scanned: Arc<ScannedFile>,
}

/// Every code file that the configuration declares and the request read, by path, with the
/// key of their sorted (path, marker key) pairs (DES-INPUTS-AND-KEYS).
#[derive(Clone, Debug)]
pub struct CodeLayer {
    files: Vec<CodeFile>,
    key: Hash,
    /// The inventory's problems that may hide code: a code file that cannot be read, or a
    /// place that may hold one. Code is completely read when there is none (DES-CODE-READERS).
    pub unread: Vec<Diagnostic>,
}

impl CodeLayer {
    /// The layer of `files`, which have distinct paths, sorted by path.
    pub fn new(mut files: Vec<CodeFile>, unread: Vec<Diagnostic>) -> CodeLayer {
        files.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        let mut key = KeyHasher::new("mara code layer", LAYER_VERSION);
        for file in &files {
            key.bytes(file.path.as_bytes());
            key.hash(&file.key.0);
        }
        CodeLayer {
            files,
            key: key.finish(),
            unread,
        }
    }

    pub fn key(&self) -> Hash {
        self.key
    }

    /// In path order.
    pub fn files(&self) -> &[CodeFile] {
        &self.files
    }

    /// The index of the file at `path`.
    pub fn file(&self, path: &str) -> Option<usize> {
        self.files
            .binary_search_by(|file| file.path.as_str().cmp(path))
            .ok()
    }
}

/// A marker that names a declared relation and an ID that items have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeLink {
    /// The file's index in the layer.
    pub file: usize,
    /// The marker's index in its file.
    pub marker: usize,
    /// The marker's line.
    pub line: usize,
    /// The relation's forward name.
    pub relation: String,
    /// The snapshot indices of the items whose ID the marker names.
    pub items: Vec<usize>,
}

/// The code links of a layer against a snapshot, and the problems of its markers.
#[derive(Clone, Debug)]
pub struct CodeLinks {
    pub layer: Arc<CodeLayer>,
    /// In path and line order.
    pub links: Vec<CodeLink>,
    /// The links that target each item, by snapshot index.
    incoming: HashMap<usize, Vec<usize>>,
    /// By ID, the file index and line of every marker that names it, its relation declared or
    /// not and its target allowed or not.
    naming: HashMap<String, Vec<(usize, usize)>>,
    /// `invalid-marker`, `unresolved-marker` and `disallowed-target`, in path and line order.
    pub problems: Vec<Diagnostic>,
    key: Hash,
}

impl CodeLinks {
    /// Resolves every marker of `layer` against `snapshot`: one pass over the markers, each
    /// looked up by hash. A project without a schema has no code links and only the markers
    /// whose words are malformed as problems.
    pub fn new(snapshot: &Snapshot, layer: Arc<CodeLayer>) -> CodeLinks {
        let mut links = Vec::new();
        let mut incoming: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut naming: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        let mut problems = Vec::new();
        // Each target's type is read once: an item's rows may be many (DES-PERFORMANCE-HARNESS).
        let mut types: HashMap<usize, String> = HashMap::new();
        let schema = snapshot.schema();
        let from_code: Vec<&str> = schema
            .map(|schema| {
                let relations = schema.relations.iter();
                relations
                    .filter(|(_, relation)| relation.targets.contains_key(CODE_SOURCE))
                    .map(|(name, _)| name.as_str())
                    .collect()
            })
            .unwrap_or_default();
        for (index, file) in layer.files().iter().enumerate() {
            for (number, marker) in file.scanned.markers.iter().enumerate() {
                let at = |code, message| Diagnostic::error(code, &file.path, marker.line, message);
                let Some(link) = &marker.link else {
                    let message = "a marker is `@mara`, a relation's forward name and an ID, and \
                                   nothing after them, such as `@mara implements REQ-LOGIN`"
                        .to_owned();
                    problems.push(at(INVALID_MARKER, message));
                    continue;
                };
                let named = naming.entry(link.id.clone()).or_default();
                named.push((index, marker.line));
                let Some(schema) = schema else {
                    continue;
                };
                let Some(relation) = schema.relations.get(&link.relation) else {
                    let message = format!(
                        "`{}` is no relation that the schema declares; the relations from code \
                         are {}",
                        link.relation,
                        names(&from_code)
                    );
                    problems.push(at(INVALID_MARKER, message));
                    continue;
                };
                let items = snapshot.items_with_id(&link.id);
                if items.is_empty() {
                    let message = format!(
                        "no item has the ID `{}`: correct the marker, or rename it with the item \
                         if its ID changed",
                        link.id
                    );
                    problems.push(at(UNRESOLVED_MARKER, message));
                    continue;
                }
                let allowed = relation.targets.get(CODE_SOURCE);
                let disallowed: Vec<String> = items
                    .iter()
                    .map(|&item| {
                        let kind = types.entry(item).or_insert_with(|| type_of(snapshot, item));
                        kind.clone()
                    })
                    .filter(|kind| !allowed.is_some_and(|types| types.contains(kind)))
                    .collect();
                if let Some(kind) = disallowed.first() {
                    let message = match allowed {
                        Some(types) => format!(
                            "`{}` leads from code to {}, not to `{kind}` `{}`",
                            link.relation,
                            names(&types.iter().map(String::as_str).collect::<Vec<_>>()),
                            link.id
                        ),
                        None => format!(
                            "`{}` does not lead from code; the relations from code are {}",
                            link.relation,
                            names(&from_code)
                        ),
                    };
                    problems.push(at(DISALLOWED_TARGET, message));
                }
                let link_index = links.len();
                for &item in items {
                    incoming.entry(item).or_default().push(link_index);
                }
                links.push(CodeLink {
                    file: index,
                    marker: number,
                    line: marker.line,
                    relation: link.relation.clone(),
                    items: items.to_vec(),
                });
            }
        }
        let mut key = KeyHasher::new("mara code links", LAYER_VERSION);
        key.hash(&snapshot.key().0);
        key.hash(&layer.key());
        CodeLinks {
            layer,
            links,
            incoming,
            naming,
            problems,
            key: key.finish(),
        }
    }

    /// The key of the snapshot and the layer together.
    pub fn key(&self) -> Hash {
        self.key
    }

    /// The links that target the item at snapshot index `item`, in path and line order.
    pub fn incoming(&self, item: usize) -> impl Iterator<Item = &CodeLink> {
        let links = self.incoming.get(&item).map_or(&[][..], Vec::as_slice);
        links.iter().map(|&index| &self.links[index])
    }

    /// The path and line of every marker that names `id`, whatever its relation, in path and
    /// line order.
    pub fn naming(&self, id: &str) -> impl Iterator<Item = (&str, usize)> {
        let markers = self.naming.get(id).map_or(&[][..], Vec::as_slice);
        let files = self.layer.files();
        markers
            .iter()
            .map(|&(file, line)| (files[file].path.as_str(), line))
    }

    /// The path of the file that holds `link`.
    pub fn path(&self, link: &CodeLink) -> &str {
        &self.layer.files()[link.file].path
    }

    /// The links whose marker is in the file at layer index `file`, in line order.
    pub fn in_file(&self, file: usize) -> &[CodeLink] {
        let start = self.links.partition_point(|link| link.file < file);
        let end = self.links.partition_point(|link| link.file <= file);
        &self.links[start..end]
    }
}

/// The type name of the item at snapshot index `item`.
fn type_of(snapshot: &Snapshot, item: usize) -> String {
    crate::document::label_name(snapshot.item(item).value("type").unwrap_or_default())
}

/// `names`, each in backticks, or `none`.
fn names(names: &[&str]) -> String {
    match names {
        [] => "none".to_owned(),
        names => list(names.iter().map(|name| format!("`{name}`")), names.len()),
    }
}

/// The new contents of one code file in which a rename replaces the ID of markers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeEdit {
    pub path: String,
    pub contents: Vec<u8>,
}

/// The edits that rename the ID `old_id` to `new_id` in every marker of `layer` that names it:
/// only the ID's bytes change. A file with no such marker is not edited.
///
/// Each edit is checked before it is returned: the bytes replaced are the old ID, and the new
/// ID has the form of an ID, so the line stays a marker of the same relation that names the new
/// ID (DES-CODE-READERS). A check that fails returns the file's path, and nothing is to be
/// written.
pub fn rename(layer: &CodeLayer, old_id: &str, new_id: &str) -> Result<Vec<CodeEdit>, String> {
    let mut edits = Vec::new();
    if old_id == new_id {
        return Ok(edits);
    }
    for file in layer.files() {
        let offsets: Vec<usize> = file
            .scanned
            .markers
            .iter()
            .filter_map(|marker| marker.link.as_ref())
            .filter(|link| link.id == old_id)
            .map(|link| link.id_start)
            .collect();
        if offsets.is_empty() {
            continue;
        }
        let contents = file.scanned.contents.as_deref().unwrap_or_default();
        let replaced = |at: usize| contents.get(at..at + old_id.len()) == Some(old_id.as_bytes());
        if !is_id(new_id) || !offsets.iter().all(|&at| replaced(at)) {
            return Err(file.path.clone());
        }
        let mut new = Vec::with_capacity(contents.len() + offsets.len() * new_id.len());
        let mut copied = 0;
        for at in offsets {
            new.extend_from_slice(&contents[copied..at]);
            new.extend_from_slice(new_id.as_bytes());
            copied = at + old_id.len();
        }
        new.extend_from_slice(&contents[copied..]);
        edits.push(CodeEdit {
            path: file.path.clone(),
            contents: new,
        });
    }
    Ok(edits)
}
