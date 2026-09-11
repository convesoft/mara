//! The block parser cannot decide whether a delimiter is inside a multiline
//! code span: Rushdown parses inline nodes only after closing blocks. Use the
//! recognition pass's boundaries, then parse real containers without changing
//! which physical source lines are accepted as Mara syntax.

use std::{cell::RefCell, collections::HashMap, fmt, ops::Range, rc::Rc};

use rushdown::{
    ast::{
        Arena, CodeBlockKind, HeadingKind, KindData, NodeKind, NodeRef, NodeType, PrettyPrint,
        TypeData,
    },
    parser::{self, AnyBlockParser, BlockParser, Parser, ParserExtension, ParserExtensionFn},
    text::{BasicReader, Lines, MultilineValue, Reader as _, Segment, Value},
};

use super::{ParsedBlock, ParsedDocument, ParsedItem, source_lines};
use crate::MarkdownBlockKind;

/// The container owns the parsed identity, ordered metadata, and exact
/// opening/body/closing provenance alongside its ordinary Markdown children.
#[derive(Debug)]
struct MaraItemNode {
    item: ParsedItem,
}

impl NodeKind for MaraItemNode {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "MaraItem"
    }
}

impl PrettyPrint for MaraItemNode {
    fn pretty_print(
        &self,
        writer: &mut dyn fmt::Write,
        _source: &str,
        level: usize,
    ) -> fmt::Result {
        writeln!(writer, "{}MaraItem", "  ".repeat(level))
    }
}

impl From<MaraItemNode> for KindData {
    fn from(node: MaraItemNode) -> Self {
        Self::Extension(Box::new(node))
    }
}

#[derive(Debug)]
struct MaraItemParser {
    item: ParsedItem,
}

impl BlockParser for MaraItemParser {
    fn trigger(&self) -> &[u8] {
        b":"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent: NodeRef,
        reader: &mut BasicReader,
        _context: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        if reader.peek_line_segment()?.start() != self.item.source.start {
            return None;
        }
        reader.advance_to_eol();
        Some((
            arena.new_node(MaraItemNode {
                item: self.item.clone(),
            }),
            parser::State::HAS_CHILDREN,
        ))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node: NodeRef,
        reader: &mut BasicReader,
        _context: &mut parser::Context,
    ) -> Option<parser::State> {
        let item = &rushdown::as_extension_data!(arena, node, MaraItemNode).item;
        let start = reader.peek_line_segment()?.start();
        if start < item.body.start {
            // Metadata and the blank body boundary are owned by Mara, not
            // ordinary Markdown paragraphs or reference definitions.
            reader.advance_to_eol();
            return Some(parser::State::NO_CHILDREN);
        }
        if start >= item.body.end {
            reader.advance_to_eol();
            return None;
        }
        Some(parser::State::HAS_CHILDREN)
    }
}

impl From<MaraItemParser> for AnyBlockParser {
    fn from(parser: MaraItemParser) -> Self {
        Self::Extension(Box::new(parser))
    }
}

/// Rushdown does not retain closing fences or quote markers in content segments.
/// Record their consumed lines while delegating the grammar to Rushdown.
#[derive(Debug)]
struct BlockParserWithSpans {
    parser: AnyBlockParser,
    ends: Rc<RefCell<HashMap<usize, usize>>>,
}

impl BlockParser for BlockParserWithSpans {
    fn trigger(&self) -> &[u8] {
        self.parser.trigger()
    }

    fn open(
        &self,
        arena: &mut Arena,
        parent: NodeRef,
        reader: &mut BasicReader,
        context: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        let segment = reader.peek_line_segment()?;
        let start = segment.start() + context.block_offset().unwrap_or(0);
        let end = segment.stop();
        let result = self.parser.open(arena, parent, reader, context)?;
        self.ends.borrow_mut().insert(start, end);
        Some(result)
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node: NodeRef,
        reader: &mut BasicReader,
        context: &mut parser::Context,
    ) -> Option<parser::State> {
        let segment = reader.peek_line_segment()?;
        let state = self.parser.cont(arena, node, reader, context);
        if state.is_some() || reader.position().1.start() > segment.start() {
            self.ends.borrow_mut().insert(
                arena[node].pos().expect("opened block position"),
                segment.stop(),
            );
        }
        state
    }

    fn close(
        &self,
        arena: &mut Arena,
        node: NodeRef,
        reader: &mut BasicReader,
        context: &mut parser::Context,
    ) {
        self.parser.close(arena, node, reader, context);
    }

    fn can_interrupt_paragraph(&self) -> bool {
        self.parser.can_interrupt_paragraph()
    }
}

