//! Strict loading of the configured v1 Mara schema document and identity.

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use mara_core::{
    Diagnostic, DiagnosticContext, DiagnosticSeverity, DiagnosticValue, IdentityConfiguration,
    MidFormat, MidIdentity, RelatedDiagnostic, SchemaDiagnosticCode, SchemaDocument, SchemaField,
    SchemaIdentity, SchemaSection, SourceIndex, SourceSpan,
};
use regex_lite::Regex;
use saphyr_parser::{Event, Marker, Parser, ScalarStyle, Span, SpannedEventReceiver, Tag};

use crate::project::{LoadedProject, open_loaded_schema};

type DecodeResult<T> = Result<T, Box<Diagnostic>>;

#[derive(Debug)]
enum DocumentDecodeFailure {
    Diagnostic(Box<Diagnostic>),
    Diagnostics(Vec<Diagnostic>),
}

impl DocumentDecodeFailure {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostic(diagnostic) => vec![*diagnostic],
            Self::Diagnostics(diagnostics) => diagnostics,
        }
    }
}

impl From<Diagnostic> for DocumentDecodeFailure {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Diagnostic(Box::new(diagnostic))
    }
}

impl From<Box<Diagnostic>> for DocumentDecodeFailure {
    fn from(diagnostic: Box<Diagnostic>) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

/// A schema input failure. Invalid source is represented entirely by diagnostics;
/// I/O failures additionally preserve their infrastructure cause and affected path.
#[derive(Debug)]
pub struct SchemaLoadError {
    diagnostics: Vec<Diagnostic>,
    path: Option<PathBuf>,
    source: Option<io::Error>,
}

impl SchemaLoadError {
    fn invalid(mut diagnostics: Vec<Diagnostic>) -> Self {
        sort_diagnostics(&mut diagnostics);
        Self {
            diagnostics,
            path: None,
            source: None,
        }
    }

    fn io(path: PathBuf, source_path: &str, operation: &'static str, source: io::Error) -> Self {
        let diagnostic = Diagnostic::new(
            SchemaDiagnosticCode::Io,
            format!("could not {operation} the configured schema"),
            None,
        )
        .with_detail("path", source_path);
        Self {
            diagnostics: vec![diagnostic],
            path: Some(path),
            source: Some(source),
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn io_source(&self) -> Option<&io::Error> {
        self.source.as_ref()
    }
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.diagnostics.as_slice() {
            [diagnostic] => write!(
                formatter,
                "{}: {}",
                diagnostic.code().as_str(),
                diagnostic.message()
            ),
            diagnostics => write!(formatter, "schema has {} diagnostics", diagnostics.len()),
        }
    }
}

impl Error for SchemaLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Loads and decodes only the schema selected by an already rooted project.
pub fn load_schema(project: &LoadedProject) -> Result<SchemaDocument, SchemaLoadError> {
    let source_path = schema_source_path(project).map_err(|source| {
        SchemaLoadError::io(
            project.schema_path.clone(),
            "<loaded schema>",
            "identify",
            source,
        )
    })?;
    let mut file = open_loaded_schema(project).map_err(|source| {
        SchemaLoadError::io(
            project.schema_path.clone(),
            &source_path,
            "open",
            io::Error::other(source),
        )
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| {
        SchemaLoadError::io(project.schema_path.clone(), &source_path, "read", source)
    })?;
    decode_schema(&bytes, &source_path)
}

fn schema_source_path(project: &LoadedProject) -> Result<String, io::Error> {
    let source_path = project.schema_source_path.clone();
    SourceSpan::try_new(source_path.as_str(), "", 0, 0, 1, 1, 1, 1).map_err(|_| {
        io::Error::other("configured schema path cannot be represented by a Mara source span")
    })?;
    Ok(source_path)
}

fn decode_schema(bytes: &[u8], path: &str) -> Result<SchemaDocument, SchemaLoadError> {
    let source = std::str::from_utf8(bytes).map_err(|error| {
        let offset = error.valid_up_to();
        let valid_prefix = std::str::from_utf8(&bytes[..offset])
            .expect("the prefix before a UTF-8 decoding error is valid");
        let (line, column) = position_at_valid_prefix(&bytes[..offset]);
        let primary = SourceSpan::try_new(
            path,
            valid_prefix,
            offset as u64,
            offset as u64,
            line as u64,
            column as u64,
            line as u64,
            column as u64,
        )
        .expect("the valid UTF-8 prefix produced a valid Mara source span");
        SchemaLoadError::invalid(vec![
            Diagnostic::new(
                SchemaDiagnosticCode::Syntax,
                "schema source is not valid UTF-8",
                Some(primary),
            )
            .with_detail("feature", "invalid_utf8"),
        ])
    })?;

    let source_index = SourceIndex::try_new(path, source)
        .expect("the rooted project produced a valid Mara source index");

    if let Some(offset) = source.find('\0') {
        let (line, column) = position_at_valid_prefix(&source.as_bytes()[..offset]);
        let primary = source_span(
            &source_index,
            offset,
            offset + 1,
            line,
            column,
            line,
            column + 1,
        );
        return Err(SchemaLoadError::invalid(vec![
            Diagnostic::new(
                SchemaDiagnosticCode::Syntax,
                "schema source contains a forbidden NUL character",
                Some(primary),
            )
            .with_detail("feature", "nul_character"),
        ]));
    }

    if let Some(diagnostic) = validate_document_directives(source, &source_index) {
        return Err(SchemaLoadError::invalid(vec![diagnostic]));
    }

    let parser_source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let parser_start_byte = source.len() - parser_source.len();
    let source_map = SourceMap::new(source, parser_start_byte, source_index);
    let mut receiver = TreeBuilder::new(&source_map);
    let parse_error = Parser::new_from_str(parser_source).find_map(|result| match result {
        Ok((event, span)) => {
            receiver.on_event(event, span);
            None
        }
        Err(error) => Some(error),
    });
    if let Some(error) = parse_error {
        let primary = marker_span(&source_map, source_map.position(*error.marker()));
        receiver.drain_nodes();
        return Err(SchemaLoadError::invalid(vec![
            Diagnostic::new(
                SchemaDiagnosticCode::Syntax,
                format!("invalid YAML syntax: {}", error.info()),
                Some(primary),
            )
            .with_detail("feature", "yaml_syntax"),
        ]));
    }
    if let Some((message, span)) = receiver.error.take() {
        receiver.drain_nodes();
        return Err(SchemaLoadError::invalid(vec![Diagnostic::new(
            SchemaDiagnosticCode::Syntax,
            message,
            Some(parser_span(&source_map, span)),
        )]));
    }
    if receiver.documents.len() != 1 {
        let (primary, feature) = if receiver.documents.is_empty() {
            let (line, column) = position_at_valid_prefix(source.as_bytes());
            (
                Some(source_span(
                    &source_map.source_index,
                    source.len(),
                    source.len(),
                    line,
                    column,
                    line,
                    column,
                )),
                "empty_document",
            )
        } else {
            (
                receiver
                    .document_starts
                    .get(1)
                    .copied()
                    .or_else(|| receiver.document_starts.first().copied())
                    .map(|span| parser_span(&source_map, span)),
                "multiple_documents",
            )
        };
        receiver.drain_nodes();
        return Err(SchemaLoadError::invalid(vec![
            Diagnostic::new(
                SchemaDiagnosticCode::Syntax,
                "schema must contain exactly one YAML document",
                primary,
            )
            .with_detail("feature", feature),
        ]));
    }

    let root = receiver.documents.pop().expect("one document was checked");
    let mut profile = Vec::new();
    validate_profile(&root, &source_map, &mut profile);
    let result = if profile.is_empty() {
        decode_v1_document(&source_map, &root)
            .map_err(|failure| SchemaLoadError::invalid(failure.into_diagnostics()))
    } else {
        Err(SchemaLoadError::invalid(profile))
    };
    drop_parsed_nodes_iteratively([root]);
    result
}

fn directive_arguments<'a>(content: &'a str, name: &str) -> Option<&'a str> {
    let rest = content.strip_prefix(name)?;
    rest.as_bytes()
        .first()
        .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
        .then_some(rest)
}

fn validate_document_directives(source: &str, source_index: &SourceIndex) -> Option<Diagnostic> {
    let mut byte_offset = 0;
    let mut line = 1;
    while byte_offset < source.len() {
        let (source_line, next_offset) = source_line_at(source, byte_offset);
        let content = source_line;
        let content = if byte_offset == 0 {
            content.strip_prefix('\u{feff}').unwrap_or(content)
        } else {
            content
        };
        if content.trim().is_empty() || content.trim_start().starts_with('#') {
            byte_offset = next_offset;
            line += 1;
            continue;
        }
        if !content.starts_with('%') {
            break;
        }

        let feature = if let Some(rest) = directive_arguments(content, "%YAML") {
            let version = rest.split_whitespace().next().unwrap_or("");
            if version == "1.2" {
                byte_offset = next_offset;
                line += 1;
                continue;
            }
            "unsupported_yaml_version"
        } else if content.starts_with("%YAML") {
            "unsupported_directive"
        } else if directive_arguments(content, "%TAG").is_some() {
            "custom_tag"
        } else {
            "unsupported_directive"
        };
        let bom_bytes = source_line.len() - content.len();
        let start_byte = byte_offset + bom_bytes;
        let start_column = source_line[..bom_bytes].chars().count() + 1;
        let primary = source_span(
            source_index,
            start_byte,
            start_byte + content.len(),
            line,
            start_column,
            line,
            start_column + content.chars().count(),
        );
        return Some(
            Diagnostic::new(
                SchemaDiagnosticCode::Syntax,
                "schema uses a YAML directive outside the supported 1.2 Core profile",
                Some(primary),
            )
            .with_detail("feature", feature),
        );
    }
    None
}

fn source_line_at(source: &str, start: usize) -> (&str, usize) {
    let remaining = &source[start..];
    let content_len = remaining.find(['\r', '\n']).unwrap_or(remaining.len());
    let terminator_len = if remaining[content_len..].starts_with("\r\n") {
        2
    } else if content_len < remaining.len() {
        1
    } else {
        0
    };
    (
        &remaining[..content_len],
        start + content_len + terminator_len,
    )
}

#[derive(Debug, Clone, Copy)]
struct ParsedPosition {
    byte: usize,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, Copy)]
