//! The document snapshot of one input generation: items, ID and MID maps, forward and inverse
//! edges, item spans and the token index (DES-SNAPSHOT and DES-INPUTS-AND-KEYS in
//! `docs/engine.mara.md`).
//!
//! A snapshot is assembled from per-file results ([`ParsedFile`]) that the host keeps by their
//! [`DocumentKey`], so a one-file change re-parses that file and then rebuilds the indexes in one
//! linear pass.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use crate::document::{self, Document, DocumentKey, Item, Link, Posting, is_mid};
use crate::hash::{Hash, KeyHasher};
use crate::schema::Schema;
use crate::work::WorkCounts;

/// The version of what [`SnapshotInputs::build`] derives, part of every [`SnapshotKey`].
pub const SNAPSHOT_VERSION: u32 = 1;

/// One document's contents and their parse: the per-file result.
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedFile {
    /// The file's bytes. Every span and line of the document refers to them.
    pub contents: Box<[u8]>,
    pub document: Document,
}

impl ParsedFile {
    /// Parses `contents` and counts one parse in `work`.
    pub fn parse(contents: Vec<u8>, work: &mut WorkCounts) -> ParsedFile {
        let document = document::parse(&contents, work);
        ParsedFile {
            contents: contents.into_boxed_slice(),
            document,
        }
    }
}

/// A document of the snapshot to build.
#[derive(Clone, Debug)]
pub struct DocumentInput {
    /// Relative to the project root, with `/` between its parts.
    pub path: String,
    pub key: DocumentKey,
    pub file: Arc<ParsedFile>,
}

/// The key of a snapshot: its documents' paths and keys, the schema hash and the configuration
/// hash (DES-INPUTS-AND-KEYS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SnapshotKey(pub Hash);

/// What a snapshot is built from, with its key.
#[derive(Debug)]
pub struct SnapshotInputs {
    key: SnapshotKey,
    documents: Vec<DocumentInput>,
    schema: Option<Arc<Schema>>,
}

impl SnapshotInputs {
    /// Sorts `documents`, which have distinct paths, by path and computes the key once.
    /// `schema_hash` and `config_hash` are the hashes of the schema and configuration files as
    /// read, `None` for a file that could not be read; `schema` is what was read of the schema.
    pub fn new(
        mut documents: Vec<DocumentInput>,
        schema: Option<Arc<Schema>>,
        schema_hash: Option<Hash>,
        config_hash: Option<Hash>,
    ) -> SnapshotInputs {
        documents.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        debug_assert!(
            documents
                .windows(2)
                .all(|pair| pair[0].path != pair[1].path)
        );
        let mut key = KeyHasher::new("mara document snapshot", SNAPSHOT_VERSION);
        key.optional(config_hash.as_ref());
        key.optional(schema_hash.as_ref());
        for document in &documents {
            key.bytes(document.path.as_bytes());
            key.u32(document.key.parser_version);
            key.hash(&document.key.file);
        }
        SnapshotInputs {
            key: SnapshotKey(key.finish()),
            documents,
            schema,
        }
    }

    pub fn key(&self) -> SnapshotKey {
        self.key
    }

    /// Builds the snapshot in one pass over the documents' items, rows, links and words, and
    /// counts one snapshot build in `work`. A row is a relation edge when the schema declares
    /// a relation of its name and its value is a link to a MID; every mention is an edge.
    pub fn build(self, work: &mut WorkCounts) -> Snapshot {
        work.snapshot_builds += 1;
        let SnapshotInputs {
            key,
            documents,
            schema,
        } = self;
        let items: usize = documents
            .iter()
            .map(|input| input.file.document.items.len())
            .sum();
        let mut snapshot = Snapshot {
            key,
            schema,
            documents: Vec::with_capacity(documents.len()),
            items: Vec::with_capacity(items),
            edges: Vec::new(),
            mids: HashMap::with_capacity(items),
            ids: HashMap::with_capacity(items),
            incoming: HashMap::new(),
            words: HashMap::new(),
        };
        for input in documents {
            snapshot.add_document(input);
        }
        snapshot
    }
}

