//! Retain parsed destinations and their authored evidence; resolution belongs to discovery.
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rushdown::{
    ast::{Arena, KindData, NodeRef},
    parser::{self, AnyInlineParser, InlineParser},
    text::{BlockReader, Reader as _},
};

use super::{ParsedDocument, ParsedReference};
use crate::ReferenceKind;

#[derive(Debug)]
pub(super) struct LinkParserWithSpans {
    parser: parser::LinkParser,
    ends: Rc<RefCell<HashMap<usize, usize>>>,
}

impl LinkParserWithSpans {
    pub(super) fn new(ends: Rc<RefCell<HashMap<usize, usize>>>) -> Self {
        Self {
            parser: parser::LinkParser::new(),
            ends,
        }
    }
}

impl InlineParser for LinkParserWithSpans {
    fn trigger(&self) -> &[u8] {
        self.parser.trigger()
    }

    fn parse(
        &self,
        arena: &mut Arena,
        parent: NodeRef,
        reader: &mut BlockReader,
        context: &mut parser::Context,
    ) -> Option<NodeRef> {
        let node = self.parser.parse(arena, parent, reader, context)?;
        if matches!(arena[node].kind_data(), KindData::Link(_))
            && let Some(start) = arena[node].pos()
        {
            self.ends
                .borrow_mut()
                .insert(start, reader.position().1.start());
        }
        Some(node)
    }

    fn close_block(
        &self,
        arena: &mut Arena,
        parent: NodeRef,
        reader: &mut BlockReader,
        context: &mut parser::Context,
    ) {
        self.parser.close_block(arena, parent, reader, context);
    }
}

impl From<LinkParserWithSpans> for AnyInlineParser {
    fn from(parser: LinkParserWithSpans) -> Self {
        Self::Extension(Box::new(parser))
    }
}

pub(super) fn collect(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    ends: &HashMap<usize, usize>,
    document: &mut ParsedDocument,
) {
    if arena[node].kind_data().kind_name() == "MaraItem" {
        let item = &rushdown::as_extension_data!(arena, node, super::containers::MaraItemNode).item;
        if !item.body_valid || !item.title_valid {
            return;
        }
    }
    if let Some(start) = arena[node].pos() {
        match arena[node].kind_data() {
            KindData::Link(link) => {
                if let Some(&end) = ends.get(&start)
                    && source.get(start..end).is_some()
                    // Recognition already gave Mara mentions precedence over
                    // Markdown. A matching reference definition must not turn
                    // their inner brackets into a second, unrelated link.
                    && !document.references.iter().any(|reference| {
                        reference.kind == ReferenceKind::Item
                            && reference.source.start <= start
                            && reference.source.end >= end
                    })
                {
                    document.references.push(ParsedReference {
                        kind: ReferenceKind::MarkdownLink,
                        target: markdown_text(link.destination_str(source)),
                        source: start..end,
                    });
                }
            }
            KindData::RawHtml(_) | KindData::HtmlBlock(_) => {
                // Consecutive standalone tags can share one HTML block. Only consume
                // its leading anchor declarations, never tags hidden in raw contexts.
                let mut offset = start;
                while let Some((name, length)) = anchor(&source[offset..]) {
                    document.references.push(ParsedReference {
                        kind: ReferenceKind::Anchor,
                        target: name,
                        source: offset..offset + length,
                    });
                    if !matches!(arena[node].kind_data(), KindData::HtmlBlock(_)) {
                        break;
                    }
                    let rest = &source[offset + length..];
                    // A blank line ends this HTML block; later AST nodes own their anchors.
                    let whitespace = rest.len() - rest.trim_start().len();
                    if rest[..whitespace].bytes().filter(|&c| c == b'\n').count() > 1 {
                        break;
                    }
                    offset += length + whitespace;
                }
            }
            _ => {}
        }
    }
    for child in arena[node].children(arena) {
        collect(arena, child, source, ends, document);
    }
}

fn markdown_text(text: &str) -> String {
    let mut escaped = String::new();
    rushdown::renderer::html::Writer::new()
        .write(&mut escaped, text)
        .expect("writing destination to String");
    String::from_utf8(rushdown::util::resolve_entity_references(escaped.as_bytes()).into_owned())
        .expect("Markdown destination is UTF-8")
}

/// The supported explicit-anchor spelling, with either HTML quote style.
fn anchor(source: &str) -> Option<(String, usize)> {
    let mut rest = source.strip_prefix("<a")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    rest = rest
        .trim_start()
        .strip_prefix("name")?
        .trim_start()
        .strip_prefix('=')?
        .trim_start();
    let quote = rest.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    rest = &rest[1..];
    let end = rest.find(quote)?;
    let name = &rest[..end];
    rest = rest[end + 1..]
        .trim_start()
        .strip_prefix('>')?
        .trim_start()
        .strip_prefix("</a>")?;
    let name =
        String::from_utf8(rushdown::util::resolve_entity_references(name.as_bytes()).into_owned())
            .ok()?;
    Some((name, source.len() - rest.len()))
}