struct ParsedSpan {
    start: ParsedPosition,
    end: ParsedPosition,
}

impl ParsedSpan {
    const fn cover(start: ParsedPosition, end: ParsedPosition) -> Self {
        Self { start, end }
    }
}

#[derive(Debug)]
struct SourceMap<'source> {
    source: &'source str,
    source_index: SourceIndex,
    char_to_byte: Vec<usize>,
    indicators: IndicatorIndex,
    marker_index_offset: usize,
    first_line_column_offset: usize,
}

impl<'source> SourceMap<'source> {
    fn new(source: &'source str, parser_start_byte: usize, source_index: SourceIndex) -> Self {
        let mut char_to_byte = source
            .char_indices()
            .map(|(byte, _)| byte)
            .collect::<Vec<_>>();
        char_to_byte.push(source.len());
        let indicators = yaml_indicator_index(source);
        let marker_index_offset = source[..parser_start_byte].chars().count();
        Self {
            source,
            source_index,
            char_to_byte,
            indicators,
            marker_index_offset,
            first_line_column_offset: marker_index_offset,
        }
    }

    fn position(&self, marker: Marker) -> ParsedPosition {
        let byte = *self
            .char_to_byte
            .get(marker.index() + self.marker_index_offset)
            .expect("YAML parser marker is inside its UTF-8 input");
        ParsedPosition {
            byte,
            line: marker.line(),
            column: marker.col()
                + 1
                + if marker.line() == 1 {
                    self.first_line_column_offset
                } else {
                    0
                },
        }
    }

    fn span(&self, span: Span) -> ParsedSpan {
        ParsedSpan {
            start: self.position(span.start),
            end: self.position(span.end),
        }
    }

    fn node_span(&self, span: Span, tag: Option<&Tag>) -> ParsedSpan {
        let mut parsed = self.span(span);
        self.expand_tag(&mut parsed, tag);
        parsed
    }

    fn scalar_span(&self, span: Span, style: ScalarStyle, tag: Option<&Tag>) -> ParsedSpan {
        let mut parsed = self.span(span);
        if matches!(style, ScalarStyle::Literal | ScalarStyle::Folded)
            && let Some(start_byte) = preceding_block_scalar_start(
                self.source,
                if style == ScalarStyle::Literal {
                    &self.indicators.literal_blocks
                } else {
                    &self.indicators.folded_blocks
                },
                parsed.start.byte,
                if style == ScalarStyle::Literal {
                    '|'
                } else {
                    '>'
                },
            )
        {
            parsed.start = self.position_at_byte(start_byte);
        }
        self.expand_tag(&mut parsed, tag);
        parsed
    }

    fn expand_tag(&self, parsed: &mut ParsedSpan, tag: Option<&Tag>) {
        if tag.is_none() {
            return;
        }
        if let Some(start_byte) =
            preceding_tag_start(self.source, &self.indicators.tags, parsed.start.byte)
        {
            parsed.start = self.position_at_byte(start_byte);
        }
    }

    fn anchor_span(&self, span: Span, anchor: usize) -> Option<ParsedSpan> {
        if anchor == 0 {
            return None;
        }
        let parser_span = self.span(span);
        let Some((start, end)) = preceding_anchor_range(
            self.source,
            &self.indicators.anchors,
            parser_span.start.byte,
        ) else {
            return Some(parser_span);
        };
        Some(ParsedSpan {
            start: self.position_at_byte(start),
            end: self.position_at_byte(end),
        })
    }

