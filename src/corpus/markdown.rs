//! Private Rushdown adapter for Mara's alpha document projection.

use std::{fmt, ops::Range};

use rushdown::{
    ast::{Arena, KindData, NodeKind, NodeRef, NodeType, PrettyPrint},
    parser::{
        self, AnyBlockParser, AnyInlineParser, BlockParser, InlineParser,
        PRIORITY_FENCED_CODE_BLOCK, PRIORITY_LINK, Parser, ParserExtension, ParserExtensionFn,
    },
    text::{self, BasicReader, Reader as _, Segment},
};

use crate::{is_item_id, is_snake_name};

const DELIMITER_BLOCK_PRIORITY: u32 = PRIORITY_FENCED_CODE_BLOCK + 50;
const MARA_INLINE_PRIORITY: u32 = PRIORITY_LINK - 50;

#[derive(Debug)]
pub(super) struct ParsedDocument {
    pub(super) items: Vec<ParsedItem>,
}

#[derive(Debug)]
pub(super) struct ParsedItem {
    pub(super) flavour: String,
    pub(super) id: String,
    pub(super) title: String,
    pub(super) metadata: Vec<ParsedMetadataEntry>,
    pub(super) body: Range<usize>,
    pub(super) mentions: Vec<ParsedMention>,
    pub(super) source: Range<usize>,
}

#[derive(Debug)]
pub(super) struct ParsedMetadataEntry {
    pub(super) key: String,
    pub(super) value: String,
    pub(super) source: Range<usize>,
}

#[derive(Debug)]
pub(super) struct ParsedMention {
    pub(super) target: String,
    pub(super) source: Range<usize>,
}

#[derive(Debug)]
pub(super) struct ParseError {
    pub(super) line: usize,
    pub(super) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelimiterKind {
    Opener,
    Closer,
}

#[derive(Debug, Clone)]
struct Delimiter {
    kind: DelimiterKind,
    source: Range<usize>,
}

#[derive(Debug)]
struct MaraBlockDelimiterNode {
    delimiter: Delimiter,
}

impl NodeKind for MaraBlockDelimiterNode {
    fn typ(&self) -> NodeType {
        NodeType::LeafBlock
    }

    fn kind_name(&self) -> &'static str {
        "MaraBlockDelimiter"
    }
}

impl PrettyPrint for MaraBlockDelimiterNode {
    fn pretty_print(
        &self,
        writer: &mut dyn fmt::Write,
        _source: &str,
        level: usize,
    ) -> fmt::Result {
        writeln!(writer, "{}MaraBlockDelimiter", "  ".repeat(level))
    }
}

impl From<MaraBlockDelimiterNode> for KindData {
    fn from(node: MaraBlockDelimiterNode) -> Self {
        Self::Extension(Box::new(node))
    }
}

#[derive(Debug)]
struct MaraInlineDelimiterNode {
    delimiter: Delimiter,
}

impl NodeKind for MaraInlineDelimiterNode {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }

    fn kind_name(&self) -> &'static str {
        "MaraInlineDelimiter"
    }
}

impl PrettyPrint for MaraInlineDelimiterNode {
    fn pretty_print(
        &self,
        writer: &mut dyn fmt::Write,
        _source: &str,
        level: usize,
    ) -> fmt::Result {
        writeln!(writer, "{}MaraInlineDelimiter", "  ".repeat(level))
    }
}

impl From<MaraInlineDelimiterNode> for KindData {
    fn from(node: MaraInlineDelimiterNode) -> Self {
        Self::Extension(Box::new(node))
    }
}

#[derive(Debug)]
struct MaraMentionNode {
    target: String,
    source: Range<usize>,
}

impl NodeKind for MaraMentionNode {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }

    fn kind_name(&self) -> &'static str {
        "MaraMention"
    }
}

impl PrettyPrint for MaraMentionNode {
    fn pretty_print(
        &self,
        writer: &mut dyn fmt::Write,
        _source: &str,
        level: usize,
    ) -> fmt::Result {
        writeln!(writer, "{}MaraMention", "  ".repeat(level))
    }
}

impl From<MaraMentionNode> for KindData {
    fn from(node: MaraMentionNode) -> Self {
        Self::Extension(Box::new(node))
    }
}

#[derive(Debug, Default)]
struct MaraDelimiterBlockParser;

impl MaraDelimiterBlockParser {
    fn new() -> Self {
        Self
    }
}

