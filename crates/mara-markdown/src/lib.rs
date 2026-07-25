//! Rushdown-backed Markdown parser-adapter boundary for Mara.

use std::{
    cell::RefCell,
    fmt::{self, Write},
    rc::Rc,
};

use mara_core::{
    Diagnostic, IdentityDiagnosticCode, Mid, MidIdentity, MidParseError, SourceDocument,
    SourceSpan, SyntaxDiagnosticCode, sort_diagnostics,
};
use rushdown::{
    ast::{
        Arena, CodeBlockKind, HeadingKind, KindData, LinkKind, LinkReferenceKind, NodeKind,
        NodeRef, NodeType, PrettyPrint, TableCellAlignment, Task, TextQualifier,
    },
    context::{ContextKey, ContextKeyRegistry, UsizeValue},
    parser::{
        self, AnyBlockParser, AnyInlineParser, BlockParser, InlineParser,
        PRIORITY_FENCED_CODE_BLOCK, PRIORITY_LINK, Parser, ParserExtension, ParserExtensionFn,
        ParserOptions,
    },
    text::{self, BasicReader, Reader as _, Segment},
};

const ITEM_DEPTH_CONTEXT: &str = "mara-item-depth";
const ITEM_PARSER_PRIORITY: u32 = PRIORITY_FENCED_CODE_BLOCK + 50;
const INLINE_REFERENCE_PARSER_PRIORITY: u32 = PRIORITY_LINK - 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDocument {
    source: SourceDocument,
    blocks: Vec<ParsedBlock>,
    preamble_end: usize,
    sections: Vec<ParsedSection>,
    diagnostics: Vec<Diagnostic>,
}

impl ParsedDocument {
    pub const fn source(&self) -> &SourceDocument {
        &self.source
    }

    pub fn blocks(&self) -> &[ParsedBlock] {
        &self.blocks
    }

    pub fn preamble(&self) -> &[ParsedBlock] {
        &self.blocks[..self.preamble_end]
    }