    fn position_at_byte(&self, byte: usize) -> ParsedPosition {
        let (line, column) = self
            .source_index
            .coordinates_at(byte as u64)
            .expect("parser evidence ends at a legal Mara source boundary");
        ParsedPosition {
            byte,
            line: usize::try_from(line).expect("source line fits in usize"),
            column: usize::try_from(column).expect("source column fits in usize"),
        }
    }
}

fn preceding_block_scalar_start(
    source: &str,
    candidates: &[IndicatorCandidate],
    content_start: usize,
    indicator: char,
) -> Option<usize> {
    let candidate_count = candidates.partition_point(|candidate| candidate.start < content_start);
    for candidate in candidates[..candidate_count].iter().rev() {
        if candidate.is_in_preceding_comment(content_start) {
            continue;
        }
        let start = candidate.start;
        if block_scalar_prefix_only(&source[start + indicator.len_utf8()..content_start]) {
            return Some(start);
        }
    }
    None
}

fn block_scalar_prefix_only(source: &str) -> bool {
    let Some(line_break) = source.find(['\r', '\n']) else {
        return false;
    };
    let header = &source[..line_break];
    let header = header.split_once('#').map_or(header, |(before, _)| before);
    let indentation = source[line_break..]
        .strip_prefix("\r\n")
        .or_else(|| source[line_break..].strip_prefix(['\r', '\n']))
        .expect("line break was located");
    header
        .chars()
        .all(|character| character.is_whitespace() || matches!(character, '+' | '-' | '1'..='9'))
        && indentation.chars().all(char::is_whitespace)
}

#[derive(Debug, Clone, Copy)]
struct IndicatorCandidate {
    start: usize,
    comment_line_end: Option<usize>,
}

impl IndicatorCandidate {
    fn is_in_preceding_comment(self, node_start: usize) -> bool {
        self.comment_line_end
            .is_some_and(|line_end| node_start > line_end)
    }
}

#[derive(Debug, Default)]
struct IndicatorIndex {
    tags: Vec<IndicatorCandidate>,
    anchors: Vec<IndicatorCandidate>,
    literal_blocks: Vec<IndicatorCandidate>,
    folded_blocks: Vec<IndicatorCandidate>,
}

fn yaml_indicator_index(source: &str) -> IndicatorIndex {
    let mut indicators = IndicatorIndex::default();
    let mut line_start = 0;
    while line_start < source.len() {
        let (line, next_line) = source_line_at(source, line_start);
        let comment_start = line.char_indices().find_map(|(offset, character)| {
            (character == '#'
                && (offset == 0
                    || line[..offset]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace)))
            .then_some(offset)
        });
        let line_end = line_start + line.len();
        for (offset, character) in line.char_indices() {
            let candidate = IndicatorCandidate {
                start: line_start + offset,
                comment_line_end: comment_start
                    .is_some_and(|comment_start| offset >= comment_start)
                    .then_some(line_end),
            };
            match character {
                '!' => indicators.tags.push(candidate),
                '&' => indicators.anchors.push(candidate),
                '|' => indicators.literal_blocks.push(candidate),
                '>' => indicators.folded_blocks.push(candidate),
                _ => {}
            }
        }
        line_start = next_line;
    }
    indicators
}

fn preceding_tag_start(
    source: &str,
    candidates: &[IndicatorCandidate],
    node_start: usize,
) -> Option<usize> {
    let candidate_count = candidates.partition_point(|candidate| candidate.start < node_start);
    let mut earliest = None;
    for candidate in candidates[..candidate_count].iter().rev() {
        if candidate.is_in_preceding_comment(node_start) {
            continue;
        }
        let start = candidate.start;
        if tag_start_boundary(source, start)
            && let Some(end) = raw_tag_end(source, start, node_start)
            && yaml_tag_suffix_only(&source[end..node_start])
        {
            earliest = Some(start);
        } else if earliest.is_some() {
            break;
        }
    }
    earliest
}

fn yaml_tag_suffix_only(source: &str) -> bool {
    let mut offset = 0;
    while offset < source.len() {
        let remaining = &source[offset..];
        if let Some(character) = remaining.chars().next()
            && character.is_whitespace()
        {
            offset += character.len_utf8();
            continue;
        }
        if remaining.starts_with('#') {
            offset += remaining.find(['\r', '\n']).unwrap_or(remaining.len());
            continue;
        }
        if remaining.starts_with('&')
            && let Some(end) = raw_anchor_end(source, offset, source.len())
        {
            offset = end;
            continue;
        }
        return false;
    }
    true
}

fn preceding_anchor_range(
    source: &str,
    candidates: &[IndicatorCandidate],
    node_start: usize,
) -> Option<(usize, usize)> {
    let candidate_count = candidates.partition_point(|candidate| candidate.start < node_start);
    let mut earliest = None;
    for candidate in candidates[..candidate_count].iter().rev() {
        if candidate.is_in_preceding_comment(node_start) {
            continue;
        }
        let start = candidate.start;
        if tag_start_boundary(source, start) {
            if let Some(end) = raw_anchor_end(source, start, node_start)
                && yaml_anchor_suffix_only(&source[end..node_start])
            {
                earliest = Some((start, end));
            } else if earliest.is_some() {
                break;
            }
        }
    }
    earliest
}

fn tag_start_boundary(source: &str, start: usize) -> bool {
    source[..start].chars().next_back().is_none_or(|character| {
        character.is_whitespace()
            || matches!(character, ':' | '-' | '?' | ',' | '[' | ']' | '{' | '}')
    })
}

fn raw_tag_end(source: &str, start: usize, node_start: usize) -> Option<usize> {
    let tag = &source[start..node_start];
    if let Some(verbatim) = tag.strip_prefix("!<") {
        return verbatim.find('>').map(|end| start + 2 + end + 1);
    }

    let suffix_start = start + '!'.len_utf8();
    let mut end = suffix_start;
    for (offset, character) in source[suffix_start..node_start].char_indices() {
        if character.is_whitespace() || matches!(character, ',' | '[' | ']' | '{' | '}') {
            break;
        }
        end = suffix_start + offset + character.len_utf8();
    }
    Some(end)
}

fn raw_anchor_end(source: &str, start: usize, node_start: usize) -> Option<usize> {
    let suffix_start = start + '&'.len_utf8();
    let mut end = suffix_start;
    for (offset, character) in source[suffix_start..node_start].char_indices() {
        if character.is_whitespace() || matches!(character, ',' | '[' | ']' | '{' | '}') {
            break;
        }
        end = suffix_start + offset + character.len_utf8();
    }
    (end > suffix_start).then_some(end)
}

fn yaml_anchor_suffix_only(source: &str) -> bool {
    let mut offset = 0;
    while offset < source.len() {
        let remaining = &source[offset..];
        if let Some(character) = remaining.chars().next()
            && character.is_whitespace()
        {
            offset += character.len_utf8();
            continue;
        }
        if remaining.starts_with('#') {
            offset += remaining.find(['\r', '\n']).unwrap_or(remaining.len());
            continue;
        }
        if remaining.starts_with('!')
            && let Some(end) = raw_tag_end(source, offset, source.len())
        {
            offset = end;
            continue;
        }
        return false;
    }
    true
}

