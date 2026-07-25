//! Rushdown-backed Markdown parser-adapter boundary for Mara.

use std::{
    cell::RefCell,
    fmt::{self, Write},
    rc::Rc,
};

use mara_core::{Diagnostic, Mid, MidIdentity, SourceDocument, SourceSpan, SyntaxDiagnosticCode};
use rushdown::{
    ast::{Arena, KindData, NodeKind, NodeRef, NodeType, PrettyPrint},
    context::{ContextKey, ContextKeyRegistry, UsizeValue},
    parser::{
        self, AnyBlockParser, BlockParser, NoParserOptions, PRIORITY_FENCED_CODE_BLOCK, Parser,
        ParserExtension, ParserExtensionFn,
    },
    text::{self, BasicReader, Reader as _, Segment},
};

const ITEM_DEPTH_CONTEXT: &str = "mara-item-depth";
const ITEM_PARSER_PRIORITY: u32 = PRIORITY_FENCED_CODE_BLOCK + 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDocument {
    source: SourceDocument,
    blocks: Vec<ParsedBlock>,
    diagnostics: Vec<Diagnostic>,
}

impl ParsedDocument {
    pub const fn source(&self) -> &SourceDocument {
        &self.source
    }

    pub fn blocks(&self) -> &[ParsedBlock] {
        &self.blocks
    }