impl From<BlockParserWithSpans> for AnyBlockParser {
    fn from(parser: BlockParserWithSpans) -> Self {
        Self::Extension(Box::new(parser))
    }
}

fn item_tree(source: &str, item: &ParsedItem) -> (Arena, NodeRef, HashMap<usize, usize>) {
    let container_item = item.clone();
    let block_ends = Rc::new(RefCell::new(HashMap::new()));
    let tracked_ends = Rc::clone(&block_ends);
    let quote_ends = Rc::clone(&block_ends);
    let extension = ParserExtensionFn::new(move |parser: &mut Parser| {
        parser.add_block_parser(
            move || BlockParserWithSpans {
                parser: parser::FencedCodeBlockParser::new().into(),
                ends: Rc::clone(&tracked_ends),
            },
            parser::NoParserOptions,
            // Rushdown 0.18 registers its default fence parser at the
            // indented-code priority; run the tracking delegate first.
            parser::PRIORITY_INDENTED_CODE_BLOCK - 1,
        );
        parser.add_block_parser(
            move || BlockParserWithSpans {
                parser: parser::BlockquoteParser::new().into(),
                ends: Rc::clone(&quote_ends),
            },
            parser::NoParserOptions,
            parser::PRIORITY_BLOCKQUOTE - 1,
        );
        parser.add_block_parser(
            move || MaraItemParser {
                item: container_item.clone(),
            },
            parser::NoParserOptions,
            super::DELIMITER_BLOCK_PRIORITY,
        );
        parser::gfm_table().apply(parser);
    });
    let parser = Parser::with_extensions(parser::Options::default(), extension);
    // Retain document-relative byte offsets, including UTF-8 and CRLF. Start
    // at the already recognized opener and stop after its closing delimiter.
    let mut reader = BasicReader::new(&source[..item.source.end]);
    let first_line_end = source[item.source.clone()]
        .find('\n')
        .map_or(item.source.end, |offset| item.source.start + offset + 1);
    reader.set_position(0, Segment::new(item.source.start, first_line_end));
    let (arena, root) = parser.parse(&mut reader);
    let ends = block_ends.take();
    (arena, root, ends)
}

pub(super) fn populate(source: &str, document: &mut ParsedDocument) {
    for item in &mut document.items {
        // Validation recovery retains partial identities and metadata. It
        // must not present a malformed item's body as trustworthy structure.
        if !item.body_valid || !item.title_valid {
            continue;
        }
        let (arena, root, block_ends) = item_tree(source, item);
        let container = arena[root]
            .first_child()
            .expect("recognized Mara item container");
        debug_assert_eq!(arena[container].kind_data().kind_name(), "MaraItem");
        *item = rushdown::as_extension_data!(arena, container, MaraItemNode)
            .item
            .clone();
        item.blocks = project_children(&arena, container, source, item.body.clone(), &block_ends);
    }
}

fn block_kind(kind: &KindData) -> Option<MarkdownBlockKind> {
    Some(match kind {
        KindData::Paragraph(_) => MarkdownBlockKind::Paragraph,
        KindData::Heading(heading) => MarkdownBlockKind::Heading {
            level: heading.level(),
        },
        KindData::ThematicBreak(_) => MarkdownBlockKind::ThematicBreak,
        KindData::CodeBlock(_) => MarkdownBlockKind::CodeBlock,
        KindData::Blockquote(_) => MarkdownBlockKind::Blockquote,
        KindData::List(_) => MarkdownBlockKind::List,
        KindData::ListItem(_) => MarkdownBlockKind::ListItem,
        KindData::HtmlBlock(_) => MarkdownBlockKind::HtmlBlock,
        KindData::LinkReferenceDefinition(_) => MarkdownBlockKind::LinkReferenceDefinition,
        KindData::Table(_) => MarkdownBlockKind::Table,
        KindData::TableHeader(_) => MarkdownBlockKind::TableHeader,
        KindData::TableBody(_) => MarkdownBlockKind::TableBody,
        KindData::TableRow(_) => MarkdownBlockKind::TableRow,
        KindData::TableCell(_) => MarkdownBlockKind::TableCell,
        _ => return None,
    })
}

fn node_start(arena: &Arena, node: NodeRef) -> Option<usize> {
    if let KindData::CodeBlock(block) = arena[node].kind_data()
        && block.code_block_kind() == CodeBlockKind::Indented
        && let Lines::Segments(segments) = block.value()
        && let Some(first) = segments.first()
    {
        // Tab padding can displace the node position into a UTF-8 character;
        // indented code retains the actual content's byte position.
        return Some(first.start());
    }
    let content_start = match arena[node].type_data() {
        TypeData::Block(block) => block.source().first().map(Segment::start),
        _ => None,
    };
    // Paragraph transformers can extract leading reference definitions without
    // updating the original node position. Only its remaining source belongs
    // to the paragraph; other block positions retain authored opening markers.
    let own = if matches!(arena[node].kind_data(), KindData::Paragraph(_)) {
        content_start.or(arena[node].pos())
    } else {
        arena[node].pos().or(content_start)
    };
    own.or_else(|| {
        arena[node]
            .children(arena)
            .find_map(|child| node_start(arena, child))
    })
}

