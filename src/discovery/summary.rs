use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{DiscoveryGraph, DiscoveryNode, DiscoveryNodeKind};
use crate::{Corpus, ItemSource, MarkdownBlockKind, QueryError};

const HANDLE_PREFIX: &str = "mara:node:1:";
const TITLE_CHARS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryKind {
    Item,
    Section,
    Block,
    Document,
}

/// References only: no recursive summaries or inherited item metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct DiscoveryContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Nearest enclosing section, excluding the node itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

/// The shared, source-backed node projection for discovery operations.
/// Only titles are truncated. Page builders must preserve references and
/// locations, or report that their mandatory fields exceed the response budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct DiscoveryNodeSummary {
    pub reference: String,
    pub kind: DiscoveryKind,
    pub source: ItemSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub title_truncated: bool,
    pub context: DiscoveryContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flavour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_kind: Option<MarkdownBlockKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_level: Option<u8>,
}

impl DiscoveryGraph<'_> {
    pub(super) fn index_references(&mut self, corpus: &Corpus) {
        let revisions = corpus
            .documents()
            .iter()
            .map(|document| {
                (
                    document.path(),
                    Sha256::digest(document.source().as_bytes()),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        for index in self.graph.node_indices() {
            let node = &mut self.graph[index];
            let reference = if let DiscoveryNodeKind::Item(item) = node.kind {
                // Legacy/recovery items without a MID remain addressable by ID;
                // corpus validation owns the missing-MID diagnostic.
                item.mid().unwrap_or(item.id()).to_owned()
            } else {
                let mut hash = Sha256::new();
                hash.update(HANDLE_PREFIX);
                // Length-frame variable fields and use fixed-width offsets. No
                // Rust Hash/Debug representation, machine word size or graph ID
                // participates in this version's encoding.
                let path = node.source.path().as_os_str().as_encoded_bytes();
                hash.update((path.len() as u64).to_be_bytes());
                hash.update(path);
                hash.update(revisions[node.source.path()]);
                let kind = match node.kind {
                    DiscoveryNodeKind::Document(_) => "document",
                    DiscoveryNodeKind::Section { .. } => "section",
                    DiscoveryNodeKind::MarkdownBlock(block) => match block.kind() {
                        MarkdownBlockKind::Paragraph => "paragraph",
                        MarkdownBlockKind::Heading { .. } => "heading",
                        MarkdownBlockKind::ThematicBreak => "thematic_break",
                        MarkdownBlockKind::CodeBlock => "code_block",
                        MarkdownBlockKind::Blockquote => "blockquote",
                        MarkdownBlockKind::List => "list",
                        MarkdownBlockKind::ListItem => "list_item",
                        MarkdownBlockKind::HtmlBlock => "html_block",
                        MarkdownBlockKind::LinkReferenceDefinition => "link_reference_definition",
                        MarkdownBlockKind::Table => "table",
                        MarkdownBlockKind::TableHeader => "table_header",
                        MarkdownBlockKind::TableBody => "table_body",
                        MarkdownBlockKind::TableRow => "table_row",
                        MarkdownBlockKind::TableCell => "table_cell",
                    },
                    DiscoveryNodeKind::Item(_) => unreachable!(),
                };
                hash.update((kind.len() as u64).to_be_bytes());
                hash.update(kind);
                hash.update((node.source.span().start_byte() as u64).to_be_bytes());
                hash.update((node.source.span().end_byte() as u64).to_be_bytes());
                format!("{HANDLE_PREFIX}{:x}", hash.finalize())
            };
            self.references
                .entry(reference.clone())
                .or_default()
                .push(index);
            if let DiscoveryNodeKind::Item(item) = node.kind
                && item.id() != reference
            {
                self.references
                    .entry(item.id().to_owned())
                    .or_default()
                    .push(index);
            }
            node.reference = reference;
        }
    }
}

impl<'corpus> DiscoveryGraph<'corpus> {
    /// Resolve against this loaded snapshot. Rebuilding after edits keeps
    /// unrelated handles valid while reflecting current corpus connections.
    pub fn resolve(&self, reference: &str) -> Result<DiscoveryNode<'_, 'corpus>, QueryError> {
        let matches = self
            .references
            .get(reference)
            .map(Vec::as_slice)
            .unwrap_or_default();
        // Source-identical parser nodes (e.g. padded empty table cells) share
        // one structural handle. Item identities must still resolve uniquely.
        if let Some(index) = matches.first()
            && (matches.len() == 1 || reference.starts_with(HANDLE_PREFIX))
        {
            return Ok(DiscoveryNode {
                graph: self,
                index: *index,
            });
        }
        if reference.starts_with("mara:node:") {
            return Err(QueryError::InvalidDiscoveryReference);
        }
        if matches.is_empty() {
            return Err(QueryError::MissingItem {
                id: reference.to_owned(),
            });
        }
        if crate::is_mid(reference) {
            Err(QueryError::AmbiguousMid {
                mid: reference.to_owned(),
            })
        } else {
            Err(QueryError::AmbiguousItem {
                id: reference.to_owned(),
            })
        }
    }
}

impl<'graph, 'corpus> DiscoveryNode<'graph, 'corpus> {
    /// Item MID (or legacy ID), otherwise a versioned opaque source handle.
    pub fn reference(self) -> &'graph str {
        &self.graph.graph[self.index].reference
    }

    pub fn summary(self) -> DiscoveryNodeSummary {
        let parent = self.parent();
        let mut ancestor = parent;
        let mut section = None;
        while let Some(node) = ancestor {
            if matches!(node.kind(), DiscoveryNodeKind::Section { .. }) {
                section = Some(node.reference().to_owned());
                break;
            }
            ancestor = node.parent();
        }
        let mut summary = DiscoveryNodeSummary {
            reference: self.reference().to_owned(),
            kind: DiscoveryKind::Document,
            source: self.source().into(),
            title: None,
            title_truncated: false,
            context: DiscoveryContext {
                parent: parent.map(|node| node.reference().to_owned()),
                section,
            },
            id: None,
            mid: None,
            flavour: None,
            block_kind: None,
            heading_level: None,
        };
        let title = match self.kind() {
            DiscoveryNodeKind::Document(_) => None,
            DiscoveryNodeKind::Item(item) => {
                summary.kind = DiscoveryKind::Item;
                summary.id = Some(item.id().to_owned());
                summary.mid = item.mid().map(ToOwned::to_owned);
                summary.flavour = Some(item.flavour().to_owned());
                Some(item.title())
            }
            DiscoveryNodeKind::Section { heading } => {
                summary.kind = DiscoveryKind::Section;
                if let MarkdownBlockKind::Heading { level } = heading.kind() {
                    summary.heading_level = Some(level);
                }
                heading.heading_text()
            }
            DiscoveryNodeKind::MarkdownBlock(block) => {
                summary.kind = DiscoveryKind::Block;
                summary.block_kind = Some(block.kind());
                None
            }
        };
        if let Some(title) = title {
            let end = title
                .char_indices()
                .nth(TITLE_CHARS)
                .map_or(title.len(), |(byte, _)| byte);
            summary.title = Some(title[..end].to_owned());
            summary.title_truncated = end < title.len();
        }
        summary
    }
}