#[derive(Debug)]
enum ParsedNode {
    Scalar {
        value: String,
        style: ScalarStyle,
        anchor: Option<ParsedSpan>,
        tag: Option<Tag>,
        span: ParsedSpan,
    },
    Sequence {
        values: Vec<Self>,
        anchor: Option<ParsedSpan>,
        tag: Option<Tag>,
        span: ParsedSpan,
    },
    Mapping {
        entries: Vec<(Self, Self)>,
        anchor: Option<ParsedSpan>,
        tag: Option<Tag>,
        span: ParsedSpan,
    },
    Alias {
        span: ParsedSpan,
    },
}

impl ParsedNode {
    const fn span(&self) -> ParsedSpan {
        match self {
            Self::Scalar { span, .. }
            | Self::Sequence { span, .. }
            | Self::Mapping { span, .. }
            | Self::Alias { span } => *span,
        }
    }
}

fn drop_parsed_nodes_iteratively(nodes: impl IntoIterator<Item = ParsedNode>) {
    let mut pending = nodes.into_iter().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        match node {
            ParsedNode::Sequence { values, .. } => pending.extend(values),
            ParsedNode::Mapping { entries, .. } => {
                for (key, value) in entries {
                    pending.push(key);
                    pending.push(value);
                }
            }
            ParsedNode::Scalar { .. } | ParsedNode::Alias { .. } => {}
        }
    }
}

#[derive(Debug)]
enum PendingContainer {
    Sequence {
        values: Vec<ParsedNode>,
        anchor: Option<ParsedSpan>,
        tag: Option<Tag>,
        start: ParsedSpan,
    },
    Mapping {
        values: Vec<ParsedNode>,
        anchor: Option<ParsedSpan>,
        tag: Option<Tag>,
        start: ParsedSpan,
    },
}

#[derive(Debug)]
struct TreeBuilder<'map, 'source> {
    source_map: &'map SourceMap<'source>,
    documents: Vec<ParsedNode>,
    document_starts: Vec<ParsedSpan>,
    stack: Vec<PendingContainer>,
    error: Option<(String, ParsedSpan)>,
}

impl<'map, 'source> TreeBuilder<'map, 'source> {
    fn new(source_map: &'map SourceMap<'source>) -> Self {
        Self {
            source_map,
            documents: Vec::new(),
            document_starts: Vec::new(),
            stack: Vec::new(),
            error: None,
        }
    }

    fn attach(&mut self, node: ParsedNode) {
        match self.stack.last_mut() {
            Some(PendingContainer::Sequence { values, .. })
            | Some(PendingContainer::Mapping { values, .. }) => values.push(node),
            None => self.documents.push(node),
        }
    }

    fn fail(&mut self, message: impl Into<String>, span: ParsedSpan) {
        if self.error.is_none() {
            self.error = Some((message.into(), span));
        }
    }

    fn drain_nodes(&mut self) {
        let mut nodes = std::mem::take(&mut self.documents);
        for container in &mut self.stack {
            match container {
                PendingContainer::Sequence { values, .. }
                | PendingContainer::Mapping { values, .. } => nodes.append(values),
            }
        }
        self.stack.clear();
        drop_parsed_nodes_iteratively(nodes);
    }
}