impl BlockParser for MaraDelimiterBlockParser {
    fn trigger(&self) -> &[u8] {
        b":"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut BasicReader,
        _context: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        let segment = reader.peek_line_segment()?;
        let delimiter = delimiter(reader.source(), segment)?;
        reader.advance_to_eol();
        Some((
            arena.new_node(MaraBlockDelimiterNode { delimiter }),
            parser::State::NO_CHILDREN,
        ))
    }

    fn cont(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        _reader: &mut BasicReader,
        _context: &mut parser::Context,
    ) -> Option<parser::State> {
        None
    }
}

impl From<MaraDelimiterBlockParser> for AnyBlockParser {
    fn from(parser: MaraDelimiterBlockParser) -> Self {
        Self::Extension(Box::new(parser))
    }
}

#[derive(Debug, Default)]
struct MaraInlineParser;

impl MaraInlineParser {
    fn new() -> Self {
        Self
    }

    fn parse_delimiter(
        &self,
        arena: &mut Arena,
        reader: &mut text::BlockReader,
    ) -> Option<NodeRef> {
        let (_, segment) = reader.peek_line_bytes()?;
        let delimiter = delimiter(reader.source(), segment)?;
        reader.advance(delimiter.source.end - delimiter.source.start);
        Some(arena.new_node(MaraInlineDelimiterNode { delimiter }))
    }

    fn parse_mention(&self, arena: &mut Arena, reader: &mut text::BlockReader) -> Option<NodeRef> {
        let (line, segment) = reader.peek_line_bytes()?;
        if !line.starts_with(b"[[") || escaped_opening(reader.source().as_bytes(), segment.start())
        {
            return None;
        }
        let closing = line[2..].windows(2).position(|pair| pair == b"]]")? + 2;
        let target = std::str::from_utf8(&line[2..closing]).ok()?;
        if !is_item_id(target) {
            return None;
        }
        let length = closing + 2;
        reader.advance(length);
        Some(arena.new_node(MaraMentionNode {
            target: target.to_owned(),
            source: segment.start()..segment.start() + length,
        }))
    }
}

impl InlineParser for MaraInlineParser {
    fn trigger(&self) -> &[u8] {
        b":["
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BlockReader,
        _context: &mut parser::Context,
    ) -> Option<NodeRef> {
        match reader.peek_byte() {
            b':' => self.parse_delimiter(arena, reader),
            b'[' => self.parse_mention(arena, reader),
            _ => None,
        }
    }
}

impl From<MaraInlineParser> for AnyInlineParser {
    fn from(parser: MaraInlineParser) -> Self {
        Self::Extension(Box::new(parser))
    }
}

fn mara_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|parser: &mut Parser| {
        parser.add_block_parser(
            MaraDelimiterBlockParser::new,
            parser::NoParserOptions,
            DELIMITER_BLOCK_PRIORITY,
        );
        parser.add_inline_parser(
            MaraInlineParser::new,
            parser::NoParserOptions,
            MARA_INLINE_PRIORITY,
        );
    })
}

pub(super) fn parse(source: &str) -> Result<ParsedDocument, ParseError> {
    let (delimiters, mentions) = parse_extensions(source);
    project(source, &delimiters, &mentions)
}

pub(super) fn parse_for_validation(source: &str) -> (ParsedDocument, Vec<ParseError>) {
    let (delimiters, mentions) = parse_extensions(source);
    project_for_validation(source, &delimiters, &mentions)
}

fn parse_extensions(source: &str) -> (Vec<Delimiter>, Vec<ParsedMention>) {
    let parser = Parser::with_extensions(parser::Options::default(), mara_extension());
    let mut reader = BasicReader::new(source);
    let (arena, document_ref) = parser.parse(&mut reader);
    let mut delimiters = Vec::new();
    let mut mentions = Vec::new();
    collect_extensions(&arena, document_ref, &mut delimiters, &mut mentions);
    delimiters.sort_by_key(|delimiter| delimiter.source.start);
    mentions.sort_by_key(|mention| mention.source.start);

    (delimiters, mentions)
}