fn line_end(source: &str, start: usize, limit: usize) -> usize {
    // A probe just before an exclusive content end can be inside a UTF-8
    // character. Scan bytes; the returned newline boundary remains valid UTF-8.
    source.as_bytes()[start..limit]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(limit, |offset| start + offset + 1)
}

fn leaf_end(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    start: usize,
    limit: usize,
) -> Option<usize> {
    match arena[node].kind_data() {
        // These parsers retain their complete content lines, with enclosing
        // quote separators already excluded. Keep literal `>` and blank lines
        // that belong inside the block instead of trimming physical source.
        KindData::HtmlBlock(block) => parsed_lines_end(block.value()),
        KindData::CodeBlock(block) if block.code_block_kind() == CodeBlockKind::Indented => {
            parsed_lines_end(block.value())
        }
        KindData::Heading(heading) => {
            let mut end = line_end(source, start, limit);
            if heading.heading_kind() == HeadingKind::Setext {
                if let TypeData::Block(block) = arena[node].type_data()
                    && let Some(last) = block.source().last()
                {
                    end = line_end(source, last.stop().saturating_sub(1).max(start), limit);
                }
                end = line_end(source, end, limit);
            }
            Some(end)
        }
        KindData::ThematicBreak(_) => Some(line_end(source, start, limit)),
        KindData::LinkReferenceDefinition(definition) => {
            let destination_end = match definition.destination() {
                Value::Index(index) => index.stop(),
                _ => return None,
            };
            let content_end = match definition.title() {
                Some(MultilineValue::Indices(indices)) => indices
                    .iter()
                    .last()
                    .map_or(destination_end, |index| index.stop()),
                _ => destination_end,
            };
            Some(line_end(source, content_end, limit))
        }
        _ => None,
    }
}

fn parsed_lines_end(lines: &Lines) -> Option<usize> {
    match lines {
        Lines::Segments(segments) => segments.last().map(Segment::stop),
        _ => None,
    }
}

fn table_span(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    scope: Range<usize>,
) -> Option<Range<usize>> {
    let span = match arena[node].kind_data() {
        KindData::TableCell(_) => {
            let TypeData::Block(block) = arena[node].type_data() else {
                return None;
            };
            if let (Some(first), Some(last)) = (block.source().first(), block.source().last()) {
                // Cell positions can point at a preceding pipe (or the last
                // byte of a quote prefix). Content segments are exact UTF-8
                // bounds and exclude the row's structural separators.
                Some(first.start()..last.stop())
            } else {
                // Rushdown pads short rows with cells having no source. Keep
                // them at the last authored cell's content end, before any
                // trailing pipe or whitespace. Multiple padded cells share it.
                let end = arena[arena[node].parent()?]
                    .children(arena)
                    .filter_map(|cell| match arena[cell].type_data() {
                        TypeData::Block(block) => block.source().last().map(Segment::stop),
                        _ => None,
                    })
                    .next_back()
                    .unwrap_or(scope.start);
                Some(end..end)
            }
        }
        KindData::TableRow(_) => {
            let start = node_start(arena, node)?;
            if !scope.contains(&start) {
                return None;
            }
            Some(start..line_end(source, start, source.len()))
        }
        KindData::TableHeader(_) | KindData::TableBody(_) => {
            let first = table_span(arena, arena[node].first_child()?, source, scope.clone())?;
            let last = table_span(arena, arena[node].last_child()?, source, scope.clone())?;
            Some(first.start..last.end)
        }
        KindData::Table(_) => {
            let header = table_span(arena, arena[node].first_child()?, source, scope.clone())?;
            let last = arena[node].last_child()?;
            let end = if matches!(arena[last].kind_data(), KindData::TableBody(_)) {
                table_span(arena, last, source, scope.clone())?.end
            } else {
                // A header-only table still owns its following delimiter row.
                if header.end >= scope.end {
                    return None;
                }
                line_end(source, header.end, source.len())
            };
            Some(header.start..end)
        }
        _ => None,
    }?;
    (span.start >= scope.start && span.end <= scope.end && source.get(span.clone()).is_some())
        .then_some(span)
}