impl<'input> SpannedEventReceiver<'input> for TreeBuilder<'_, 'input> {
    fn on_event(&mut self, event: Event<'input>, span: Span) {
        let parser_span = self.source_map.span(span);
        match event {
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd | Event::Nothing => {}
            Event::DocumentStart(_) => self.document_starts.push(parser_span),
            Event::Alias(_) => self.attach(ParsedNode::Alias { span: parser_span }),
            Event::Scalar(value, style, anchor, tag) => {
                let anchor = self.source_map.anchor_span(span, anchor);
                let span = self.source_map.scalar_span(span, style, tag.as_deref());
                self.attach(ParsedNode::Scalar {
                    value: value.into_owned(),
                    style,
                    anchor,
                    tag: tag.map(|tag| tag.into_owned()),
                    span,
                });
            }
            Event::SequenceStart(anchor, tag) => {
                let anchor = self.source_map.anchor_span(span, anchor);
                let span = self.source_map.node_span(span, tag.as_deref());
                self.stack.push(PendingContainer::Sequence {
                    values: Vec::new(),
                    anchor,
                    tag: tag.map(|tag| tag.into_owned()),
                    start: span,
                });
            }
            Event::MappingStart(anchor, tag) => {
                let anchor = self.source_map.anchor_span(span, anchor);
                let span = self.source_map.node_span(span, tag.as_deref());
                self.stack.push(PendingContainer::Mapping {
                    values: Vec::new(),
                    anchor,
                    tag: tag.map(|tag| tag.into_owned()),
                    start: span,
                });
            }
            Event::SequenceEnd => match self.stack.pop() {
                Some(PendingContainer::Sequence {
                    values,
                    anchor,
                    tag,
                    start,
                }) => self.attach(ParsedNode::Sequence {
                    values,
                    anchor,
                    tag,
                    span: ParsedSpan::cover(start.start, parser_span.end),
                }),
                _ => self.fail("YAML sequence events are unbalanced", parser_span),
            },
            Event::MappingEnd => match self.stack.pop() {
                Some(PendingContainer::Mapping {
                    values,
                    anchor,
                    tag,
                    start,
                }) => {
                    if values.len() % 2 != 0 {
                        self.fail("YAML mapping contains an unmatched key", parser_span);
                        return;
                    }
                    let mut values = values.into_iter();
                    let mut entries = Vec::new();
                    while let Some(key) = values.next() {
                        let value = values.next().expect("mapping length was checked");
                        entries.push((key, value));
                    }
                    self.attach(ParsedNode::Mapping {
                        entries,
                        anchor,
                        tag,
                        span: ParsedSpan::cover(start.start, parser_span.end),
                    });
                }
                _ => self.fail("YAML mapping events are unbalanced", parser_span),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarKind {
    String,
    Null,
    Boolean,
    Integer,
    Float,
}

enum ProfileTask<'node> {
    Visit(&'node ParsedNode),
    MappingEntry {
        key: &'node ParsedNode,
        value: &'node ParsedNode,
        mapping: usize,
    },
    CheckMappingKey {
        key: &'node ParsedNode,
        mapping: usize,
        diagnostic_count: usize,
    },
}

fn validate_profile(
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut tasks = vec![ProfileTask::Visit(node)];
    let mut mapping_keys = Vec::<HashMap<String, ParsedSpan>>::new();
    while let Some(task) = tasks.pop() {
        match task {
            ProfileTask::Visit(ParsedNode::Alias { span }) => diagnostics.push(profile_diagnostic(
                "YAML aliases are not permitted in schema documents",
                source_map,
                *span,
                "alias",
            )),
            ProfileTask::Visit(ParsedNode::Scalar {
                value,
                style,
                anchor,
                tag,
                span,
            }) => {
                validate_anchor(*anchor, source_map, *span, diagnostics);
                match resolve_scalar(value, *style, tag.as_ref()) {
                    Ok(ScalarKind::Null) => diagnostics.push(profile_diagnostic(
                        "null values are not permitted in schema documents",
                        source_map,
                        *span,
                        "null",
                    )),
                    Ok(_) => {}
                    Err(message) => diagnostics.push(profile_diagnostic(
                        message,
                        source_map,
                        *span,
                        "custom_tag",
                    )),
                }
            }
            ProfileTask::Visit(ParsedNode::Sequence {
                values,
                anchor,
                tag,
                span,
            }) => {
                validate_anchor(*anchor, source_map, *span, diagnostics);
                validate_collection_tag(tag.as_ref(), "seq", source_map, *span, diagnostics);
                tasks.extend(values.iter().rev().map(ProfileTask::Visit));
            }
            ProfileTask::Visit(ParsedNode::Mapping {
                entries,
                anchor,
                tag,
                span,
            }) => {
                validate_anchor(*anchor, source_map, *span, diagnostics);
                validate_collection_tag(tag.as_ref(), "map", source_map, *span, diagnostics);
                let mapping = mapping_keys.len();
                mapping_keys.push(HashMap::new());
                tasks.extend(
                    entries
                        .iter()
                        .rev()
                        .map(|(key, value)| ProfileTask::MappingEntry {
                            key,
                            value,
                            mapping,
                        }),
                );
            }
            ProfileTask::MappingEntry {
                key,
                value,
                mapping,
            } => {
                let diagnostic_count = diagnostics.len();
                tasks.push(ProfileTask::Visit(value));
                tasks.push(ProfileTask::CheckMappingKey {
                    key,
                    mapping,
                    diagnostic_count,
                });
                tasks.push(ProfileTask::Visit(key));
            }
            ProfileTask::CheckMappingKey {
                key,
                mapping,
                diagnostic_count,
            } => {
                if diagnostics.len() == diagnostic_count {
                    match string_key(key) {
                        Ok((_name, is_merge)) if is_merge => diagnostics.push(profile_diagnostic(
                            "YAML merge keys are not permitted in schema documents",
                            source_map,
                            key.span(),
                            "merge_key",
                        )),
                        Ok((name, _)) => {
                            if let Some(first) = mapping_keys[mapping].get(name).copied() {
                                diagnostics.push(
                                    Diagnostic::new(
                                        SchemaDiagnosticCode::DuplicateKey,
                                        format!("mapping key {name:?} is declared more than once"),
                                        Some(parser_span(source_map, key.span())),
                                    )
                                    .with_related(RelatedDiagnostic::new(
                                        "first declaration of this key",
                                        parser_span(source_map, first),
                                    ))
                                    .with_detail("key", name.to_owned()),
                                );
                            } else {
                                mapping_keys[mapping].insert(name.to_owned(), key.span());
                            }
                        }
                        Err(()) => diagnostics.push(profile_diagnostic(
                            "every YAML mapping key must resolve to a string",
                            source_map,
                            key.span(),
                            "non_string_key",
                        )),
                    }
                }
            }
        }
    }
}

fn validate_anchor(
    anchor: Option<ParsedSpan>,
    source_map: &SourceMap<'_>,
    _span: ParsedSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(anchor) = anchor {
        diagnostics.push(profile_diagnostic(
            "YAML anchors are not permitted in schema documents",
            source_map,
            anchor,
            "anchor",
        ));
    }
}

fn validate_collection_tag(
    tag: Option<&Tag>,
    expected: &'static str,
    source_map: &SourceMap<'_>,
    span: ParsedSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(tag) = tag else {
        return;
    };
    if is_non_specific_tag(tag) || core_tag_suffix(tag) == Some(expected) {
        return;
    }
    diagnostics.push(profile_diagnostic(
        "custom or incompatible YAML tags are not permitted in schema documents",
        source_map,
        span,
        "custom_tag",
    ));
}

fn profile_diagnostic(
    message: impl Into<String>,
    source_map: &SourceMap<'_>,
    span: ParsedSpan,
    feature: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::Syntax,
        message,
        Some(parser_span(source_map, span)),
    )
    .with_detail("feature", feature)
}

fn string_key(node: &ParsedNode) -> Result<(&str, bool), ()> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return Err(());
    };
    if resolve_scalar(value, *style, tag.as_ref()).map_err(|_| ())? != ScalarKind::String {
        return Err(());
    }
    let is_merge = *style == ScalarStyle::Plain && tag.is_none() && value == "<<";
    Ok((value, is_merge))
}

fn resolve_scalar(
    value: &str,
    style: ScalarStyle,
    tag: Option<&Tag>,
) -> Result<ScalarKind, &'static str> {
    if let Some(tag) = tag {
        if is_non_specific_tag(tag) {
            return Ok(ScalarKind::String);
        }
        let Some(suffix) = core_tag_suffix(tag) else {
            return Err("custom YAML tags are not permitted in schema documents");
        };
        return match suffix {
            "str" => Ok(ScalarKind::String),
            "null" => Ok(ScalarKind::Null),
            "bool" if is_core_boolean(value) => Ok(ScalarKind::Boolean),
            "int" if is_core_integer(value) => Ok(ScalarKind::Integer),
            "float" if is_core_float(value) => Ok(ScalarKind::Float),
            "bool" | "int" | "float" => {
                Err("explicit YAML core tag has an invalid scalar representation")
            }
            _ => Err("custom or incompatible YAML tags are not permitted in schema documents"),
        };
    }
    if style != ScalarStyle::Plain {
        return Ok(ScalarKind::String);
    }
    if is_core_null(value) {
        Ok(ScalarKind::Null)
    } else if is_core_boolean(value) {
        Ok(ScalarKind::Boolean)
    } else if is_core_integer(value) {
        Ok(ScalarKind::Integer)
    } else if is_core_float(value) {
        Ok(ScalarKind::Float)
    } else {
        Ok(ScalarKind::String)
    }
}

fn is_non_specific_tag(tag: &Tag) -> bool {
    tag.handle.is_empty() && tag.suffix == "!"
}

fn core_tag_suffix(tag: &Tag) -> Option<&str> {
    if tag.is_yaml_core_schema() {
        Some(tag.suffix.as_str())
    } else if tag.handle.is_empty() {
        tag.suffix.strip_prefix("tag:yaml.org,2002:")
    } else {
        None
    }
}

fn is_core_null(value: &str) -> bool {
    matches!(value, "" | "~" | "null" | "Null" | "NULL")
}

fn is_core_boolean(value: &str) -> bool {
    matches!(
        value,
        "true" | "True" | "TRUE" | "false" | "False" | "FALSE"
    )
}