fn collect_extensions(
    arena: &Arena,
    node_ref: NodeRef,
    delimiters: &mut Vec<Delimiter>,
    mentions: &mut Vec<ParsedMention>,
) {
    match arena[node_ref].kind_data().kind_name() {
        "MaraBlockDelimiter" => {
            let node = rushdown::as_extension_data!(arena, node_ref, MaraBlockDelimiterNode);
            delimiters.push(node.delimiter.clone());
        }
        "MaraInlineDelimiter" => {
            let node = rushdown::as_extension_data!(arena, node_ref, MaraInlineDelimiterNode);
            delimiters.push(node.delimiter.clone());
        }
        "MaraMention" => {
            let node = rushdown::as_extension_data!(arena, node_ref, MaraMentionNode);
            mentions.push(ParsedMention {
                target: node.target.clone(),
                source: node.source.clone(),
            });
        }
        _ => {}
    }

    for child in arena[node_ref].children(arena) {
        collect_extensions(arena, child, delimiters, mentions);
    }
}

fn project(
    source: &str,
    delimiters: &[Delimiter],
    mentions: &[ParsedMention],
) -> Result<ParsedDocument, ParseError> {
    let lines = source_lines(source);
    let mut items = Vec::new();
    let mut delimiter_index = 0;

    while let Some(delimiter) = delimiters.get(delimiter_index) {
        if delimiter.kind == DelimiterKind::Closer {
            delimiter_index += 1;
            continue;
        }

        items.push(project_item(
            &lines,
            delimiters,
            mentions,
            &mut delimiter_index,
        )?);
    }

    Ok(ParsedDocument { items })
}

fn project_for_validation(
    source: &str,
    delimiters: &[Delimiter],
    mentions: &[ParsedMention],
) -> (ParsedDocument, Vec<ParseError>) {
    let lines = source_lines(source);
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut delimiter_index = 0;

    while let Some(delimiter) = delimiters.get(delimiter_index) {
        if delimiter.kind == DelimiterKind::Closer {
            delimiter_index += 1;
            continue;
        }

        let opener_index = delimiter_index;
        match project_item(&lines, delimiters, mentions, &mut delimiter_index) {
            Ok(item) => items.push(item),
            Err(error) => {
                errors.push(error);
                if delimiter_index == opener_index {
                    delimiter_index += 1;
                    if delimiters
                        .get(delimiter_index)
                        .is_some_and(|delimiter| delimiter.kind == DelimiterKind::Closer)
                    {
                        delimiter_index += 1;
                    }
                }
            }
        }
    }

    (ParsedDocument { items }, errors)
}