    pub fn sections(&self) -> &[ParsedSection] {
        &self.sections
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
pub struct ParsedSection {
    level: u8,
    title: String,
    source: SourceSpan,
    heading_source: SourceSpan,
    heading_block: usize,
    content_start: usize,
    content_end: usize,
    children: Vec<ParsedSection>,
}

impl ParsedSection {
    pub const fn level(&self) -> u8 {
        self.level
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn heading_source(&self) -> &SourceSpan {
        &self.heading_source
    }

    pub const fn heading_block(&self) -> usize {
        self.heading_block
    }

    pub fn content_range(&self) -> std::ops::Range<usize> {
        self.content_start..self.content_end
    }

    pub fn children(&self) -> &[ParsedSection] {
        &self.children
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
    kind: NarrativeKind,
    heading_level: Option<u8>,
    heading_kind: Option<ParsedHeadingKind>,
    heading_title: Option<String>,
    references: Vec<InlineReference>,
    structure: Box<MarkdownNode>,
}

impl MarkdownSegment {
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn kind(&self) -> NarrativeKind {
        self.kind
    }

    pub const fn heading_level(&self) -> Option<u8> {
        self.heading_level
    }

    pub const fn heading_kind(&self) -> Option<ParsedHeadingKind> {
        self.heading_kind
    }

    pub fn heading_title(&self) -> Option<&str> {
        self.heading_title.as_deref()
    }

    pub fn references(&self) -> &[InlineReference] {
        &self.references
    }

    pub fn structure(&self) -> &MarkdownNode {
        self.structure.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedHeadingKind {
    Atx,
    Setext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrativeKind {
    Paragraph,
    Heading,
    List,
    Quote,
    Code,
    Table,
    ThematicBreak,
    Html,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownNode {
    kind: MarkdownNodeKind,
    source: SourceSpan,
    payload: MarkdownNodePayload,
    children: Vec<MarkdownNode>,
}

impl MarkdownNode {
    pub const fn kind(&self) -> MarkdownNodeKind {
        self.kind
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn payload(&self) -> &MarkdownNodePayload {
        &self.payload
    }

    pub fn children(&self) -> &[MarkdownNode] {
        &self.children
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownNodePayload {
    None,
    Heading {
        level: u8,
        kind: ParsedHeadingKind,
    },
    CodeBlock {
        kind: MarkdownCodeBlockKind,
        info: Option<String>,
        value: String,
    },
    List {
        marker: char,
        start: u32,
        tight: bool,
    },
    ListItem {
        task: Option<MarkdownTaskState>,
    },
    Text {
        value: String,
        soft_break: bool,
        hard_break: bool,
    },
    CodeSpan {
        value: String,
    },
    Link {
        destination: String,
        title: Option<String>,
        kind: MarkdownLinkKind,
    },
    Image {
        destination: String,
        title: Option<String>,
        kind: MarkdownLinkKind,
    },
    Html {
        value: String,
    },
    LinkReferenceDefinition {
        label: String,
        destination: String,
        title: Option<String>,
    },
    TableCell {
        alignment: MarkdownTableAlignment,
    },
    InlineReference {
        target: String,
        label: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownCodeBlockKind {
    Indented,
    Fenced,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownTaskState {
    Active,
    Completed,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownLinkKind {
    Inline,
    Reference {
        label: String,
        kind: MarkdownLinkReferenceKind,
    },
    Auto {
        text: String,
    },
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownLinkReferenceKind {
    Full,
    Collapsed,
    Shortcut,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownTableAlignment {
    Left,
    Center,
    Right,
    None,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownNodeKind {
    Paragraph,
    Heading,
    ThematicBreak,
    CodeBlock,
    Blockquote,
    List,
    ListItem,
    HtmlBlock,
    Text,
    CodeSpan,
    Emphasis,
    Strong,
    Link,
    Image,
    RawHtml,
    LinkReferenceDefinition,
    Table,
    TableHeader,
    TableBody,
    TableRow,
    TableCell,
    Strikethrough,
    InlineReference,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineReferenceContext {
    Text,
    Heading,
    ListItem,
    TableCell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineReference {
    target: String,
    label: Option<String>,
    context: InlineReferenceContext,
    source: SourceSpan,
}

impl InlineReference {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub const fn context(&self) -> InlineReferenceContext {
        self.context
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
    references: Vec<InlineReference>,
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

    pub fn references(&self) -> &[InlineReference] {
        &self.references
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
    let parser = Parser::with_extensions(
        parser::Options::default(),
        mara_item_extension(identity.clone()).and(parser::gfm_table()),
    );
    let mut reader = BasicReader::new(source.source().as_str());
    let (arena, document_ref) = parser.parse(&mut reader);

    let mut diagnostics = Vec::new();
    let mut events = Vec::new();
    for node_ref in arena[document_ref].children(&arena) {
        if arena[node_ref].kind_data().kind_name() != "MaraItemBlock" {
            if let Some(start) = arena[node_ref].pos() {
                events.push(DocumentEvent::Markdown { node_ref, start });
            }
            continue;
        }
        let node = rushdown::as_extension_data!(arena, node_ref, MaraItemBlockNode);
        if let Some(mut item) = convert_item(&source, identity, node, &mut diagnostics) {
            item.references = collect_inline_references(
                &arena,
                node_ref,
                &source,
                Some((
                    item.body_source.start_byte() as usize,
                    item.body_source.end_byte() as usize,
                )),
            );
            events.push(DocumentEvent::Item(Box::new(item)));
        } else {
            events.push(DocumentEvent::Markdown {
                node_ref,
                start: node.opening.start(),
            });
        }
    }
    sort_diagnostics(&mut diagnostics);

    let blocks = convert_document_blocks(&source, &arena, events);
    let (preamble_end, sections) = build_sections(&source, &blocks);
    ParsedDocument {
        source,
        blocks,
        preamble_end,
        sections,
        diagnostics,
    }
}

enum DocumentEvent {
    Markdown { node_ref: NodeRef, start: usize },
    Item(Box<ParsedItem>),
}

impl DocumentEvent {
    fn start(&self) -> usize {
        match self {
            Self::Markdown { start, .. } => *start,
            Self::Item(item) => item.source.start_byte() as usize,
        }
    }
}

#[derive(Debug)]
struct MaraItemBlockNode {
    opening: Segment,
    closing: Option<Segment>,
    nested_openings: Vec<text::Index>,
}

#[derive(Debug)]
struct MaraInlineReferenceNode {
    index: text::Index,
    target: String,
    label: Option<String>,
}

impl NodeKind for MaraInlineReferenceNode {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }

    fn kind_name(&self) -> &'static str {
        "MaraInlineReference"
    }
}

impl PrettyPrint for MaraInlineReferenceNode {
    fn pretty_print(&self, writer: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(writer, "{}MaraInlineReference", "  ".repeat(level))
    }
}

impl From<MaraInlineReferenceNode> for KindData {
    fn from(node: MaraInlineReferenceNode) -> Self {
        Self::Extension(Box::new(node))
    }
}

#[derive(Debug, Default)]
struct MaraInlineReferenceParser;

impl MaraInlineReferenceParser {
    fn new() -> Self {
        Self
    }
}

impl InlineParser for MaraInlineReferenceParser {
    fn trigger(&self) -> &[u8] {
        b"["
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BlockReader,
        _context: &mut parser::Context,
    ) -> Option<NodeRef> {
        let (line, segment) = reader.peek_line_bytes()?;
        if !line.starts_with(b"[[") || escaped_opening(reader.source().as_bytes(), segment.start())
        {
            return None;
        }
        let closing = line[2..].windows(2).position(|pair| pair == b"]]")? + 2;
        let inner = str::from_utf8(&line[2..closing]).ok()?;
        let (target, label) = split_inline_reference(inner)?;
        let length = closing + 2;
        reader.advance(length);
        Some(arena.new_node(MaraInlineReferenceNode {
            index: text::Index::new(segment.start(), segment.start() + length),
            target: target.to_owned(),
            label: label.map(str::to_owned),
        }))
    }
}

impl From<MaraInlineReferenceParser> for AnyInlineParser {
    fn from(parser: MaraInlineReferenceParser) -> Self {
        Self::Extension(Box::new(parser))
    }
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
    identity: MidIdentity,
}

impl MaraItemBlockParser {
    fn new(options: MaraItemParserOptions, registry: Rc<RefCell<ContextKeyRegistry>>) -> Self {
        let item_depth = registry
            .borrow_mut()
            .get_or_create::<UsizeValue>(ITEM_DEPTH_CONTEXT);
        Self {
            item_depth,
            identity: options.identity,
        }
    }
}

#[derive(Debug)]
struct MaraItemParserOptions {
    identity: MidIdentity,
}

impl ParserOptions for MaraItemParserOptions {}

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
        if !is_item_candidate(
            segment.bytes(reader.source()).as_ref(),
            self.identity.prefix().value(),
        ) {
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
        if is_item_candidate(line.as_ref(), self.identity.prefix().value()) {
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

fn mara_item_extension(identity: MidIdentity) -> impl ParserExtension {
    ParserExtensionFn::new(move |parser: &mut Parser| {
        parser.add_inline_parser(
            MaraInlineReferenceParser::new,
            parser::NoParserOptions,
            INLINE_REFERENCE_PARSER_PRIORITY,
        );
        parser.add_block_parser(
            MaraItemBlockParser::new,
            MaraItemParserOptions { identity },
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
        Err(HeaderError::Syntax(reason)) => {
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
        Err(HeaderError::InvalidMid(error)) => {
            diagnostics.push(
                Diagnostic::new(
                    IdentityDiagnosticCode::InvalidMid,
                    "item header contains an invalid MID",
                    Some(header_span),
                )
                .with_detail("reason", error.to_string()),
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
        references: Vec::new(),
        source: item_span,
        header_source: header_span,
        body_source: body_span,
    })
}

#[derive(Debug)]
enum HeaderError {
    Syntax(&'static str),
    InvalidMid(MidParseError),
}

fn parse_header(line: &str, identity: &MidIdentity) -> Result<(String, Mid), HeaderError> {
    let rest = line
        .strip_prefix(":::")
        .ok_or(HeaderError::Syntax("missing opening marker"))?;
    let (flavour, mid_text) = rest
        .split_once(' ')
        .ok_or(HeaderError::Syntax("missing MID token"))?;
    if !valid_snake_name(flavour) {
        return Err(HeaderError::Syntax(
            "flavour must be a lowercase ASCII snake name",
        ));
    }
    if mid_text.is_empty() || mid_text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(HeaderError::Syntax(
            "header must contain exactly one flavour and one MID token",
        ));
    }
    let mid = Mid::parse(mid_text, identity).map_err(HeaderError::InvalidMid)?;
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

fn convert_document_blocks(
    document: &SourceDocument,
    arena: &Arena,
    events: Vec<DocumentEvent>,
) -> Vec<ParsedBlock> {
    let source = document.source().as_str();
    let mut blocks = Vec::new();
    let mut cursor = 0;
    for (index, event) in events.iter().enumerate() {
        let event_start = event.start();
        if cursor < event_start {
            blocks.push(markdown_segment(
                document,
                cursor,
                event_start,
                MarkdownSegmentData {
                    kind: NarrativeKind::Other,
                    heading_level: None,
                    heading_kind: None,
                    heading_title: None,
                    references: Vec::new(),
                    structure: MarkdownNode {
                        kind: MarkdownNodeKind::Other,
                        source: span(document, cursor, event_start),
                        payload: MarkdownNodePayload::None,
                        children: Vec::new(),
                    },
                },
            ));
        }

        match event {
            DocumentEvent::Item(item) => {
                cursor = item.source().end_byte() as usize;
                blocks.push(ParsedBlock::Item(item.clone()));
            }
            DocumentEvent::Markdown { node_ref, start } => {
                let end = events
                    .get(index + 1)
                    .map_or(source.len(), DocumentEvent::start);
                if *start < end {
                    blocks.push(markdown_from_node(document, arena, *node_ref, *start, end));
                    cursor = end;
                }
            }
        }
    }
    if cursor < source.len() {
        blocks.push(markdown_segment(
            document,
            cursor,
            source.len(),
            MarkdownSegmentData {
                kind: NarrativeKind::Other,
                heading_level: None,
                heading_kind: None,
                heading_title: None,
                references: Vec::new(),
                structure: MarkdownNode {
                    kind: MarkdownNodeKind::Other,
                    source: span(document, cursor, source.len()),
                    payload: MarkdownNodePayload::None,
                    children: Vec::new(),
                },
            },
        ));
    }
    blocks
}

fn markdown_from_node(
    document: &SourceDocument,
    arena: &Arena,
    node_ref: NodeRef,
    start: usize,
    end: usize,
) -> ParsedBlock {
    let (kind, heading_level, heading_kind) = match arena[node_ref].kind_data() {
        KindData::Paragraph(_) => (NarrativeKind::Paragraph, None, None),
        KindData::Heading(heading) => (
            NarrativeKind::Heading,
            Some(heading.level()),
            Some(match heading.heading_kind() {
                HeadingKind::Atx => ParsedHeadingKind::Atx,
                HeadingKind::Setext => ParsedHeadingKind::Setext,
                _ => ParsedHeadingKind::Atx,
            }),
        ),
        KindData::List(_) => (NarrativeKind::List, None, None),
        KindData::Blockquote(_) => (NarrativeKind::Quote, None, None),
        KindData::CodeBlock(_) => (NarrativeKind::Code, None, None),
        KindData::Table(_) => (NarrativeKind::Table, None, None),
        KindData::ThematicBreak(_) => (NarrativeKind::ThematicBreak, None, None),
        KindData::HtmlBlock(_) => (NarrativeKind::Html, None, None),
        _ => (NarrativeKind::Other, None, None),
    };
    let heading_title =
        heading_level.map(|_| plain_text(arena, node_ref, document.source().as_str()));
    let references = collect_inline_references(arena, node_ref, document, Some((start, end)));
    markdown_segment(
        document,
        start,
        end,
        MarkdownSegmentData {
            kind,
            heading_level,
            heading_kind,
            heading_title,
            references,
            structure: convert_markdown_node(document, arena, node_ref, start, end),
        },
    )
}

struct MarkdownSegmentData {
    kind: NarrativeKind,
    heading_level: Option<u8>,
    heading_kind: Option<ParsedHeadingKind>,
    heading_title: Option<String>,
    references: Vec<InlineReference>,
    structure: MarkdownNode,
}

fn markdown_segment(
    document: &SourceDocument,
    start: usize,
    end: usize,
    data: MarkdownSegmentData,
) -> ParsedBlock {
    ParsedBlock::Markdown(MarkdownSegment {
        raw: document.source().as_str()[start..end].to_owned(),
        source: span(document, start, end),
        kind: data.kind,
        heading_level: data.heading_level,
        heading_kind: data.heading_kind,
        heading_title: data.heading_title,
        references: data.references,
        structure: Box::new(data.structure),
    })
}

fn convert_markdown_node(
    document: &SourceDocument,
    arena: &Arena,
    node_ref: NodeRef,
    start: usize,
    end: usize,
) -> MarkdownNode {
    let data = arena[node_ref].kind_data();
    let kind = match data {
        KindData::Paragraph(_) => MarkdownNodeKind::Paragraph,
        KindData::Heading(_) => MarkdownNodeKind::Heading,
        KindData::ThematicBreak(_) => MarkdownNodeKind::ThematicBreak,
        KindData::CodeBlock(_) => MarkdownNodeKind::CodeBlock,
        KindData::Blockquote(_) => MarkdownNodeKind::Blockquote,
        KindData::List(_) => MarkdownNodeKind::List,
        KindData::ListItem(_) => MarkdownNodeKind::ListItem,
        KindData::HtmlBlock(_) => MarkdownNodeKind::HtmlBlock,
        KindData::Text(_) => MarkdownNodeKind::Text,
        KindData::CodeSpan(_) => MarkdownNodeKind::CodeSpan,
        KindData::Emphasis(_) => MarkdownNodeKind::Emphasis,
        KindData::Strong(_) => MarkdownNodeKind::Strong,
        KindData::Link(_) => MarkdownNodeKind::Link,
        KindData::Image(_) => MarkdownNodeKind::Image,
        KindData::RawHtml(_) => MarkdownNodeKind::RawHtml,
        KindData::LinkReferenceDefinition(_) => MarkdownNodeKind::LinkReferenceDefinition,
        KindData::Table(_) => MarkdownNodeKind::Table,
        KindData::TableHeader(_) => MarkdownNodeKind::TableHeader,
        KindData::TableBody(_) => MarkdownNodeKind::TableBody,
        KindData::TableRow(_) => MarkdownNodeKind::TableRow,
        KindData::TableCell(_) => MarkdownNodeKind::TableCell,
        KindData::Strikethrough(_) => MarkdownNodeKind::Strikethrough,
        KindData::Extension(_) if data.kind_name() == "MaraInlineReference" => {
            MarkdownNodeKind::InlineReference
        }
        _ => MarkdownNodeKind::Other,
    };
    let source_text = document.source().as_str();
    let payload = convert_markdown_payload(arena, node_ref, source_text);
    let (source_start, source_end) =
        intrinsic_markdown_node_range(arena, node_ref).unwrap_or((start, end));
    let child_refs = arena[node_ref].children(arena).collect::<Vec<_>>();
    let children = child_refs
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let child_start = markdown_node_start(arena, *child)
                .unwrap_or(start)
                .clamp(start, end);
            let child_end = child_refs[index + 1..]
                .iter()
                .find_map(|next| markdown_node_start(arena, *next))
                .unwrap_or(end)
                .clamp(child_start, end);
            convert_markdown_node(document, arena, *child, child_start, child_end)
        })
        .collect();
    MarkdownNode {
        kind,
        source: span(document, source_start, source_end),
        payload,
        children,
    }
}

fn convert_markdown_payload(arena: &Arena, node_ref: NodeRef, source: &str) -> MarkdownNodePayload {
    match arena[node_ref].kind_data() {
        KindData::Heading(heading) => MarkdownNodePayload::Heading {
            level: heading.level(),
            kind: convert_heading_kind(heading.heading_kind()),
        },
        KindData::CodeBlock(code) => MarkdownNodePayload::CodeBlock {
            kind: match code.code_block_kind() {
                CodeBlockKind::Indented => MarkdownCodeBlockKind::Indented,
                CodeBlockKind::Fenced => MarkdownCodeBlockKind::Fenced,
                _ => MarkdownCodeBlockKind::Other,
            },
            info: code.info_str(source).map(str::to_owned),
            value: lines_text(code.value(), source),
        },
        KindData::List(list) => MarkdownNodePayload::List {
            marker: char::from(list.marker()),
            start: list.start(),
            tight: list.is_tight(),
        },
        KindData::ListItem(item) => MarkdownNodePayload::ListItem {
            task: item.task().map(|task| match task {
                Task::Active => MarkdownTaskState::Active,
                Task::Completed => MarkdownTaskState::Completed,
                _ => MarkdownTaskState::Other,
            }),
        },
        KindData::HtmlBlock(html) => MarkdownNodePayload::Html {
            value: lines_text(html.value(), source),
        },
        KindData::Text(text) => MarkdownNodePayload::Text {
            value: text.str(source).to_owned(),
            soft_break: text.has_qualifiers(TextQualifier::SOFT_LINE_BREAK),
            hard_break: text.has_qualifiers(TextQualifier::HARD_LINE_BREAK),
        },
        KindData::CodeSpan(code) => MarkdownNodePayload::CodeSpan {
            value: code.str(source).into_owned(),
        },
        KindData::Link(link) => MarkdownNodePayload::Link {
            destination: link.destination_str(source).to_owned(),
            title: link.title_str(source).map(|title| title.into_owned()),
            kind: convert_link_kind(link.link_kind(), source),
        },
        KindData::Image(image) => MarkdownNodePayload::Image {
            destination: image.destination_str(source).to_owned(),
            title: image.title_str(source).map(|title| title.into_owned()),
            kind: convert_link_kind(image.link_kind(), source),
        },
        KindData::RawHtml(html) => MarkdownNodePayload::Html {
            value: html.str(source).into_owned(),
        },
        KindData::LinkReferenceDefinition(reference) => {
            MarkdownNodePayload::LinkReferenceDefinition {
                label: reference.label_str(source).into_owned(),
                destination: reference.destination_str(source).to_owned(),
                title: reference.title_str(source).map(|title| title.into_owned()),
            }
        }
        KindData::TableCell(cell) => MarkdownNodePayload::TableCell {
            alignment: match cell.alignment() {
                TableCellAlignment::Left => MarkdownTableAlignment::Left,
                TableCellAlignment::Center => MarkdownTableAlignment::Center,
                TableCellAlignment::Right => MarkdownTableAlignment::Right,
                TableCellAlignment::None => MarkdownTableAlignment::None,
                _ => MarkdownTableAlignment::Other,
            },
        },
        data if data.kind_name() == "MaraInlineReference" => {
            let reference = rushdown::as_extension_data!(arena, node_ref, MaraInlineReferenceNode);
            MarkdownNodePayload::InlineReference {
                target: reference.target.clone(),
                label: reference.label.clone(),
            }
        }
        _ => MarkdownNodePayload::None,
    }
}

fn convert_heading_kind(kind: HeadingKind) -> ParsedHeadingKind {
    match kind {
        HeadingKind::Atx => ParsedHeadingKind::Atx,
        HeadingKind::Setext => ParsedHeadingKind::Setext,
        _ => ParsedHeadingKind::Atx,
    }
}

fn convert_link_kind(kind: &LinkKind, source: &str) -> MarkdownLinkKind {
    match kind {
        LinkKind::Inline => MarkdownLinkKind::Inline,
        LinkKind::Reference(reference) => MarkdownLinkKind::Reference {
            label: reference.value_str(source).into_owned(),
            kind: match reference.link_reference_kind() {
                LinkReferenceKind::Full => MarkdownLinkReferenceKind::Full,
                LinkReferenceKind::Collapsed => MarkdownLinkReferenceKind::Collapsed,
                LinkReferenceKind::Shortcut => MarkdownLinkReferenceKind::Shortcut,
                _ => MarkdownLinkReferenceKind::Other,
            },
        },
        LinkKind::Auto(auto) => MarkdownLinkKind::Auto {
            text: auto.text_str(source).to_owned(),
        },
        _ => MarkdownLinkKind::Other,
    }
}

fn lines_text(lines: &text::Lines, source: &str) -> String {
    let mut value = String::new();
    for line in lines.iter(source) {
        value.push_str(&line);
    }
    value
}

fn markdown_node_start(arena: &Arena, node_ref: NodeRef) -> Option<usize> {
    if arena[node_ref].kind_data().kind_name() == "MaraInlineReference" {
        let reference = rushdown::as_extension_data!(arena, node_ref, MaraInlineReferenceNode);
        return Some(reference.index.start());
    }
    arena[node_ref].pos()
}

fn intrinsic_markdown_node_range(arena: &Arena, node_ref: NodeRef) -> Option<(usize, usize)> {
    match arena[node_ref].kind_data() {
        KindData::Text(text) => text.index().map(|index| (index.start(), index.stop())),
        data if data.kind_name() == "MaraInlineReference" => {
            let reference = rushdown::as_extension_data!(arena, node_ref, MaraInlineReferenceNode);
            Some((reference.index.start(), reference.index.stop()))
        }
        _ => None,
    }
}

fn plain_text(arena: &Arena, node_ref: NodeRef, source: &str) -> String {
    let mut result = String::new();
    append_plain_text(arena, node_ref, source, &mut result);
    result
}

fn append_plain_text(arena: &Arena, node_ref: NodeRef, source: &str, result: &mut String) {
    if let KindData::Text(text) = arena[node_ref].kind_data() {
        result.push_str(text.str(source));
        return;
    }
    if let KindData::CodeSpan(code) = arena[node_ref].kind_data() {
        result.push_str(&code.str(source));
        return;
    }
    if arena[node_ref].kind_data().kind_name() == "MaraInlineReference" {
        let node = rushdown::as_extension_data!(arena, node_ref, MaraInlineReferenceNode);
        result.push_str(node.index.str(source));
        return;
    }
    for child in arena[node_ref].children(arena) {
        append_plain_text(arena, child, source, result);
    }
}

fn collect_inline_references(
    arena: &Arena,
    node_ref: NodeRef,
    document: &SourceDocument,
    bounds: Option<(usize, usize)>,
) -> Vec<InlineReference> {
    let mut references = Vec::new();
    collect_inline_references_from_node(arena, node_ref, document, None, bounds, &mut references);
    references.sort_by_key(|reference| reference.source.start_byte());
    references.dedup_by(|left, right| left.source == right.source);
    references
}

fn collect_inline_references_from_node(
    arena: &Arena,
    node_ref: NodeRef,
    document: &SourceDocument,
    context: Option<InlineReferenceContext>,
    bounds: Option<(usize, usize)>,
    references: &mut Vec<InlineReference>,
) {
    let kind = arena[node_ref].kind_data();
    if matches!(
        kind,
        KindData::CodeBlock(_)
            | KindData::HtmlBlock(_)
            | KindData::CodeSpan(_)
            | KindData::RawHtml(_)
    ) {
        return;
    }

    let context = match kind {
        KindData::Heading(_) => Some(InlineReferenceContext::Heading),
        KindData::TableCell(_) => Some(InlineReferenceContext::TableCell),
        KindData::ListItem(_) => Some(InlineReferenceContext::ListItem),
        KindData::Paragraph(_) => context.or(Some(InlineReferenceContext::Text)),
        _ => context,
    };

    if kind.kind_name() == "MaraInlineReference" {
        let node = rushdown::as_extension_data!(arena, node_ref, MaraInlineReferenceNode);
        let range = (node.index.start(), node.index.stop());
        if let Some(context) = context
            && bounds.is_none_or(|(start, end)| range.0 >= start && range.1 <= end)
        {
            references.push(InlineReference {
                target: node.target.clone(),
                label: node.label.clone(),
                context,
                source: span(document, range.0, range.1),
            });
        }
        return;
    }

    for child in arena[node_ref].children(arena) {
        collect_inline_references_from_node(arena, child, document, context, bounds, references);
    }
}

fn split_inline_reference(inner: &str) -> Option<(&str, Option<&str>)> {
    let (target, label) = inner
        .split_once('|')
        .map_or((inner, None), |(target, label)| (target, Some(label)));
    (!target.is_empty()
        && !target.contains(['[', ']'])
        && label.is_none_or(|label| !label.is_empty() && !label.contains(['[', ']', '|'])))
    .then_some((target, label))
}

fn escaped_opening(source: &[u8], opening: usize) -> bool {
    let mut cursor = opening;
    while cursor > 0 && source[cursor - 1] == b'\\' {
        cursor -= 1;
    }
    (opening - cursor) % 2 == 1
}

fn build_sections(
    document: &SourceDocument,
    blocks: &[ParsedBlock],
) -> (usize, Vec<ParsedSection>) {
    let preamble_end = blocks
        .iter()
        .position(|block| {
            block
                .as_markdown()
                .and_then(MarkdownSegment::heading_level)
                .is_some()
        })
        .unwrap_or(blocks.len());
    let mut index = preamble_end;
    let mut sections = Vec::new();
    while index < blocks.len() {
        sections.push(build_section(document, blocks, &mut index));
    }
    (preamble_end, sections)
}

fn build_section(
    document: &SourceDocument,
    blocks: &[ParsedBlock],
    index: &mut usize,
) -> ParsedSection {
    let heading_block = *index;
    let heading = blocks[heading_block]
        .as_markdown()
        .expect("section headings are Markdown blocks");
    let level = heading
        .heading_level()
        .expect("section starts at a heading");
    let title = heading.heading_title().unwrap_or_default().to_owned();
    *index += 1;
    let content_start = *index;
    while *index < blocks.len()
        && blocks[*index]
            .as_markdown()
            .and_then(MarkdownSegment::heading_level)
            .is_none()
    {
        *index += 1;
    }
    let content_end = *index;
    let mut children = Vec::new();
    while *index < blocks.len() {
        let next_level = blocks[*index]
            .as_markdown()
            .and_then(MarkdownSegment::heading_level)
            .expect("non-heading content was consumed above");
        if next_level <= level {
            break;
        }
        children.push(build_section(document, blocks, index));
    }
    let end = blocks
        .get(*index)
        .map_or(document.span().end_byte() as usize, |block| match block {
            ParsedBlock::Markdown(markdown) => markdown.source().start_byte() as usize,
            ParsedBlock::Item(item) => item.source().start_byte() as usize,
        });
    ParsedSection {
        level,
        title,
        source: span(document, heading.source().start_byte() as usize, end),
        heading_source: heading_markup_span(document, heading),
        heading_block,
        content_start,
        content_end,
        children,
    }
}

fn heading_markup_span(document: &SourceDocument, heading: &MarkdownSegment) -> SourceSpan {
    let source = document.source().as_str();
    let start = heading.source().start_byte() as usize;
    let end = heading.source().end_byte() as usize;
    let first_line = next_line(source, start, end);
    let first_content = line_content_index(first_line, source);
    let heading_end =
        if heading.heading_kind() == Some(ParsedHeadingKind::Atx) || first_line.stop() >= end {
            first_content.stop()
        } else {
            line_content_index(next_line(source, first_line.stop(), end), source).stop()
        };
    span(document, start, heading_end)
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

fn is_item_candidate(line: &[u8], configured_prefix: &str) -> bool {
    let Ok(line) = str::from_utf8(without_line_ending(line)) else {
        return false;
    };
    let Some(rest) = line.strip_prefix(":::") else {
        return false;
    };
    if rest.is_empty() || rest.starts_with(':') || rest.starts_with(char::is_whitespace) {
        return false;
    }

    let mut tokens = rest.split_ascii_whitespace();
    let Some(_flavour) = tokens.next() else {
        return false;
    };
    let Some(mid) = tokens.next() else {
        return false;
    };
    is_mid_shaped(mid, configured_prefix)
}

fn is_mid_shaped(token: &str, configured_prefix: &str) -> bool {
    if token.starts_with(configured_prefix) {
        return true;
    }

    let Some((prefix, encoded)) = token.split_once('_') else {
        return false;
    };
    valid_name_segment(prefix, true) && encoded.chars().count() == 26
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