    pub fn items(&self) -> impl Iterator<Item = &ParsedItem> {
        self.blocks.iter().filter_map(ParsedBlock::as_item)
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedBlock {
    Markdown(MarkdownSegment),
    Item(Box<ParsedItem>),
}

impl ParsedBlock {
    pub const fn as_markdown(&self) -> Option<&MarkdownSegment> {
        match self {
            Self::Markdown(markdown) => Some(markdown),
            Self::Item(_) => None,
        }
    }

    pub fn as_item(&self) -> Option<&ParsedItem> {
        match self {
            Self::Markdown(_) => None,
            Self::Item(item) => Some(item.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownSegment {
    raw: String,
    source: SourceSpan,
}

impl MarkdownSegment {
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedItem {
    flavour: String,
    mid: Mid,
    metadata: Vec<ParsedMetadataEntry>,
    body_markdown: String,
    source: SourceSpan,
    header_source: SourceSpan,
    body_source: SourceSpan,
}

impl ParsedItem {
    pub fn flavour(&self) -> &str {
        &self.flavour
    }

    pub const fn mid(&self) -> &Mid {
        &self.mid
    }

    pub fn metadata(&self) -> &[ParsedMetadataEntry] {
        &self.metadata
    }

    pub fn body_markdown(&self) -> &str {
        &self.body_markdown
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn header_source(&self) -> &SourceSpan {
        &self.header_source
    }

    pub const fn body_source(&self) -> &SourceSpan {
        &self.body_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMetadataEntry {
    key: String,
    raw_value: String,
    value: String,
    source: SourceSpan,
}

impl ParsedMetadataEntry {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn raw_value(&self) -> &str {
        &self.raw_value
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

/// Parses one complete source document through Rushdown and converts the
/// adapter AST immediately into Mara-owned structural values.
pub fn parse_document(source: SourceDocument, identity: &MidIdentity) -> ParsedDocument {
    let parser = Parser::with_extensions(parser::Options::default(), mara_item_extension());
    let mut reader = BasicReader::new(source.source().as_str());
    let (arena, document_ref) = parser.parse(&mut reader);

    let mut diagnostics = Vec::new();
    let mut items = Vec::new();
    for node_ref in arena[document_ref].children(&arena) {
        if arena[node_ref].kind_data().kind_name() != "MaraItemBlock" {
            continue;
        }
        let node = rushdown::as_extension_data!(arena, node_ref, MaraItemBlockNode);
        if let Some(item) = convert_item(&source, identity, node, &mut diagnostics) {
            items.push(item);
        }
    }

    let blocks = interleave_markdown(&source, items);
    ParsedDocument {
        source,
        blocks,
        diagnostics,
    }
}

#[derive(Debug)]
struct MaraItemBlockNode {
    opening: Segment,
    closing: Option<Segment>,
    nested_openings: Vec<text::Index>,
}

impl MaraItemBlockNode {
    fn new(opening: Segment) -> Self {
        Self {
            opening,
            closing: None,
            nested_openings: Vec::new(),
        }
    }
}

impl NodeKind for MaraItemBlockNode {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "MaraItemBlock"
    }
}

impl PrettyPrint for MaraItemBlockNode {
    fn pretty_print(&self, writer: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(writer, "{}MaraItemBlock", "  ".repeat(level))
    }
}

impl From<MaraItemBlockNode> for KindData {
    fn from(node: MaraItemBlockNode) -> Self {
        Self::Extension(Box::new(node))
    }
}

#[derive(Debug)]
struct MaraItemBlockParser {
    item_depth: ContextKey<UsizeValue>,
}

impl MaraItemBlockParser {
    fn new(registry: Rc<RefCell<ContextKeyRegistry>>) -> Self {
        let item_depth = registry
            .borrow_mut()
            .get_or_create::<UsizeValue>(ITEM_DEPTH_CONTEXT);
        Self { item_depth }
    }
}

impl BlockParser for MaraItemBlockParser {
    fn trigger(&self) -> &[u8] {
        b":"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut BasicReader,
        context: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        if context.get(self.item_depth).copied().unwrap_or(0) != 0 {
            return None;
        }
        let segment = reader.peek_line_segment()?;
        if !is_item_candidate(segment.bytes(reader.source()).as_ref()) {
            return None;
        }

        let node_ref = arena.new_node(MaraItemBlockNode::new(segment));
        context.insert(self.item_depth, 1);
        reader.advance_to_eol();
        Some((node_ref, parser::State::HAS_CHILDREN))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut BasicReader,
        context: &mut parser::Context,
    ) -> Option<parser::State> {
        if context.last_opened_block().is_some_and(|last| {
            last != node_ref
                && matches!(
                    arena[last].kind_data(),
                    KindData::CodeBlock(_) | KindData::HtmlBlock(_)
                )
        }) {
            return Some(parser::State::HAS_CHILDREN);
        }

        let segment = reader.peek_line_segment()?;
        let line = segment.bytes(reader.source());
        if is_exact_closer(line.as_ref()) {
            rushdown::as_extension_data_mut!(arena, node_ref, MaraItemBlockNode).closing =
                Some(segment);
            reader.advance_to_eol();
            return None;
        }
        if is_item_candidate(line.as_ref()) {
            rushdown::as_extension_data_mut!(arena, node_ref, MaraItemBlockNode)
                .nested_openings
                .push(line_content_index(segment, reader.source()));
        }
        Some(parser::State::HAS_CHILDREN)
    }

    fn close(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        _reader: &mut BasicReader,
        context: &mut parser::Context,
    ) {
        context.insert(self.item_depth, 0);
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

impl From<MaraItemBlockParser> for AnyBlockParser {
    fn from(parser: MaraItemBlockParser) -> Self {
        Self::Extension(Box::new(parser))
    }
}

fn mara_item_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|parser: &mut Parser| {
        parser.add_block_parser(
            MaraItemBlockParser::new,
            NoParserOptions,
            ITEM_PARSER_PRIORITY,
        );
    })
}

fn convert_item(
    document: &SourceDocument,
    identity: &MidIdentity,
    node: &MaraItemBlockNode,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ParsedItem> {
    let source = document.source().as_str();
    let header_index = line_content_index(node.opening, source);
    let header_span = span(document, header_index.start(), header_index.stop());
    let (flavour, mid) = match parse_header(header_index.str(source), identity) {
        Ok(header) => header,
        Err(reason) => {
            diagnostics.push(
                Diagnostic::new(
                    SyntaxDiagnosticCode::InvalidItemHeader,
                    "item header must match ':::<flavour> <mid>'",
                    Some(header_span),
                )
                .with_detail("reason", reason),
            );
            return None;
        }
    };

    let Some(closing) = node.closing else {
        diagnostics.push(Diagnostic::new(
            SyntaxDiagnosticCode::UnclosedItem,
            "item has no exact standalone closing marker",
            Some(header_span),
        ));
        return None;
    };

    for nested in &node.nested_openings {
        if parse_header(nested.str(source), identity).is_ok() {
            diagnostics.push(Diagnostic::new(
                SyntaxDiagnosticCode::InvalidItemHeader,
                "Mara item blocks cannot be nested",
                Some(span(document, nested.start(), nested.stop())),
            ));
        }
    }

    let closing_index = line_content_index(closing, source);
    let (metadata, body_start) = parse_metadata(
        document,
        node.opening.stop(),
        closing_index.start(),
        diagnostics,
    )?;
    let body_end = closing_index.start();
    let body_span = span(document, body_start, body_end);
    let item_span = span(document, node.opening.start(), closing_index.stop());

    Some(ParsedItem {
        flavour,
        mid,
        metadata,
        body_markdown: source[body_start..body_end].to_owned(),
        source: item_span,
        header_source: header_span,
        body_source: body_span,
    })
}

fn parse_header(line: &str, identity: &MidIdentity) -> Result<(String, Mid), String> {
    let rest = line
        .strip_prefix(":::")
        .ok_or_else(|| "missing opening marker".to_owned())?;
    let (flavour, mid_text) = rest
        .split_once(' ')
        .ok_or_else(|| "missing MID token".to_owned())?;
    if !valid_snake_name(flavour) {
        return Err("flavour must be a lowercase ASCII snake name".to_owned());
    }
    if mid_text.is_empty() || mid_text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err("header must contain exactly one flavour and one MID token".to_owned());
    }
    let mid = Mid::parse(mid_text, identity).map_err(|error| error.to_string())?;
    Ok((flavour.to_owned(), mid))
}

fn parse_metadata(
    document: &SourceDocument,
    start: usize,
    end: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Vec<ParsedMetadataEntry>, usize)> {
    let source = document.source().as_str();
    let mut entries = Vec::new();
    let mut cursor = start;
    let mut valid = true;

    while cursor < end {
        let line = next_line(source, cursor, end);
        let content = line_content_index(line, source);
        let text = content.str(source);
        if text.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
            return valid.then_some((entries, line.stop()));
        }

        match parse_metadata_line(text) {
            Ok((key, raw_value, value)) => entries.push(ParsedMetadataEntry {
                key,
                raw_value,
                value,
                source: span(document, content.start(), content.stop()),
            }),
            Err(reason) => {
                valid = false;
                diagnostics.push(
                    Diagnostic::new(
                        SyntaxDiagnosticCode::InvalidMetadata,
                        "metadata line must match ':<key>: <value>'",
                        Some(span(document, content.start(), content.stop())),
                    )
                    .with_detail("reason", reason),
                );
            }
        }
        cursor = line.stop();
    }

    diagnostics.push(Diagnostic::new(
        SyntaxDiagnosticCode::InvalidMetadata,
        "item metadata must be followed by a blank line before the body",
        Some(span(document, end, end)),
    ));
    None
}

fn parse_metadata_line(line: &str) -> Result<(String, String, String), String> {
    let rest = line
        .strip_prefix(':')
        .ok_or_else(|| "metadata must begin at byte column zero with ':'".to_owned())?;
    let (key, raw_value) = rest
        .split_once(':')
        .ok_or_else(|| "metadata key is missing its closing ':' delimiter".to_owned())?;
    if !valid_snake_name(key) {
        return Err("metadata key must be a lowercase ASCII snake name".to_owned());
    }
    let value = raw_value.trim_matches([' ', '\t']).to_owned();
    Ok((key.to_owned(), raw_value.to_owned(), value))
}

fn interleave_markdown(document: &SourceDocument, items: Vec<ParsedItem>) -> Vec<ParsedBlock> {
    let source = document.source().as_str();
    let mut blocks = Vec::new();
    let mut cursor = 0;
    for item in items {
        let item_start = item.source().start_byte() as usize;
        if cursor < item_start {
            blocks.push(markdown_segment(document, cursor, item_start));
        }
        cursor = item.source().end_byte() as usize;
        blocks.push(ParsedBlock::Item(Box::new(item)));
    }
    if cursor < source.len() {
        blocks.push(markdown_segment(document, cursor, source.len()));
    }
    blocks
}

fn markdown_segment(document: &SourceDocument, start: usize, end: usize) -> ParsedBlock {
    ParsedBlock::Markdown(MarkdownSegment {
        raw: document.source().as_str()[start..end].to_owned(),
        source: span(document, start, end),
    })
}

fn span(document: &SourceDocument, start: usize, end: usize) -> SourceSpan {
    let index = document.source_index();
    let (start_line, start_column) = index
        .coordinates_at(start as u64)
        .expect("Rushdown returned a legal source boundary");
    let (end_line, end_column) = index
        .coordinates_at(end as u64)
        .expect("Rushdown returned a legal source boundary");
    index
        .try_span(
            start as u64,
            end as u64,
            start_line,
            start_column,
            end_line,
            end_column,
        )
        .expect("Rushdown returned a valid source range")
}

fn next_line(source: &str, start: usize, end: usize) -> Segment {
    let stop = source.as_bytes()[start..end]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(end, |offset| start + offset + 1);
    Segment::new(start, stop)
}

fn line_content_index(segment: Segment, source: &str) -> text::Index {
    let bytes = segment.bytes(source);
    let mut stop = segment.stop();
    if bytes.ends_with(b"\n") {
        stop -= 1;
        if bytes.len() >= 2 && bytes[bytes.len() - 2] == b'\r' {
            stop -= 1;
        }
    } else if bytes.ends_with(b"\r") {
        stop -= 1;
    }
    text::Index::new(segment.start(), stop)
}

fn is_item_candidate(line: &[u8]) -> bool {
    let line = without_line_ending(line);
    line.len() > 3
        && line.starts_with(b":::")
        && line[3].is_ascii_lowercase()
        && !line.starts_with(b"::::")
}

fn is_exact_closer(line: &[u8]) -> bool {
    without_line_ending(line) == b":::"
}

fn without_line_ending(mut line: &[u8]) -> &[u8] {
    if line.ends_with(b"\n") {
        line = &line[..line.len() - 1];
        if line.ends_with(b"\r") {
            line = &line[..line.len() - 1];
        }
    } else if line.ends_with(b"\r") {
        line = &line[..line.len() - 1];
    }
    line
}

fn valid_snake_name(value: &str) -> bool {
    let mut segments = value.split('_');
    let Some(first) = segments.next() else {
        return false;
    };
    valid_name_segment(first, true) && segments.all(|segment| valid_name_segment(segment, false))
}

fn valid_name_segment(value: &str, require_letter_first: bool) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    ((!require_letter_first && first.is_ascii_digit()) || first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}