fn is_core_integer(value: &str) -> bool {
    // YAML 1.2.2 Core permits a sign only on the base-ten alternative.
    let decimal = value.strip_prefix(['+', '-']).unwrap_or(value);
    if !decimal.is_empty() && decimal.bytes().all(|byte| byte.is_ascii_digit()) {
        return true;
    }
    value.strip_prefix("0o").is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| matches!(byte, b'0'..=b'7'))
    }) || value.strip_prefix("0x").is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn is_core_float(value: &str) -> bool {
    if matches!(
        value,
        ".nan"
            | ".NaN"
            | ".NAN"
            | ".inf"
            | ".Inf"
            | ".INF"
            | "+.inf"
            | "+.Inf"
            | "+.INF"
            | "-.inf"
            | "-.Inf"
            | "-.INF"
    ) {
        return true;
    }
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    let exponent = unsigned.find(['e', 'E']);
    let (mantissa, exponent_digits) = match exponent {
        Some(index) => {
            if unsigned[index + 1..].contains(['e', 'E']) {
                return false;
            }
            (&unsigned[..index], Some(&unsigned[index + 1..]))
        }
        None => (unsigned, None),
    };
    if let Some(exponent_digits) = exponent_digits {
        let exponent_digits = exponent_digits
            .strip_prefix(['+', '-'])
            .unwrap_or(exponent_digits);
        if exponent_digits.is_empty() || !exponent_digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return false;
        }
    }
    if let Some(fraction) = mantissa.strip_prefix('.') {
        !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
    } else if let Some((whole, fraction)) = mantissa.split_once('.') {
        !whole.is_empty()
            && whole.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.bytes().all(|byte| byte.is_ascii_digit())
            && !fraction.contains('.')
    } else {
        !mantissa.is_empty() && mantissa.bytes().all(|byte| byte.is_ascii_digit())
    }
}

fn decode_v1_document(
    source_map: &SourceMap<'_>,
    root: &ParsedNode,
) -> Result<SchemaDocument, DocumentDecodeFailure> {
    let ParsedNode::Mapping {
        entries,
        span: root_span,
        ..
    } = root
    else {
        return Err(invalid_declaration(
            "schema document root must be a mapping",
            source_map,
            root.span(),
            "root",
        )
        .into());
    };

    let (format_key, format_value) =
        required_entry(entries, "format_version", "root", source_map, *root_span)?;
    let format_version = decode_format_version(format_key, format_value, source_map)?;

    let unknown_keys = v1_unknown_key_diagnostics(entries, source_map);
    if !unknown_keys.is_empty() {
        return Err(DocumentDecodeFailure::Diagnostics(unknown_keys));
    }

    let (schema_key, schema_value) =
        required_entry(entries, "schema", "root", source_map, *root_span)?;
    let schema_identity = decode_schema_identity(schema_value, source_map)?;
    let schema = field(source_map, schema_key, schema_value, schema_identity);

    let (identity_key, identity_value) =
        required_entry(entries, "identity", "root", source_map, *root_span)?;
    let identity_configuration = decode_identity(identity_value, source_map)?;
    let identity = field(
        source_map,
        identity_key,
        identity_value,
        identity_configuration,
    );

    let (flavours_key, flavours_value) =
        required_entry(entries, "flavours", "root", source_map, *root_span)?;
    let flavours_len = mapping_len(flavours_value).ok_or_else(|| {
        invalid_declaration(
            "root.flavours must be a mapping",
            source_map,
            flavours_value.span(),
            "flavours",
        )
    })?;
    let flavours = section(source_map, flavours_key, flavours_value, flavours_len);

    let relations = if let Some((key, value)) = optional_entry(entries, "relations") {
        let Some(len) = mapping_len(value) else {
            return Err(invalid_declaration(
                "root.relations must be a mapping",
                source_map,
                value.span(),
                "relations",
            )
            .into());
        };
        Some(section(source_map, key, value, len))
    } else {
        None
    };

    let rules = if let Some((key, value)) = optional_entry(entries, "rules") {
        let Some(len) = sequence_len(value) else {
            return Err(invalid_declaration(
                "root.rules must be a sequence",
                source_map,
                value.span(),
                "rules",
            )
            .into());
        };
        Some(section(source_map, key, value, len))
    } else {
        None
    };

    let (end_line, end_column) = position_at_valid_prefix(source_map.source.as_bytes());
    let document_source = source_span(
        &source_map.source_index,
        0,
        source_map.source.len(),
        1,
        1,
        end_line,
        end_column,
    );
    Ok(SchemaDocument::new(
        document_source,
        format_version,
        schema,
        identity,
        flavours,
        relations,
        rules,
    ))
}

fn decode_format_version(
    key: &ParsedNode,
    value: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> DecodeResult<SchemaField<u32>> {
    let ParsedNode::Scalar {
        value: raw,
        style,
        tag,
        span,
        ..
    } = value
    else {
        return Err(Box::new(invalid_declaration(
            "format_version must be the integer 1",
            source_map,
            value.span(),
            "format_version",
        )));
    };
    if resolve_scalar(raw, *style, tag.as_ref()).ok() != Some(ScalarKind::Integer)
        || raw.is_empty()
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Box::new(invalid_declaration(
            "format_version must be an unsigned base-ten YAML integer",
            source_map,
            *span,
            "format_version",
        )));
    }
    let significant = raw.trim_start_matches('0');
    if significant != "1" {
        return Err(Box::new(
            Diagnostic::new(
                SchemaDiagnosticCode::UnsupportedFormat,
                format!("schema format version {raw} is not supported"),
                Some(parser_span(source_map, *span)),
            )
            .with_context(DiagnosticContext::new(
                Some("format_version".to_owned()),
                None,
                None,
            ))
            .with_detail("format_version", raw.to_owned()),
        ));
    }
    Ok(field(source_map, key, value, 1))
}