fn project_children(
    arena: &Arena,
    parent: NodeRef,
    source: &str,
    scope: Range<usize>,
    block_ends: &HashMap<usize, usize>,
) -> Vec<ParsedBlock> {
    let children = arena[parent]
        .children(arena)
        .filter_map(|child| block_kind(arena[child].kind_data()).map(|kind| (child, kind)))
        .collect::<Vec<_>>();
    children
        .iter()
        .enumerate()
        .filter_map(|(index, &(child, kind))| {
            let sibling_limit = children
                .get(index + 1)
                .and_then(|&(next, _)| node_start(arena, next))
                .unwrap_or(scope.end)
                .clamp(scope.start, scope.end);
            if matches!(
                kind,
                MarkdownBlockKind::Table
                    | MarkdownBlockKind::TableHeader
                    | MarkdownBlockKind::TableBody
                    | MarkdownBlockKind::TableRow
                    | MarkdownBlockKind::TableCell
            ) {
                let span = table_span(arena, child, source, scope.start..sibling_limit)?;
                return Some(ParsedBlock {
                    kind,
                    children: project_children(arena, child, source, span.clone(), block_ends),
                    source: span,
                });
            }
            let start = node_start(arena, child).unwrap_or(scope.start);
            // Tab padding can give an empty Rushdown child a position beyond
            // its parent's source. Do not move it onto another block's bytes.
            if !scope.contains(&start) || !source.is_char_boundary(start) {
                return None;
            }
            let limit = sibling_limit.max(start);
            // A sibling in a quote/list may start after its line's prefix.
            // That prefix does not belong to the preceding block.
            let next_line_start = source.as_bytes()[..limit]
                .iter()
                .rposition(|&byte| byte == b'\n')
                .map_or(0, |pos| pos + 1);
            let limit = if next_line_start > start {
                next_line_start
            } else {
                limit
            };
            if !source.is_char_boundary(limit) {
                return None;
            }
            if matches!(
                kind,
                MarkdownBlockKind::Blockquote
                    | MarkdownBlockKind::List
                    | MarkdownBlockKind::ListItem
            ) {
                // Lists own their item markers and children, not trailing blank
                // lines from an enclosing quote. Quotes additionally own each
                // explicitly consumed `>` line, even when it has no children.
                let nested = project_children(arena, child, source, start..limit, block_ends);
                let own_end = block_ends
                    .get(&start)
                    .copied()
                    .unwrap_or_else(|| line_end(source, start, limit))
                    .clamp(start, limit);
                // Lazy paragraph continuations may extend past the last
                // explicit quote marker, so retain the children's full extent.
                let end = nested
                    .last()
                    .map_or(own_end, |last| own_end.max(last.source.end));
                return Some(ParsedBlock {
                    kind,
                    children: nested,
                    source: start..end,
                });
            }
            // Include authored block markers and closing fences, but not blank
            // separator lines. Never serialize the AST to reconstruct source.
            let mut end = block_ends
                .get(&start)
                .copied()
                .or_else(|| leaf_end(arena, child, source, start, limit))
                .unwrap_or_else(|| {
                    source_lines(&source[start..limit])
                        .iter()
                        .rev()
                        .find(|line| !line.text.trim().is_empty())
                        .map_or(start, |line| start + line.full_end)
                })
                .clamp(start, limit);
            if kind == MarkdownBlockKind::Paragraph
                && let TypeData::Block(block) = arena[child].type_data()
                && let Some(last) = block.source().last()
            {
                // Rushdown knows the paragraph's last content line, even
                // when a following blank quote line still contains `>`.
                let content_end = last.stop().clamp(start, end);
                end = source[content_end..end]
                    .find('\n')
                    .map_or(end, |offset| content_end + offset + 1);
            }
            let span = start..end;
            Some(ParsedBlock {
                kind,
                children: project_children(arena, child, source, span.clone(), block_ends),
                source: span,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rushdown_item_is_a_container_with_only_markdown_body_children() {
        let source =
            "Prelude.\n\n:::mara requirement REQ-ONE\n:title: One\n\n# Heading\n\nBody.\n:::\n";
        let parsed = super::super::parse(source).unwrap();
        let (arena, root, _) = item_tree(source, &parsed.items[0]);
        let container = arena[root].first_child().unwrap();
        assert_eq!(arena[container].kind_data().typ(), NodeType::ContainerBlock);
        assert_eq!(arena[container].kind_data().kind_name(), "MaraItem");
        let children = arena[container]
            .children(&arena)
            .map(|child| arena[child].kind_data().kind_name())
            .collect::<Vec<_>>();
        assert_eq!(children, ["Heading", "Paragraph"]);
        assert!(arena[container].next_sibling().is_none());
    }
}