/// The immutable snapshot of one input generation, shared by `Arc`.
#[derive(Debug)]
pub struct Snapshot {
    key: SnapshotKey,
    schema: Option<Arc<Schema>>,
    /// By path.
    documents: Vec<SnapshotDocument>,
    /// In path order, then source order.
    items: Vec<ItemEntry>,
    /// By source: each item's in row order, then mention order, then each document's narrative.
    edges: Vec<Edge>,
    /// The items with each MID, and with each `ID` value. A repeated key keeps every item.
    mids: HashMap<String, Vec<usize>>,
    ids: HashMap<String, Vec<usize>>,
    /// The edges to each MID, whether or not an item has it.
    incoming: HashMap<String, Vec<usize>>,
    /// The postings of each search word, by snapshot item.
    words: HashMap<String, Vec<Posting>>,
}

/// A document of a snapshot.
#[derive(Debug)]
pub struct SnapshotDocument {
    pub path: String,
    pub key: DocumentKey,
    pub file: Arc<ParsedFile>,
    /// Its items among the snapshot's items.
    pub items: Range<usize>,
    /// Its mentions outside every item among the snapshot's edges.
    pub narrative: Range<usize>,
}

/// Where an item of a snapshot is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemEntry {
    pub document: usize,
    /// The item's index among its document's items.
    pub index: usize,
    /// Its outgoing edges among the snapshot's edges.
    pub edges: Range<usize>,
}

/// A link to a MID from an item's metadata or text, or from the narrative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub document: usize,
    /// The item it leaves, or `None` for a mention outside every item.
    pub item: Option<usize>,
    pub source: EdgeSource,
}

/// The link of an edge, by its index in the parsed document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeSource {
    /// A relation: the row at this index of the item's rows.
    Row(usize),
    /// The mention at this index of the item's mentions.
    Mention(usize),
    /// The mention at this index of the document's mentions outside items.
    Narrative(usize),
}

impl Snapshot {
    pub fn key(&self) -> SnapshotKey {
        self.key
    }

    /// The schema the snapshot was built with, if one could be read.
    pub fn schema(&self) -> Option<&Arc<Schema>> {
        self.schema.as_ref()
    }

    /// The documents, by path.
    pub fn documents(&self) -> &[SnapshotDocument] {
        &self.documents
    }

    /// The index of the document at `path`.
    pub fn document(&self, path: &str) -> Option<usize> {
        self.documents
            .binary_search_by(|document| document.path.as_str().cmp(path))
            .ok()
    }

    /// The items, in path order, then source order.
    pub fn items(&self) -> &[ItemEntry] {
        &self.items
    }

    /// The item at `index` of [`Snapshot::items`]. Its span and lines refer to its document's
    /// contents.
    pub fn item(&self, index: usize) -> &Item {
        let entry = &self.items[index];
        &self.documents[entry.document].file.document.items[entry.index]
    }

    /// The items whose MID is `mid`: one, unless the project repeats it.
    pub fn items_with_mid(&self, mid: &str) -> &[usize] {
        self.mids.get(mid).map_or(&[], Vec::as_slice)
    }

    /// The items with an `ID` row whose value is `id`: one, unless the project repeats it.
    pub fn items_with_id(&self, id: &str) -> &[usize] {
        self.ids.get(id).map_or(&[], Vec::as_slice)
    }

    /// The edges that leave the item at `item`, in row order, then mention order.
    pub fn outgoing(&self, item: usize) -> &[Edge] {
        &self.edges[self.items[item].edges.clone()]
    }

    /// The mentions outside every item of the document at `document`.
    pub fn narrative(&self, document: usize) -> &[Edge] {
        &self.edges[self.documents[document].narrative.clone()]
    }