fn decode_schema_identity(
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> DecodeResult<SchemaIdentity> {
    let ParsedNode::Mapping { entries, span, .. } = node else {
        return Err(Box::new(invalid_declaration(
            "root.schema must be a mapping",
            source_map,
            node.span(),
            "schema",
        )));
    };
    let (name_key, name_value) = required_entry(entries, "name", "schema", source_map, *span)?;
    let name = expect_string(name_value, "schema.name", source_map)?;
    if !valid_kebab_name(name) {
        return Err(Box::new(invalid_name(
            "schema.name must match [a-z][a-z0-9]*(?:-[a-z0-9]+)*",
            source_map,
            name_value.span(),
            "schema.name",
            name,
        )));
    }
    let name = field(source_map, name_key, name_value, name.to_owned());

    let (version_key, version_value) =
        required_entry(entries, "version", "schema", source_map, *span)?;
    let version = expect_string(version_value, "schema.version", source_map)?;
    if !valid_semver(version) {
        return Err(Box::new(
            invalid_declaration(
                "schema.version must use SemVer 2.0.0 syntax",
                source_map,
                version_value.span(),
                "schema.version",
            )
            .with_detail("value", version.to_owned()),
        ));
    }
    let version = field(source_map, version_key, version_value, version.to_owned());
    Ok(SchemaIdentity::new(name, version))
}

fn decode_identity(
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> DecodeResult<IdentityConfiguration> {
    let ParsedNode::Mapping { entries, span, .. } = node else {
        return Err(Box::new(invalid_declaration(
            "root.identity must be a mapping",
            source_map,
            node.span(),
            "identity",
        )));
    };
    let (mid_key, mid_value) = required_entry(entries, "mid", "identity", source_map, *span)?;
    let ParsedNode::Mapping {
        entries: mid_entries,
        span: mid_span,
        ..
    } = mid_value
    else {
        return Err(Box::new(invalid_declaration(
            "identity.mid must be a mapping",
            source_map,
            mid_value.span(),
            "identity.mid",
        )));
    };
    let (format_key, format_value) =
        required_entry(mid_entries, "format", "identity.mid", source_map, *mid_span)?;
    let format = expect_string(format_value, "identity.mid.format", source_map)?;
    if format != "ulid" {
        return Err(Box::new(
            invalid_declaration(
                "identity.mid.format must be \"ulid\" in format version 1",
                source_map,
                format_value.span(),
                "identity.mid.format",
            )
            .with_detail("value", format.to_owned()),
        ));
    }
    let format = field(source_map, format_key, format_value, MidFormat::Ulid);

    let (prefix_key, prefix_value) =
        required_entry(mid_entries, "prefix", "identity.mid", source_map, *mid_span)?;
    let prefix = expect_string(prefix_value, "identity.mid.prefix", source_map)?;
    if !valid_mid_prefix(prefix) {
        return Err(Box::new(invalid_name(
            "identity.mid.prefix must match [a-z][a-z0-9]*_",
            source_map,
            prefix_value.span(),
            "identity.mid.prefix",
            prefix,
        )));
    }
    let prefix = field(source_map, prefix_key, prefix_value, prefix.to_owned());
    let mid = field(
        source_map,
        mid_key,
        mid_value,
        MidIdentity::new(format, prefix),
    );
    Ok(IdentityConfiguration::new(mid))
}

fn v1_unknown_key_diagnostics(
    entries: &[(ParsedNode, ParsedNode)],
    source_map: &SourceMap<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_unknown_key_diagnostics(
        entries,
        &[
            "format_version",
            "schema",
            "identity",
            "flavours",
            "relations",
            "rules",
        ],
        "root",
        source_map,
        &mut diagnostics,
    );

    if let Some((_, ParsedNode::Mapping { entries, .. })) = optional_entry(entries, "schema") {
        collect_unknown_key_diagnostics(
            entries,
            &["name", "version"],
            "schema",
            source_map,
            &mut diagnostics,
        );
    }
    if let Some((_, ParsedNode::Mapping { entries, .. })) = optional_entry(entries, "identity") {
        collect_unknown_key_diagnostics(
            entries,
            &["mid"],
            "identity",
            source_map,
            &mut diagnostics,
        );
        if let Some((_, ParsedNode::Mapping { entries, .. })) = optional_entry(entries, "mid") {
            collect_unknown_key_diagnostics(
                entries,
                &["format", "prefix"],
                "identity.mid",
                source_map,
                &mut diagnostics,
            );
        }
    }
    diagnostics
}

fn collect_unknown_key_diagnostics(
    entries: &[(ParsedNode, ParsedNode)],
    allowed: &[&str],
    mapping: &'static str,
    source_map: &SourceMap<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (key, _) in entries {
        let (name, _) = string_key(key).expect("profile validation requires string keys");
        if !allowed.contains(&name) {
            diagnostics.push(
                Diagnostic::new(
                    SchemaDiagnosticCode::UnknownKey,
                    format!("{mapping} contains unknown key {name:?}"),
                    Some(parser_span(source_map, key.span())),
                )
                .with_context(DiagnosticContext::new(Some(name.to_owned()), None, None))
                .with_detail("key", name.to_owned())
                .with_detail("mapping", mapping),
            );
        }
    }
}

fn required_entry<'a>(
    entries: &'a [(ParsedNode, ParsedNode)],
    name: &'static str,
    mapping: &'static str,
    source_map: &SourceMap<'_>,
    mapping_span: ParsedSpan,
) -> DecodeResult<(&'a ParsedNode, &'a ParsedNode)> {
    optional_entry(entries, name).ok_or_else(|| {
        Box::new(
            invalid_declaration(
                format!("{mapping} is missing required key {name:?}"),
                source_map,
                mapping_span,
                name,
            )
            .with_detail("key", name)
            .with_detail("mapping", mapping),
        )
    })
}

fn optional_entry<'a>(
    entries: &'a [(ParsedNode, ParsedNode)],
    name: &str,
) -> Option<(&'a ParsedNode, &'a ParsedNode)> {
    entries.iter().find_map(|(key, value)| {
        let (key_name, _) = string_key(key).expect("profile validation requires string keys");
        (key_name == name).then_some((key, value))
    })
}

fn expect_string<'a>(
    node: &'a ParsedNode,
    field: &'static str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<&'a str> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return Err(Box::new(invalid_declaration(
            format!("{field} must be a string"),
            source_map,
            node.span(),
            field,
        )));
    };
    if resolve_scalar(value, *style, tag.as_ref()).ok() != Some(ScalarKind::String) {
        return Err(Box::new(invalid_declaration(
            format!("{field} must be a string"),
            source_map,
            node.span(),
            field,
        )));
    }
    Ok(value)
}

fn mapping_len(node: &ParsedNode) -> Option<usize> {
    match node {
        ParsedNode::Mapping { entries, .. } => Some(entries.len()),
        _ => None,
    }
}

fn sequence_len(node: &ParsedNode) -> Option<usize> {
    match node {
        ParsedNode::Sequence { values, .. } => Some(values.len()),
        _ => None,
    }
}

fn field<T>(
    source_map: &SourceMap<'_>,
    key: &ParsedNode,
    value: &ParsedNode,
    decoded: T,
) -> SchemaField<T> {
    SchemaField::new(
        parser_span(source_map, key.span()),
        parser_span(source_map, value.span()),
        decoded,
    )
}

fn section(
    source_map: &SourceMap<'_>,
    key: &ParsedNode,
    value: &ParsedNode,
    len: usize,
) -> SchemaSection {
    SchemaSection::new(
        parser_span(source_map, key.span()),
        parser_span(source_map, value.span()),
        len,
    )
}

fn invalid_declaration(
    message: impl Into<String>,
    source_map: &SourceMap<'_>,
    span: ParsedSpan,
    field: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::InvalidDeclaration,
        message,
        Some(parser_span(source_map, span)),
    )
    .with_context(DiagnosticContext::new(Some(field.to_owned()), None, None))
}