fn project_item(
    lines: &[SourceLine<'_>],
    delimiters: &[Delimiter],
    mentions: &[ParsedMention],
    delimiter_index: &mut usize,
) -> Result<ParsedItem, ParseError> {
    let delimiter = &delimiters[*delimiter_index];
    debug_assert_eq!(delimiter.kind, DelimiterKind::Opener);

    let opener_line = line_at(lines, delimiter.source.start);
    let (flavour, id) = opener(opener_line.text).ok_or_else(|| ParseError {
        line: opener_line.number,
        message: "item opener must be ':::mara <flavour> <id>' with no other tokens".to_owned(),
    })?;
    if !is_snake_name(flavour) {
        return Err(ParseError {
            line: opener_line.number,
            message: format!("invalid flavour '{flavour}'"),
        });
    }
    if !is_item_id(id) {
        return Err(ParseError {
            line: opener_line.number,
            message: format!("invalid item ID '{id}'"),
        });
    }

    let (metadata, body_start) = parse_metadata(lines, opener_line)?;
    let title_entries = metadata
        .iter()
        .filter(|entry| entry.key == "title")
        .collect::<Vec<_>>();
    if title_entries.len() != 1 || title_entries[0].value.is_empty() {
        return Err(ParseError {
            line: opener_line.number,
            message: "item must have exactly one non-empty title entry".to_owned(),
        });
    }
    let title = title_entries[0].value.clone();

    *delimiter_index += 1;
    let closing = loop {
        let Some(next) = delimiters.get(*delimiter_index) else {
            return Err(ParseError {
                line: opener_line.number,
                message: "item is missing its closing delimiter".to_owned(),
            });
        };
        if next.source.start < body_start {
            *delimiter_index += 1;
            continue;
        }
        match next.kind {
            DelimiterKind::Closer => break next,
            DelimiterKind::Opener => {
                let nested = line_at(lines, next.source.start);
                let message = if opener(nested.text).is_some() {
                    "items cannot nest"
                } else {
                    "invalid nested item opener"
                };
                return Err(ParseError {
                    line: nested.number,
                    message: message.to_owned(),
                });
            }
        }
    };
    let closing_line = line_at(lines, closing.source.start);
    let body_end = closing_line.start;
    let item_mentions = mentions
        .iter()
        .filter(|mention| mention.source.start >= body_start && mention.source.end <= body_end)
        .map(|mention| ParsedMention {
            target: mention.target.clone(),
            source: mention.source.clone(),
        })
        .collect();
    *delimiter_index += 1;

    Ok(ParsedItem {
        flavour: flavour.to_owned(),
        id: id.to_owned(),
        title,
        metadata,
        body: body_start..body_end,
        mentions: item_mentions,
        source: opener_line.start..closing_line.full_end,
    })
}

fn parse_metadata(
    lines: &[SourceLine<'_>],
    opener_line: SourceLine<'_>,
) -> Result<(Vec<ParsedMetadataEntry>, usize), ParseError> {
    let mut metadata = Vec::new();
    let mut line_index = opener_line.number;
    while line_index < lines.len() && !lines[line_index].text.trim().is_empty() {
        let line = lines[line_index];
        let Some(rest) = line.text.strip_prefix(':') else {
            return Err(ParseError {
                line: line.number,
                message: "expected metadata or a blank line before the item body".to_owned(),
            });
        };
        let Some((key, value)) = rest.split_once(':') else {
            return Err(ParseError {
                line: line.number,
                message: "invalid metadata entry".to_owned(),
            });
        };
        if !is_snake_name(key) {
            return Err(ParseError {
                line: line.number,
                message: format!("invalid metadata key '{key}'"),
            });
        }
        metadata.push(ParsedMetadataEntry {
            key: key.to_owned(),
            value: value.trim().to_owned(),
            source: line.start..line.end,
        });
        line_index += 1;
    }
    if line_index == lines.len() {
        return Err(ParseError {
            line: opener_line.number,
            message: "item is missing its body boundary and closing delimiter".to_owned(),
        });
    }

    Ok((metadata, lines[line_index].full_end))
}

fn delimiter(source: &str, segment: Segment) -> Option<Delimiter> {
    if !is_physical_line_start(source, segment.start()) {
        return None;
    }
    let mut end = segment.stop();
    let bytes = source.as_bytes();
    if end > segment.start() && bytes[end - 1] == b'\n' {
        end -= 1;
    }
    if end > segment.start() && bytes[end - 1] == b'\r' {
        end -= 1;
    }
    let line = &source[segment.start()..end];
    let kind = if line == ":::" {
        DelimiterKind::Closer
    } else if looks_like_item_opener(line) {
        DelimiterKind::Opener
    } else {
        return None;
    };
    Some(Delimiter {
        kind,
        source: segment.start()..end,
    })
}

fn is_physical_line_start(source: &str, start: usize) -> bool {
    start == 0 || source.as_bytes().get(start.wrapping_sub(1)) == Some(&b'\n')
}

fn opener(line: &str) -> Option<(&str, &str)> {
    let declaration = line.strip_prefix(":::mara ")?;
    let (flavour, id) = declaration.split_once(' ')?;
    (!flavour.is_empty() && !id.is_empty() && !id.bytes().any(|byte| byte.is_ascii_whitespace()))
        .then_some((flavour, id))
}

fn looks_like_item_opener(line: &str) -> bool {
    line.strip_prefix(":::mara")
        .is_some_and(|rest| rest.is_empty() || rest.chars().next().is_some_and(char::is_whitespace))
}

fn escaped_opening(source: &[u8], opening: usize) -> bool {
    let mut cursor = opening;
    while cursor > 0 && source[cursor - 1] == b'\\' {
        cursor -= 1;
    }
    (opening - cursor) % 2 == 1
}

#[derive(Clone, Copy)]
struct SourceLine<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    full_end: usize,
    number: usize,
}

fn source_lines(source: &str) -> Vec<SourceLine<'_>> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut number = 1;
    while start < bytes.len() {
        let newline = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset);
        let full_end = newline.map_or(bytes.len(), |offset| offset + 1);
        let mut end = newline.unwrap_or(bytes.len());
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        lines.push(SourceLine {
            text: &source[start..end],
            start,
            end,
            full_end,
            number,
        });
        start = full_end;
        number += 1;
    }
    lines
}

fn line_at<'a>(lines: &[SourceLine<'a>], start: usize) -> SourceLine<'a> {
    let index = lines
        .binary_search_by_key(&start, |line| line.start)
        .expect("Rushdown Mara delimiter starts on a source line");
    lines[index]
}