    /// The edges whose link targets `mid`, in path and source order.
    pub fn incoming(&self, mid: &str) -> impl Iterator<Item = &Edge> {
        self.incoming
            .get(mid)
            .into_iter()
            .flatten()
            .map(|&edge| &self.edges[edge])
    }

    /// The link of `edge`.
    pub fn link(&self, edge: &Edge) -> &Link {
        let document = &self.documents[edge.document].file.document;
        let item = || self.item(edge.item.expect("an item's edge has an item"));
        match edge.source {
            EdgeSource::Row(row) => item().rows[row]
                .link
                .as_ref()
                .expect("a relation edge's row has a link"),
            EdgeSource::Mention(mention) => &item().mentions[mention],
            EdgeSource::Narrative(mention) => &document.mentions[mention],
        }
    }

    /// The relation name of `edge`, or `None` for a mention.
    pub fn relation(&self, edge: &Edge) -> Option<&str> {
        match (edge.source, edge.item) {
            (EdgeSource::Row(row), Some(item)) => Some(&self.item(item).rows[row].name),
            _ => None,
        }
    }

    /// The items whose text holds `word`, a normalized word ([`document::words`]), in item
    /// order, then zone order.
    pub fn postings(&self, word: &str) -> &[Posting] {
        self.words.get(word).map_or(&[], Vec::as_slice)
    }

    fn add_document(&mut self, input: DocumentInput) {
        let index = self.documents.len();
        let document = &input.file.document;
        let first = self.items.len();
        for (local, item) in document.items.iter().enumerate() {
            let at = self.items.len();
            add(&mut self.mids, &item.mid, at);
            for row in item.rows.iter().filter(|row| row.name == "id") {
                add(&mut self.ids, &row.value, at);
            }
            let start = self.edges.len();
            for (row, value) in item.rows.iter().enumerate() {
                let relation = self
                    .schema
                    .as_ref()
                    .is_some_and(|schema| schema.relations.contains_key(&value.name));
                if let Some(link) = value.link.as_ref().filter(|_| relation) {
                    self.add_edge(index, Some(at), EdgeSource::Row(row), link);
                }
            }
            for (mention, link) in item.mentions.iter().enumerate() {
                self.add_edge(index, Some(at), EdgeSource::Mention(mention), link);
            }
            self.items.push(ItemEntry {
                document: index,
                index: local,
                edges: start..self.edges.len(),
            });
        }
        let narrative = self.edges.len();
        for (mention, link) in document.mentions.iter().enumerate() {
            self.add_edge(index, None, EdgeSource::Narrative(mention), link);
        }
        for word in &document.words {
            let postings = word.postings.iter().map(|posting| Posting {
                item: first + posting.item,
                zone: posting.zone,
            });
            match self.words.get_mut(&word.text) {
                Some(all) => all.extend(postings),
                None => {
                    self.words.insert(word.text.clone(), postings.collect());
                }
            }
        }
        self.documents.push(SnapshotDocument {
            items: first..self.items.len(),
            narrative: narrative..self.edges.len(),
            path: input.path,
            key: input.key,
            file: input.file,
        });
    }

    /// Adds the edge of `link` when its fragment is a MID.
    fn add_edge(&mut self, document: usize, item: Option<usize>, source: EdgeSource, link: &Link) {
        let Some(mid) = link.fragment.as_deref().filter(|fragment| is_mid(fragment)) else {
            return;
        };
        add(&mut self.incoming, mid, self.edges.len());
        self.edges.push(Edge {
            document,
            item,
            source,
        });
    }
}

/// Adds `value` to the values of `key`, copying the key only when it is new.
fn add(map: &mut HashMap<String, Vec<usize>>, key: &str, value: usize) {
    match map.get_mut(key) {
        Some(values) => values.push(value),
        None => {
            map.insert(key.to_owned(), vec![value]);
        }
    }
}