fn invalid_name(
    message: impl Into<String>,
    source_map: &SourceMap<'_>,
    span: ParsedSpan,
    field: &'static str,
    value: &str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::InvalidName,
        message,
        Some(parser_span(source_map, span)),
    )
    .with_context(DiagnosticContext::new(Some(field.to_owned()), None, None))
    .with_detail("value", value.to_owned())
}

fn valid_kebab_name(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    valid_lower_alphanumeric_segment(first, true)
        && segments.all(|segment| valid_lower_alphanumeric_segment(segment, false))
}

fn valid_lower_alphanumeric_segment(value: &str, require_letter_first: bool) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if require_letter_first && !first.is_ascii_lowercase() {
        return false;
    }
    if !(require_letter_first || first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn valid_mid_prefix(value: &str) -> bool {
    let Some(stem) = value.strip_suffix('_') else {
        return false;
    };
    valid_lower_alphanumeric_segment(stem, true)
}

fn valid_semver(value: &str) -> bool {
    static SEMVER: OnceLock<Regex> = OnceLock::new();
    SEMVER
        .get_or_init(|| {
            Regex::new(concat!(
                r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)",
                r"(?:-(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)",
                r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?",
                r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$",
            ))
            .expect("the SemVer 2.0.0 syntax pattern is valid")
        })
        .is_match(value)
}

fn parser_span(source_map: &SourceMap<'_>, span: ParsedSpan) -> SourceSpan {
    source_span(
        &source_map.source_index,
        span.start.byte,
        span.end.byte,
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}

fn marker_span(source_map: &SourceMap<'_>, marker: ParsedPosition) -> SourceSpan {
    source_span(
        &source_map.source_index,
        marker.byte,
        marker.byte,
        marker.line,
        marker.column,
        marker.line,
        marker.column,
    )
}

#[allow(clippy::too_many_arguments)]
fn source_span(
    source_index: &SourceIndex,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
) -> SourceSpan {
    source_index
        .try_span(
            start_byte as u64,
            end_byte as u64,
            start_line as u64,
            start_column as u64,
            end_line as u64,
            end_column as u64,
        )
        .expect("parser and rooted project produced a valid Mara source span")
}

fn position_at_valid_prefix(prefix: &[u8]) -> (usize, usize) {
    let text = std::str::from_utf8(prefix).expect("UTF-8 error prefix is valid");
    let mut line = 1;
    let mut column = 1;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                line += 1;
                column = 1;
            }
            '\n' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
    }
    (line, column)
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        let left_primary = left.primary();
        let right_primary = right.primary();
        match (left_primary, right_primary) {
            (Some(left), Some(right)) => left
                .path()
                .as_bytes()
                .cmp(right.path().as_bytes())
                .then_with(|| left.start_byte().cmp(&right.start_byte())),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| severity_rank(left.severity()).cmp(&severity_rank(right.severity())))
        .then_with(|| left.code().as_str().cmp(right.code().as_str()))
        .then_with(|| {
            canonical_details_bytes(left.details()).cmp(&canonical_details_bytes(right.details()))
        })
    });
}

fn canonical_details_bytes(details: &BTreeMap<String, DiagnosticValue>) -> Vec<u8> {
    let value = serde_json::Value::Object(
        details
            .iter()
            .map(|(key, value)| (key.clone(), diagnostic_json_value(value)))
            .collect(),
    );
    let mut bytes = serde_json::to_vec_pretty(&value)
        .expect("Mara diagnostic details always contain serializable JSON values");
    bytes.push(b'\n');
    bytes
}

fn diagnostic_json_value(value: &DiagnosticValue) -> serde_json::Value {
    match value {
        DiagnosticValue::Null => serde_json::Value::Null,
        DiagnosticValue::Boolean(value) => serde_json::Value::Bool(*value),
        DiagnosticValue::Integer(value) => serde_json::Value::Number((*value).into()),
        DiagnosticValue::Unsigned(value) => serde_json::Value::Number((*value).into()),
        DiagnosticValue::Number(value) => serde_json::Value::Number(
            serde_json::Number::from_f64(value.get()).expect("Mara diagnostic numbers are finite"),
        ),
        DiagnosticValue::String(value) => serde_json::Value::String(value.clone()),
        DiagnosticValue::Array(values) => {
            serde_json::Value::Array(values.iter().map(diagnostic_json_value).collect())
        }
        DiagnosticValue::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), diagnostic_json_value(value)))
                .collect(),
        ),
    }
}

const fn severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Info => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_scalar_resolution_matches_yaml_1_2_examples() {
        for value in ["null", "Null", "NULL", "~", ""] {
            assert_eq!(
                resolve_scalar(value, ScalarStyle::Plain, None),
                Ok(ScalarKind::Null)
            );
        }
        for value in ["true", "FALSE"] {
            assert_eq!(
                resolve_scalar(value, ScalarStyle::Plain, None),
                Ok(ScalarKind::Boolean)
            );
        }
        for value in ["0", "+12", "0o7", "0x3A", "-19"] {
            assert_eq!(
                resolve_scalar(value, ScalarStyle::Plain, None),
                Ok(ScalarKind::Integer)
            );
        }
        for value in ["0.", "-0.0", ".5", "+12e03", "-.Inf", ".NAN"] {
            assert_eq!(
                resolve_scalar(value, ScalarStyle::Plain, None),
                Ok(ScalarKind::Float)
            );
        }
        assert_eq!(
            resolve_scalar("on", ScalarStyle::Plain, None),
            Ok(ScalarKind::String)
        );
        assert_eq!(
            resolve_scalar("null", ScalarStyle::DoubleQuoted, None),
            Ok(ScalarKind::String)
        );

        let explicit_integer = Tag {
            handle: "tag:yaml.org,2002:".to_owned(),
            suffix: "int".to_owned(),
        };
        for value in ["-0x1", "+0o7"] {
            assert_eq!(
                resolve_scalar(value, ScalarStyle::Plain, None),
                Ok(ScalarKind::String)
            );
            assert!(resolve_scalar(value, ScalarStyle::Plain, Some(&explicit_integer)).is_err());
        }
    }

    #[test]
    fn source_position_counts_unicode_scalars_and_crlf_once() {
        assert_eq!(position_at_valid_prefix("aé\r\nb".as_bytes()), (2, 2));
    }

    #[test]
    fn diagnostic_sorting_uses_canonical_details_as_the_final_tie_breaker() {
        let span = SourceSpan::try_new("schema.yaml", "x", 0, 1, 1, 1, 1, 2).unwrap();
        let mut diagnostics = vec![
            Diagnostic::new(
                SchemaDiagnosticCode::UnknownKey,
                "second",
                Some(span.clone()),
            )
            .with_detail("key", "z"),
            Diagnostic::new(SchemaDiagnosticCode::UnknownKey, "first", Some(span))
                .with_detail("key", "a"),
        ];

        sort_diagnostics(&mut diagnostics);

        assert_eq!(diagnostics[0].message(), "first");
        assert_eq!(diagnostics[1].message(), "second");
    }
}
