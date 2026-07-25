//! Strict loading and compilation of the configured v1 Mara schema document.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use mara_core::{
    CardinalityBound, CardinalityMaximum, DerivedSourceKind, Diagnostic, DiagnosticContext,
    DiagnosticSeverity, DiagnosticValue, DisplayIdDefinition, FieldDefinition, FieldRuleSelection,
    FieldType, FlavourDefinition, FlavourDefinitions, FlavourGuidance, IdentityConfiguration,
    MidFormat, MidIdentity, OrphanRule, RelatedDiagnostic, RelationCardinality, RelationDefinition,
    RelationDefinitions, RelationRuleSelection, RelationSourceEndpoint, RelationTargetEndpoint,
    RequiredBuiltInDefinition, RequiresFieldRule, RequiresRelationRule, RuleApplicability,
    RuleCondition, RuleConditionNumber, RuleConditionValue, RuleConfiguration, RuleCount,
    RuleDefinition, RuleDefinitions, RuleDirection, RuleKind, RuleSeverity, SchemaDiagnosticCode,
    SchemaDocument, SchemaField, SchemaIdentity, SchemaSection, SchemaValue, SourceIndex,
    SourceSpan,
};
use regex::Regex as UnicodeRegex;
use regex_lite::Regex as LiteRegex;
use saphyr_parser::{Event, Marker, Parser, ScalarStyle, Span, SpannedEventReceiver, Tag};

use crate::project::{LoadedProject, open_loaded_schema};

type DecodeResult<T> = Result<T, Box<Diagnostic>>;
type CompilationResult<T> = Result<T, Vec<Diagnostic>>;

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

impl From<Vec<Diagnostic>> for DocumentDecodeFailure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
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

    fn node_span(
        &self,
        span: Span,
        tag: Option<&Tag>,
        quoted_scalars: &[ParsedSpan],
    ) -> ParsedSpan {
        let mut parsed = self.span(span);
        self.expand_tag(&mut parsed, tag, quoted_scalars);
        parsed
    }

    fn scalar_span(
        &self,
        span: Span,
        style: ScalarStyle,
        tag: Option<&Tag>,
        quoted_scalars: &[ParsedSpan],
    ) -> ParsedSpan {
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
                quoted_scalars,
            )
        {
            parsed.start = self.position_at_byte(start_byte);
        }
        self.expand_tag(&mut parsed, tag, quoted_scalars);
        parsed
    }

    fn expand_tag(
        &self,
        parsed: &mut ParsedSpan,
        tag: Option<&Tag>,
        quoted_scalars: &[ParsedSpan],
    ) {
        if tag.is_none() {
            return;
        }
        if let Some(start_byte) = preceding_tag_start(
            self.source,
            &self.indicators.tags,
            parsed.start.byte,
            quoted_scalars,
        ) {
            parsed.start = self.position_at_byte(start_byte);
        }
    }

    fn anchor_span(
        &self,
        span: Span,
        anchor: usize,
        quoted_scalars: &[ParsedSpan],
    ) -> Option<ParsedSpan> {
        if anchor == 0 {
            return None;
        }
        let parser_span = self.span(span);
        let Some((start, end)) = preceding_anchor_range(
            self.source,
            &self.indicators.anchors,
            parser_span.start.byte,
            quoted_scalars,
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
    quoted_scalars: &[ParsedSpan],
) -> Option<usize> {
    let candidate_count = candidates.partition_point(|candidate| candidate.start < content_start);
    for candidate in candidates[..candidate_count].iter().rev() {
        if candidate.is_in_preceding_comment(content_start, quoted_scalars) {
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
    possible_comment_start: Option<usize>,
    comment_line_end: Option<usize>,
}

impl IndicatorCandidate {
    fn is_in_preceding_comment(self, node_start: usize, quoted_scalars: &[ParsedSpan]) -> bool {
        let Some(comment_start) = self.possible_comment_start else {
            return false;
        };
        if self
            .comment_line_end
            .is_none_or(|line_end| node_start <= line_end)
        {
            return false;
        }

        let containing_count =
            quoted_scalars.partition_point(|span| span.start.byte <= comment_start);
        !containing_count.checked_sub(1).is_some_and(|index| {
            let span = quoted_scalars[index];
            comment_start < span.end.byte
        })
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
        let line_end = line_start + line.len();
        let mut possible_comment_start = None;
        for (offset, character) in line.char_indices() {
            if character == '#'
                && (offset == 0
                    || line[..offset]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace))
            {
                possible_comment_start = Some(line_start + offset);
            }
            let candidate = IndicatorCandidate {
                start: line_start + offset,
                possible_comment_start,
                comment_line_end: possible_comment_start.map(|_| line_end),
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
    quoted_scalars: &[ParsedSpan],
) -> Option<usize> {
    let candidate_count = candidates.partition_point(|candidate| candidate.start < node_start);
    let mut earliest = None;
    for candidate in candidates[..candidate_count].iter().rev() {
        if candidate.is_in_preceding_comment(node_start, quoted_scalars) {
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
    quoted_scalars: &[ParsedSpan],
) -> Option<(usize, usize)> {
    let candidate_count = candidates.partition_point(|candidate| candidate.start < node_start);
    let mut earliest = None;
    for candidate in candidates[..candidate_count].iter().rev() {
        if candidate.is_in_preceding_comment(node_start, quoted_scalars) {
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
    quoted_scalars: Vec<ParsedSpan>,
    error: Option<(String, ParsedSpan)>,
}

impl<'map, 'source> TreeBuilder<'map, 'source> {
    fn new(source_map: &'map SourceMap<'source>) -> Self {
        Self {
            source_map,
            documents: Vec::new(),
            document_starts: Vec::new(),
            stack: Vec::new(),
            quoted_scalars: Vec::new(),
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
                let anchor = self
                    .source_map
                    .anchor_span(span, anchor, &self.quoted_scalars);
                let span =
                    self.source_map
                        .scalar_span(span, style, tag.as_deref(), &self.quoted_scalars);
                if matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted) {
                    self.quoted_scalars.push(parser_span);
                }
                self.attach(ParsedNode::Scalar {
                    value: value.into_owned(),
                    style,
                    anchor,
                    tag: tag.map(|tag| tag.into_owned()),
                    span,
                });
            }
            Event::SequenceStart(anchor, tag) => {
                let anchor = self
                    .source_map
                    .anchor_span(span, anchor, &self.quoted_scalars);
                let span = self
                    .source_map
                    .node_span(span, tag.as_deref(), &self.quoted_scalars);
                self.stack.push(PendingContainer::Sequence {
                    values: Vec::new(),
                    anchor,
                    tag: tag.map(|tag| tag.into_owned()),
                    start: span,
                });
            }
            Event::MappingStart(anchor, tag) => {
                let anchor = self
                    .source_map
                    .anchor_span(span, anchor, &self.quoted_scalars);
                let span = self
                    .source_map
                    .node_span(span, tag.as_deref(), &self.quoted_scalars);
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

    let mut diagnostics = v1_unknown_key_diagnostics(entries, source_map);
    let schema = collect_decode(
        required_entry(entries, "schema", "root", source_map, *root_span),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(decode_schema_identity(value, source_map), &mut diagnostics)
            .map(|decoded| field(source_map, key, value, decoded))
    });

    let identity = collect_decode(
        required_entry(entries, "identity", "root", source_map, *root_span),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(decode_identity(value, source_map), &mut diagnostics)
            .map(|decoded| field(source_map, key, value, decoded))
    });

    let flavour_namespaces = optional_entry(entries, "flavours")
        .and_then(|(_, value)| FlavourNamespaces::from_node(value, source_map));
    let flavours = collect_decode(
        required_entry(entries, "flavours", "root", source_map, *root_span),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(decode_flavours(key, value, source_map), &mut diagnostics)
    });

    let relations = if let Some((key, value)) = optional_entry(entries, "relations") {
        collect_compilation(
            decode_relations(key, value, flavour_namespaces.as_ref(), source_map),
            &mut diagnostics,
        )
    } else {
        None
    };

    let relation_namespaces = match optional_entry(entries, "relations") {
        Some((_, value)) => RelationNamespaces::from_node(value),
        None => Some(RelationNamespaces::default()),
    };
    let rules = if let Some((key, value)) = optional_entry(entries, "rules") {
        collect_compilation(
            decode_rules(
                key,
                value,
                flavour_namespaces.as_ref(),
                relation_namespaces.as_ref(),
                source_map,
            ),
            &mut diagnostics,
        )
    } else {
        None
    };

    if !diagnostics.is_empty() {
        return Err(DocumentDecodeFailure::Diagnostics(diagnostics));
    }

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
        schema.expect("valid schema identity compilation produced a value"),
        identity.expect("valid MID identity compilation produced a value"),
        flavours.expect("valid flavour compilation produced a value"),
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
) -> CompilationResult<SchemaIdentity> {
    let ParsedNode::Mapping { entries, span, .. } = node else {
        return Err(vec![invalid_declaration(
            "root.schema must be a mapping",
            source_map,
            node.span(),
            "schema",
        )]);
    };
    let mut diagnostics = Vec::new();
    let name = collect_decode(
        required_entry(entries, "name", "schema", source_map, *span).and_then(|(key, value)| {
            let name = expect_string(value, "schema.name", source_map)?;
            if !valid_kebab_name(name) {
                return Err(Box::new(invalid_name(
                    "schema.name must match [a-z][a-z0-9]*(?:-[a-z0-9]+)*",
                    source_map,
                    value.span(),
                    "schema.name",
                    name,
                )));
            }
            Ok(field(source_map, key, value, name.to_owned()))
        }),
        &mut diagnostics,
    );
    let version = collect_decode(
        required_entry(entries, "version", "schema", source_map, *span).and_then(|(key, value)| {
            let version = expect_string(value, "schema.version", source_map)?;
            if !valid_semver(version) {
                return Err(Box::new(
                    invalid_declaration(
                        "schema.version must use SemVer 2.0.0 syntax",
                        source_map,
                        value.span(),
                        "schema.version",
                    )
                    .with_detail("value", version.to_owned()),
                ));
            }
            Ok(field(source_map, key, value, version.to_owned()))
        }),
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(SchemaIdentity::new(
        name.expect("valid schema name compilation produced a value"),
        version.expect("valid schema version compilation produced a value"),
    ))
}

fn decode_identity(
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> CompilationResult<IdentityConfiguration> {
    let ParsedNode::Mapping { entries, span, .. } = node else {
        return Err(vec![invalid_declaration(
            "root.identity must be a mapping",
            source_map,
            node.span(),
            "identity",
        )]);
    };
    let (mid_key, mid_value) = required_entry(entries, "mid", "identity", source_map, *span)
        .map_err(|diagnostic| vec![*diagnostic])?;
    let ParsedNode::Mapping {
        entries: mid_entries,
        span: mid_span,
        ..
    } = mid_value
    else {
        return Err(vec![invalid_declaration(
            "identity.mid must be a mapping",
            source_map,
            mid_value.span(),
            "identity.mid",
        )]);
    };

    let mut diagnostics = Vec::new();
    let format = collect_decode(
        required_entry(mid_entries, "format", "identity.mid", source_map, *mid_span).and_then(
            |(key, value)| {
                let format = expect_string(value, "identity.mid.format", source_map)?;
                if format != "ulid" {
                    return Err(Box::new(
                        invalid_declaration(
                            "identity.mid.format must be \"ulid\" in format version 1",
                            source_map,
                            value.span(),
                            "identity.mid.format",
                        )
                        .with_detail("value", format.to_owned()),
                    ));
                }
                Ok(field(source_map, key, value, MidFormat::Ulid))
            },
        ),
        &mut diagnostics,
    );
    let prefix = collect_decode(
        required_entry(mid_entries, "prefix", "identity.mid", source_map, *mid_span).and_then(
            |(key, value)| {
                let prefix = expect_string(value, "identity.mid.prefix", source_map)?;
                if !valid_mid_prefix(prefix) {
                    return Err(Box::new(invalid_name(
                        "identity.mid.prefix must match [a-z][a-z0-9]*_",
                        source_map,
                        value.span(),
                        "identity.mid.prefix",
                        prefix,
                    )));
                }
                Ok(field(source_map, key, value, prefix.to_owned()))
            },
        ),
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mid = field(
        source_map,
        mid_key,
        mid_value,
        MidIdentity::new(
            format.expect("valid MID format compilation produced a value"),
            prefix.expect("valid MID prefix compilation produced a value"),
        ),
    );
    Ok(IdentityConfiguration::new(mid))
}

fn decode_flavours(
    key: &ParsedNode,
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> CompilationResult<FlavourDefinitions> {
    let entries = expect_mapping(node, "root.flavours", source_map)
        .map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();
    let mut declared_names = BTreeSet::new();
    for (name_node, _) in entries {
        let (name, _) = string_key(name_node).expect("profile validation requires string keys");
        if !valid_snake_name(name) {
            diagnostics.push(invalid_name(
                "flavour names must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                source_map,
                name_node.span(),
                "flavours",
                name,
            ));
            continue;
        }
        declared_names.insert(name.to_owned());
    }

    let mut definitions = BTreeMap::new();
    for (name_node, definition_node) in entries {
        let (name, _) = string_key(name_node).expect("profile validation requires string keys");
        let name_valid = valid_snake_name(name);
        let result = decode_flavour(
            name,
            name_node,
            definition_node,
            &declared_names,
            source_map,
        );
        if let Some(definition) = collect_compilation(result, &mut diagnostics)
            && name_valid
        {
            definitions.insert(name.to_owned(), definition);
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(FlavourDefinitions::new(
        parser_span(source_map, key.span()),
        parser_span(source_map, node.span()),
        definitions,
    ))
}

struct FlavourFieldNamespace {
    source: SourceSpan,
    field_type: Option<FieldType>,
    repeatable: Option<bool>,
    values: Option<BTreeSet<String>>,
    pattern: Option<String>,
}

struct FlavourNamespace {
    fields: BTreeMap<String, FlavourFieldNamespace>,
}

struct FlavourNamespaces {
    definitions: BTreeMap<String, FlavourNamespace>,
}

impl FlavourNamespaces {
    fn from_node(node: &ParsedNode, source_map: &SourceMap<'_>) -> Option<Self> {
        let ParsedNode::Mapping { entries, .. } = node else {
            return None;
        };
        let mut definitions = BTreeMap::new();
        for (name_node, definition_node) in entries {
            let (name, _) = string_key(name_node).expect("profile validation requires string keys");
            if !valid_snake_name(name) {
                continue;
            }
            let mut fields = BTreeMap::new();
            if let ParsedNode::Mapping { entries, .. } = definition_node
                && let Some((_, ParsedNode::Mapping { entries, .. })) =
                    optional_entry(entries, "fields")
            {
                for (field_node, definition_node) in entries {
                    let (field, _) =
                        string_key(field_node).expect("profile validation requires string keys");
                    let (field_type, repeatable, values, pattern) = if let ParsedNode::Mapping {
                        entries,
                        ..
                    } = definition_node
                    {
                        let field_type = optional_entry(entries, "type")
                            .and_then(|(_, value)| parsed_string(value))
                            .and_then(|value| match value {
                                "string" => Some(FieldType::String),
                                "integer" => Some(FieldType::Integer),
                                "number" => Some(FieldType::Number),
                                "boolean" => Some(FieldType::Boolean),
                                "enum" => Some(FieldType::Enum),
                                _ => None,
                            });
                        let repeatable = optional_entry(entries, "repeatable")
                            .and_then(|(_, value)| parsed_boolean(value));
                        let values = optional_entry(entries, "values").and_then(|(_, value)| {
                            let ParsedNode::Sequence { values, .. } = value else {
                                return None;
                            };
                            values
                                .iter()
                                .map(parsed_string)
                                .collect::<Option<BTreeSet<_>>>()
                                .map(|values| values.into_iter().map(str::to_owned).collect())
                        });
                        let pattern = optional_entry(entries, "pattern")
                            .and_then(|(_, value)| parsed_string(value))
                            .map(str::to_owned);
                        (field_type, repeatable, values, pattern)
                    } else {
                        (None, None, None, None)
                    };
                    fields.insert(
                        field.to_owned(),
                        FlavourFieldNamespace {
                            source: parser_span(source_map, field_node.span()),
                            field_type,
                            repeatable,
                            values,
                            pattern,
                        },
                    );
                }
            }
            definitions.insert(name.to_owned(), FlavourNamespace { fields });
        }
        Some(Self { definitions })
    }

    fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    fn field(&self, flavour: &str, name: &str) -> Option<&FlavourFieldNamespace> {
        self.definitions
            .get(flavour)
            .and_then(|namespace| namespace.fields.get(name))
    }
}

#[derive(Default)]
struct RelationNamespaces {
    definitions: BTreeSet<String>,
}

impl RelationNamespaces {
    fn from_node(node: &ParsedNode) -> Option<Self> {
        let ParsedNode::Mapping { entries, .. } = node else {
            return None;
        };
        Some(Self {
            definitions: entries
                .iter()
                .map(|(key, _)| {
                    string_key(key)
                        .expect("profile validation requires string keys")
                        .0
                        .to_owned()
                })
                .collect(),
        })
    }

    fn contains(&self, name: &str) -> bool {
        self.definitions.contains(name)
    }
}

fn decode_rules(
    key: &ParsedNode,
    node: &ParsedNode,
    flavour_namespaces: Option<&FlavourNamespaces>,
    relation_namespaces: Option<&RelationNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RuleDefinitions> {
    let values =
        expect_sequence(node, "root.rules", source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut definitions = Vec::with_capacity(values.len());
    let mut names = BTreeMap::<String, SourceSpan>::new();
    let mut diagnostics = Vec::new();

    for rule_node in values {
        let duplicate = rule_name_source(rule_node, source_map).is_some_and(|(name, source)| {
            if let Some(first_source) = names.get(name) {
                diagnostics.push(
                    invalid_declaration_at_source(
                        format!("rule name {name:?} is declared more than once"),
                        source,
                        "rules",
                    )
                    .with_related(RelatedDiagnostic::new(
                        "first rule with this name is declared here",
                        first_source.clone(),
                    ))
                    .with_detail("name", name.to_owned()),
                );
                true
            } else {
                names.insert(name.to_owned(), source);
                false
            }
        });
        let definition = collect_compilation(
            decode_rule(
                rule_node,
                flavour_namespaces,
                relation_namespaces,
                source_map,
            ),
            &mut diagnostics,
        );
        if !duplicate && let Some(definition) = definition {
            definitions.push(definition);
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RuleDefinitions::new(
        parser_span(source_map, key.span()),
        parser_span(source_map, node.span()),
        definitions,
    ))
}

fn rule_name_source<'a>(
    node: &'a ParsedNode,
    source_map: &SourceMap<'_>,
) -> Option<(&'a str, SourceSpan)> {
    let ParsedNode::Mapping { entries, .. } = node else {
        return None;
    };
    let (_, value) = optional_entry(entries, "name")?;
    let name = parsed_string(value)?;
    Some((name, parser_span(source_map, value.span())))
}

fn decode_rule(
    node: &ParsedNode,
    flavour_namespaces: Option<&FlavourNamespaces>,
    relation_namespaces: Option<&RelationNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RuleDefinition> {
    let entries =
        expect_mapping(node, "rules[]", source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();

    let name = collect_decode(
        required_entry(entries, "name", "rules[]", source_map, node.span()).and_then(
            |(key, value)| {
                let name = expect_string(value, "rules[].name", source_map)?;
                if !valid_snake_name(name) {
                    return Err(Box::new(invalid_name(
                        "rule names must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                        source_map,
                        value.span(),
                        "rules[].name",
                        name,
                    )));
                }
                Ok(field(source_map, key, value, name.to_owned()))
            },
        ),
        &mut diagnostics,
    );
    let kind = collect_decode(
        required_entry(entries, "kind", "rules[]", source_map, node.span()).and_then(
            |(key, value)| {
                let raw = expect_string(value, "rules[].kind", source_map)?;
                let kind = match raw {
                    "requires_relation" => RuleKind::RequiresRelation,
                    "requires_field" => RuleKind::RequiresField,
                    "orphan" => RuleKind::Orphan,
                    _ => {
                        return Err(Box::new(
                            invalid_declaration(
                                "rule kind must be requires_relation, requires_field, or orphan",
                                source_map,
                                value.span(),
                                "rules[].kind",
                            )
                            .with_detail("value", raw.to_owned()),
                        ));
                    }
                };
                Ok(field(source_map, key, value, kind))
            },
        ),
        &mut diagnostics,
    );
    let severity = collect_decode(
        required_entry(entries, "severity", "rules[]", source_map, node.span()).and_then(
            |(key, value)| {
                let raw = expect_string(value, "rules[].severity", source_map)?;
                let severity = match raw {
                    "error" => RuleSeverity::Error,
                    "warning" => RuleSeverity::Warning,
                    "info" => RuleSeverity::Info,
                    _ => {
                        return Err(Box::new(
                            invalid_declaration(
                                "rule severity must be error, warning, or info",
                                source_map,
                                value.span(),
                                "rules[].severity",
                            )
                            .with_detail("value", raw.to_owned()),
                        ));
                    }
                };
                Ok(field(source_map, key, value, severity))
            },
        ),
        &mut diagnostics,
    );
    let reference_flavours = rule_reference_flavours(entries);
    let applies_to = collect_decode(
        required_entry(entries, "applies_to", "rules[]", source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_rule_applicability(value, flavour_namespaces, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    let condition = optional_entry(entries, "when").and_then(|(key, value)| {
        collect_compilation(
            decode_rule_condition(
                value,
                reference_flavours.as_deref(),
                flavour_namespaces,
                source_map,
            ),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let configuration = kind.as_ref().and_then(|kind| {
        let result = match kind.value() {
            RuleKind::RequiresRelation => {
                decode_requires_relation_rule(entries, node.span(), relation_namespaces, source_map)
                    .map(RuleConfiguration::RequiresRelation)
            }
            RuleKind::RequiresField => decode_requires_field_rule(
                entries,
                node.span(),
                reference_flavours.as_deref(),
                flavour_namespaces,
                source_map,
            )
            .map(RuleConfiguration::RequiresField),
            RuleKind::Orphan => {
                decode_orphan_rule(entries, node.span(), relation_namespaces, source_map)
                    .map(RuleConfiguration::Orphan)
            }
        };
        collect_compilation(result, &mut diagnostics)
    });

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RuleDefinition::new(
        parser_span(source_map, node.span()),
        name.expect("valid rule name compilation produced a value"),
        kind.expect("valid rule kind compilation produced a value"),
        severity.expect("valid rule severity compilation produced a value"),
        applies_to.expect("valid rule applicability compilation produced a value"),
        condition,
        configuration.expect("valid rule configuration compilation produced a value"),
    ))
}

fn rule_reference_flavours(entries: &[(ParsedNode, ParsedNode)]) -> Option<Vec<String>> {
    let (_, ParsedNode::Mapping { entries, .. }) = optional_entry(entries, "applies_to")? else {
        return None;
    };
    let (_, ParsedNode::Sequence { values, .. }) = optional_entry(entries, "flavours")? else {
        return None;
    };
    Some(
        values
            .iter()
            .filter_map(parsed_string)
            .filter(|name| valid_snake_name(name))
            .map(str::to_owned)
            .collect(),
    )
}

fn decode_rule_applicability(
    node: &ParsedNode,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RuleApplicability> {
    let entries = expect_mapping(node, "rules[].applies_to", source_map)
        .map_err(|diagnostic| vec![*diagnostic])?;
    let (key, value) = required_entry(
        entries,
        "flavours",
        "rules[].applies_to",
        source_map,
        node.span(),
    )
    .map_err(|diagnostic| vec![*diagnostic])?;
    let flavours = decode_rule_flavour_sequence(
        value,
        "rules[].applies_to.flavours",
        flavour_namespaces,
        source_map,
    )?;
    Ok(RuleApplicability::new(field(
        source_map, key, value, flavours,
    )))
}

fn decode_rule_flavour_sequence(
    node: &ParsedNode,
    field_name: &str,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<String>>> {
    let values = decode_unique_string_sequence(node, field_name, true, source_map)?;
    let mut diagnostics = Vec::new();
    for value in &values {
        if !valid_snake_name(value.value()) {
            diagnostics.push(invalid_name_at_source(
                "rule flavour references must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                value.source().clone(),
                field_name,
                value.value(),
            ));
        } else if flavour_namespaces.is_some_and(|namespaces| !namespaces.contains(value.value())) {
            diagnostics.push(
                invalid_declaration_at_source(
                    format!(
                        "rule flavour {:?} is not declared by this schema",
                        value.value()
                    ),
                    value.source().clone(),
                    field_name,
                )
                .with_detail("flavour", value.value().clone()),
            );
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(values)
}

fn decode_rule_condition(
    node: &ParsedNode,
    applies_to: Option<&[String]>,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RuleCondition> {
    let entries =
        expect_mapping(node, "rules[].when", source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();
    let field_name = collect_decode(
        required_entry(entries, "field", "rules[].when", source_map, node.span()).and_then(
            |(key, value)| {
                let name = expect_string(value, "rules[].when.field", source_map)?;
                if !valid_snake_name(name) {
                    return Err(Box::new(invalid_name(
                        "condition field names must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                        source_map,
                        value.span(),
                        "rules[].when.field",
                        name,
                    )));
                }
                Ok(field(source_map, key, value, name.to_owned()))
            },
        ),
        &mut diagnostics,
    );
    let values = collect_decode(
        required_entry(entries, "in", "rules[].when", source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_unique_rule_condition_sequence(value, "rules[].when.in", source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    if let (Some(field_name), Some(values), Some(applies_to), Some(flavour_namespaces)) = (
        field_name.as_ref(),
        values.as_ref(),
        applies_to,
        flavour_namespaces,
    ) {
        diagnostics.extend(validate_rule_condition_references(
            field_name,
            values,
            applies_to,
            flavour_namespaces,
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RuleCondition::new(
        field_name.expect("valid condition field compilation produced a value"),
        values.expect("valid condition values compilation produced a value"),
    ))
}

fn validate_rule_condition_references(
    field: &SchemaField<String>,
    values: &SchemaField<Vec<SchemaValue<RuleConditionValue>>>,
    applies_to: &[String],
    flavour_namespaces: &FlavourNamespaces,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut fields = Vec::new();
    for flavour in applies_to {
        if !flavour_namespaces.contains(flavour) {
            continue;
        }
        let Some(definition) = flavour_namespaces.field(flavour, field.value()) else {
            diagnostics.push(
                invalid_declaration_at_source(
                    format!(
                        "condition field {:?} is not declared on applied flavour {:?}",
                        field.value(),
                        flavour
                    ),
                    field.value_source().clone(),
                    "rules[].when.field",
                )
                .with_detail("field", field.value().clone())
                .with_detail("flavour", flavour.clone()),
            );
            continue;
        };
        if definition.repeatable == Some(true) {
            diagnostics.push(
                invalid_declaration_at_source(
                    format!(
                        "condition field {:?} is repeatable on applied flavour {:?}",
                        field.value(),
                        flavour
                    ),
                    field.value_source().clone(),
                    "rules[].when.field",
                )
                .with_related(RelatedDiagnostic::new(
                    "repeatable field is declared here",
                    definition.source.clone(),
                ))
                .with_detail("field", field.value().clone())
                .with_detail("flavour", flavour.clone()),
            );
            continue;
        }
        fields.push(definition);
    }

    for value in values.value() {
        let mut known_domain = false;
        let mut accepted = false;
        for field in &fields {
            if let Some(valid) = rule_condition_value_is_valid(value.value(), field) {
                known_domain = true;
                accepted |= valid;
            }
        }
        if known_domain && !accepted {
            diagnostics.push(
                invalid_declaration_at_source(
                    format!(
                        "condition value {:?} is invalid for every applied flavour",
                        value.value()
                    ),
                    value.source().clone(),
                    "rules[].when.in",
                )
                .with_detail("value", rule_condition_value_detail(value.value())),
            );
        }
    }
    diagnostics
}

fn rule_condition_value_is_valid(
    value: &RuleConditionValue,
    field: &FlavourFieldNamespace,
) -> Option<bool> {
    match (field.field_type?, value) {
        (FieldType::String, RuleConditionValue::String(value)) => match field.pattern.as_deref() {
            Some(pattern) => UnicodeRegex::new(pattern).ok().map(|pattern| {
                pattern
                    .find(value)
                    .is_some_and(|found| found.start() == 0 && found.end() == value.len())
            }),
            None => Some(true),
        },
        (FieldType::Integer, RuleConditionValue::Integer(_))
        | (FieldType::Number, RuleConditionValue::Integer(_))
        | (FieldType::Number, RuleConditionValue::Number(_))
        | (FieldType::Boolean, RuleConditionValue::Boolean(_)) => Some(true),
        (FieldType::Enum, RuleConditionValue::String(value)) => {
            field.values.as_ref().map(|values| values.contains(value))
        }
        _ => Some(false),
    }
}

fn rule_condition_value_detail(value: &RuleConditionValue) -> DiagnosticValue {
    match value {
        RuleConditionValue::String(value) => DiagnosticValue::String(value.clone()),
        RuleConditionValue::Integer(value) => DiagnosticValue::Integer(*value),
        RuleConditionValue::Number(value) => DiagnosticValue::String(value.get().to_string()),
        RuleConditionValue::Boolean(value) => DiagnosticValue::Boolean(*value),
    }
}

fn valid_rule_integer_syntax(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty()
        && (digits == "0"
            || (!digits.starts_with('0') && digits.bytes().all(|byte| byte.is_ascii_digit())))
}

fn decode_requires_relation_rule(
    entries: &[(ParsedNode, ParsedNode)],
    rule_span: ParsedSpan,
    relation_namespaces: Option<&RelationNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RequiresRelationRule> {
    let mut diagnostics = Vec::new();
    let relations = collect_rule_relation_selection(
        entries,
        rule_span,
        relation_namespaces,
        source_map,
        &mut diagnostics,
    );
    let direction = collect_decode(
        required_entry(entries, "direction", "rules[]", source_map, rule_span).and_then(
            |(key, value)| {
                let raw = expect_string(value, "rules[].direction", source_map)?;
                let direction = match raw {
                    "outgoing" => RuleDirection::Outgoing,
                    "incoming" => RuleDirection::Incoming,
                    _ => {
                        return Err(Box::new(
                            invalid_declaration(
                                "rule direction must be outgoing or incoming",
                                source_map,
                                value.span(),
                                "rules[].direction",
                            )
                            .with_detail("value", raw.to_owned()),
                        ));
                    }
                };
                Ok(field(source_map, key, value, direction))
            },
        ),
        &mut diagnostics,
    );
    let count = collect_compilation(
        decode_rule_count(entries, rule_span, source_map),
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RequiresRelationRule::new(
        relations.expect("valid rule relation selection produced a value"),
        direction.expect("valid rule direction compilation produced a value"),
        count.expect("valid rule count compilation produced a value"),
    ))
}

fn collect_rule_relation_selection(
    entries: &[(ParsedNode, ParsedNode)],
    rule_span: ParsedSpan,
    relation_namespaces: Option<&RelationNamespaces>,
    source_map: &SourceMap<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RelationRuleSelection> {
    let relation = optional_entry(entries, "relation");
    let any_of = optional_entry(entries, "relation_any_of");
    if relation.is_some() == any_of.is_some() {
        diagnostics.push(invalid_declaration(
            "requires_relation rules require exactly one of relation or relation_any_of",
            source_map,
            relation.or(any_of).map_or(rule_span, |(key, _)| key.span()),
            "rules[]",
        ));
    }
    match (relation, any_of) {
        (Some((key, value)), None) => collect_compilation(
            decode_rule_relation_name(value, "rules[].relation", relation_namespaces, source_map),
            diagnostics,
        )
        .map(|name| RelationRuleSelection::Relation(field(source_map, key, value, name))),
        (None, Some((key, value))) => collect_compilation(
            decode_rule_relation_sequence(
                value,
                "rules[].relation_any_of",
                relation_namespaces,
                source_map,
            ),
            diagnostics,
        )
        .map(|names| RelationRuleSelection::AnyOf(field(source_map, key, value, names))),
        (Some((_, relation)), Some((_, any_of))) => {
            let _ = collect_compilation(
                decode_rule_relation_name(
                    relation,
                    "rules[].relation",
                    relation_namespaces,
                    source_map,
                ),
                diagnostics,
            );
            let _ = collect_compilation(
                decode_rule_relation_sequence(
                    any_of,
                    "rules[].relation_any_of",
                    relation_namespaces,
                    source_map,
                ),
                diagnostics,
            );
            None
        }
        (None, None) => None,
    }
}

fn decode_requires_field_rule(
    entries: &[(ParsedNode, ParsedNode)],
    rule_span: ParsedSpan,
    applies_to: Option<&[String]>,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RequiresFieldRule> {
    let mut diagnostics = Vec::new();
    let fields = collect_rule_field_selection(
        entries,
        rule_span,
        applies_to,
        flavour_namespaces,
        source_map,
        &mut diagnostics,
    );
    let count = collect_compilation(
        decode_rule_count(entries, rule_span, source_map),
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RequiresFieldRule::new(
        fields.expect("valid rule field selection produced a value"),
        count.expect("valid rule count compilation produced a value"),
    ))
}

fn collect_rule_field_selection(
    entries: &[(ParsedNode, ParsedNode)],
    rule_span: ParsedSpan,
    applies_to: Option<&[String]>,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<FieldRuleSelection> {
    let field_entry = optional_entry(entries, "field");
    let any_of = optional_entry(entries, "field_any_of");
    if field_entry.is_some() == any_of.is_some() {
        diagnostics.push(invalid_declaration(
            "requires_field rules require exactly one of field or field_any_of",
            source_map,
            field_entry
                .or(any_of)
                .map_or(rule_span, |(key, _)| key.span()),
            "rules[]",
        ));
    }
    match (field_entry, any_of) {
        (Some((key, value)), None) => collect_compilation(
            decode_rule_field_name(
                value,
                "rules[].field",
                applies_to,
                flavour_namespaces,
                source_map,
            ),
            diagnostics,
        )
        .map(|name| FieldRuleSelection::Field(field(source_map, key, value, name))),
        (None, Some((key, value))) => collect_compilation(
            decode_rule_field_sequence(
                value,
                "rules[].field_any_of",
                applies_to,
                flavour_namespaces,
                source_map,
            ),
            diagnostics,
        )
        .map(|names| FieldRuleSelection::AnyOf(field(source_map, key, value, names))),
        (Some((_, field)), Some((_, any_of))) => {
            let _ = collect_compilation(
                decode_rule_field_name(
                    field,
                    "rules[].field",
                    applies_to,
                    flavour_namespaces,
                    source_map,
                ),
                diagnostics,
            );
            let _ = collect_compilation(
                decode_rule_field_sequence(
                    any_of,
                    "rules[].field_any_of",
                    applies_to,
                    flavour_namespaces,
                    source_map,
                ),
                diagnostics,
            );
            None
        }
        (None, None) => None,
    }
}

fn decode_orphan_rule(
    entries: &[(ParsedNode, ParsedNode)],
    rule_span: ParsedSpan,
    relation_namespaces: Option<&RelationNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<OrphanRule> {
    let (key, value) = required_entry(entries, "relations", "rules[]", source_map, rule_span)
        .map_err(|diagnostic| vec![*diagnostic])?;
    let relations =
        decode_rule_relation_sequence(value, "rules[].relations", relation_namespaces, source_map)?;
    Ok(OrphanRule::new(field(source_map, key, value, relations)))
}

fn decode_rule_relation_name(
    node: &ParsedNode,
    field_name: &str,
    relation_namespaces: Option<&RelationNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<String> {
    let name =
        expect_string(node, field_name, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let source = parser_span(source_map, node.span());
    validate_rule_relation_reference(name, source, field_name, relation_namespaces)?;
    Ok(name.to_owned())
}

fn decode_rule_relation_sequence(
    node: &ParsedNode,
    field_name: &str,
    relation_namespaces: Option<&RelationNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<String>>> {
    let values = decode_unique_string_sequence(node, field_name, true, source_map)?;
    let mut diagnostics = Vec::new();
    for value in &values {
        if let Err(mut found) = validate_rule_relation_reference(
            value.value(),
            value.source().clone(),
            field_name,
            relation_namespaces,
        ) {
            diagnostics.append(&mut found);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(values)
}

fn validate_rule_relation_reference(
    name: &str,
    source: SourceSpan,
    field_name: &str,
    relation_namespaces: Option<&RelationNamespaces>,
) -> CompilationResult<()> {
    if !valid_snake_name(name) {
        return Err(vec![invalid_name_at_source(
            "rule relation references must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
            source,
            field_name,
            name,
        )]);
    }
    if relation_namespaces.is_some_and(|namespaces| !namespaces.contains(name)) {
        return Err(vec![
            invalid_declaration_at_source(
                format!("rule relation {name:?} is not declared by this schema"),
                source,
                field_name,
            )
            .with_detail("relation", name.to_owned()),
        ]);
    }
    Ok(())
}

fn decode_rule_field_name(
    node: &ParsedNode,
    field_name: &str,
    applies_to: Option<&[String]>,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<String> {
    let name =
        expect_string(node, field_name, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let source = parser_span(source_map, node.span());
    validate_rule_field_reference(name, source, field_name, applies_to, flavour_namespaces)?;
    Ok(name.to_owned())
}

fn decode_rule_field_sequence(
    node: &ParsedNode,
    field_name: &str,
    applies_to: Option<&[String]>,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<String>>> {
    let values = decode_unique_string_sequence(node, field_name, true, source_map)?;
    let mut diagnostics = Vec::new();
    for value in &values {
        if let Err(mut found) = validate_rule_field_reference(
            value.value(),
            value.source().clone(),
            field_name,
            applies_to,
            flavour_namespaces,
        ) {
            diagnostics.append(&mut found);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(values)
}

fn validate_rule_field_reference(
    name: &str,
    source: SourceSpan,
    field_name: &str,
    applies_to: Option<&[String]>,
    flavour_namespaces: Option<&FlavourNamespaces>,
) -> CompilationResult<()> {
    if !valid_snake_name(name) {
        return Err(vec![invalid_name_at_source(
            "rule field references must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
            source,
            field_name,
            name,
        )]);
    }
    let (Some(applies_to), Some(flavour_namespaces)) = (applies_to, flavour_namespaces) else {
        return Ok(());
    };
    let mut diagnostics = Vec::new();
    for flavour in applies_to {
        if !flavour_namespaces.contains(flavour) {
            continue;
        }
        if flavour_namespaces.field(flavour, name).is_none() {
            diagnostics.push(
                invalid_declaration_at_source(
                    format!(
                        "rule field {name:?} is not declared on applied flavour {:?}",
                        flavour
                    ),
                    source.clone(),
                    field_name,
                )
                .with_detail("field", name.to_owned())
                .with_detail("flavour", flavour.clone()),
            );
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(())
}

fn decode_rule_count(
    entries: &[(ParsedNode, ParsedNode)],
    rule_span: ParsedSpan,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RuleCount> {
    let mut diagnostics = Vec::new();
    let min = collect_decode(
        required_entry(entries, "min", "rules[]", source_map, rule_span).and_then(
            |(key, value)| {
                expect_non_negative_integer(value, "rules[].min", source_map)
                    .map(|decoded| field(source_map, key, value, decoded))
            },
        ),
        &mut diagnostics,
    );
    let max = optional_entry(entries, "max").and_then(|(key, value)| {
        collect_decode(
            expect_cardinality_maximum(value, "rules[].max", source_map)
                .map(|decoded| field(source_map, key, value, decoded)),
            &mut diagnostics,
        )
    });
    if let (Some(min), Some(maximum)) = (&min, &max)
        && let CardinalityMaximum::Bounded(maximum_value) = *maximum.value()
        && maximum_value < *min.value()
    {
        diagnostics.push(
            invalid_declaration_at_source(
                "rule max must be greater than or equal to min",
                maximum.value_source().clone(),
                "rules[].max",
            )
            .with_detail("max", DiagnosticValue::Unsigned(maximum_value))
            .with_detail("min", DiagnosticValue::Unsigned(*min.value())),
        );
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RuleCount::new(
        min.expect("valid rule minimum compilation produced a value"),
        max,
    ))
}

fn decode_relations(
    key: &ParsedNode,
    node: &ParsedNode,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RelationDefinitions> {
    let entries = expect_mapping(node, "root.relations", source_map)
        .map_err(|diagnostic| vec![*diagnostic])?;
    let mut definitions = BTreeMap::new();
    let mut authoring_names = Vec::new();
    let mut diagnostics = Vec::new();

    for (name_node, definition_node) in entries {
        let (name, _) = string_key(name_node).expect("profile validation requires string keys");
        let mut name_valid = true;
        if !valid_snake_name(name) {
            diagnostics.push(invalid_name(
                "relation names must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                source_map,
                name_node.span(),
                "relations",
                name,
            ));
            name_valid = false;
        }
        if let Some(kind) = reserved_authoring_name_kind(name) {
            diagnostics.push(
                invalid_name(
                    match kind {
                        ReservedAuthoringNameKind::BuiltIn => {
                            format!("relation name {name:?} collides with an item built-in")
                        }
                        ReservedAuthoringNameKind::Reserved => {
                            format!("relation name {name:?} is reserved")
                        }
                    },
                    source_map,
                    name_node.span(),
                    "relations",
                    name,
                )
                .with_detail("collision", kind.as_str())
                .with_detail("name", name.to_owned()),
            );
            name_valid = false;
        }

        if let Some(flavour_namespaces) = flavour_namespaces {
            authoring_names.extend(relation_authoring_names(
                name,
                name_node,
                definition_node,
                flavour_namespaces,
                source_map,
            ));
        }

        let definition = collect_compilation(
            decode_relation(
                name,
                name_node,
                definition_node,
                flavour_namespaces,
                source_map,
            ),
            &mut diagnostics,
        );
        if name_valid && let Some(definition) = definition {
            definitions.insert(name.to_owned(), definition);
        }
    }

    if let Some(flavour_namespaces) = flavour_namespaces {
        diagnostics.extend(validate_authoring_namespaces(
            authoring_names,
            flavour_namespaces,
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(RelationDefinitions::new(
        parser_span(source_map, key.span()),
        parser_span(source_map, node.span()),
        definitions,
    ))
}

fn decode_relation(
    name: &str,
    name_node: &ParsedNode,
    node: &ParsedNode,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RelationDefinition> {
    let mapping = format!("relations.{name}");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();

    let source = collect_decode(
        required_entry(entries, "source", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_relation_source(name, value, flavour_namespaces, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    let target = collect_decode(
        required_entry(entries, "target", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_relation_target(name, value, flavour_namespaces, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let inverse_entry = optional_entry(entries, "inverse");
    let inverse = inverse_entry.and_then(|(key, value)| {
        let field_name = format!("{mapping}.inverse");
        let inverse = collect_decode(
            expect_string(value, &field_name, source_map),
            &mut diagnostics,
        )?;
        if !valid_snake_name(inverse) {
            diagnostics.push(invalid_name(
                "relation inverse names must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                source_map,
                value.span(),
                &field_name,
                inverse,
            ));
            return None;
        }
        Some(field(source_map, key, value, inverse.to_owned()))
    });
    let inverse_authoring = collect_decode(
        decode_optional_boolean(entries, "inverse_authoring", &mapping, source_map),
        &mut diagnostics,
    )
    .flatten();
    let symmetric = collect_decode(
        decode_optional_boolean(entries, "symmetric", &mapping, source_map),
        &mut diagnostics,
    )
    .flatten();
    let same_flavour = collect_decode(
        decode_optional_boolean(entries, "same_flavour", &mapping, source_map),
        &mut diagnostics,
    )
    .flatten();
    let self_reference = collect_decode(
        decode_optional_boolean(entries, "self_reference", &mapping, source_map),
        &mut diagnostics,
    )
    .flatten();
    let acyclic = collect_decode(
        decode_optional_boolean(entries, "acyclic", &mapping, source_map),
        &mut diagnostics,
    )
    .flatten();
    let cardinality = optional_entry(entries, "cardinality").and_then(|(key, value)| {
        collect_compilation(
            decode_relation_cardinality(name, value, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let inverse_authoring_enabled = inverse_authoring
        .as_ref()
        .is_some_and(|enabled| *enabled.value());
    if inverse_authoring_enabled && inverse_entry.is_none() {
        let enabled = inverse_authoring
            .as_ref()
            .expect("enabled inverse authoring has a source field");
        diagnostics.push(invalid_declaration_at_source(
            "inverse_authoring: true requires an inverse name",
            enabled.value_source().clone(),
            &mapping,
        ));
    }

    let symmetric_enabled = symmetric.as_ref().is_some_and(|enabled| *enabled.value());
    if symmetric_enabled {
        if let Some((_, value)) = inverse_entry {
            diagnostics.push(invalid_declaration(
                "symmetric relations cannot declare an inverse name",
                source_map,
                value.span(),
                &mapping,
            ));
        }
        if let Some((_, value)) = optional_entry(entries, "inverse_authoring") {
            diagnostics.push(invalid_declaration(
                "symmetric relations cannot declare inverse_authoring",
                source_map,
                value.span(),
                &mapping,
            ));
        }
        if let (Some(source_flavours), Some(target_flavours)) = (
            authored_relation_flavours(entries, "source"),
            authored_relation_flavours(entries, "target"),
        ) && source_flavours != target_flavours
        {
            let target_source = optional_entry(entries, "target")
                .map(|(_, target)| parser_span(source_map, target.span()))
                .expect("a decoded target flavour set has a target mapping");
            diagnostics.push(invalid_declaration_at_source(
                "symmetric relations require identical source and target flavour sets",
                target_source,
                &mapping,
            ));
        }
    }

    let acyclic_enabled = acyclic.as_ref().is_some_and(|enabled| *enabled.value());
    if acyclic_enabled && let Some(external_key) = authored_external_target_key(entries) {
        diagnostics.push(
            invalid_declaration_at_source(
                "acyclic relations cannot permit external targets",
                acyclic
                    .as_ref()
                    .expect("enabled acyclicity has a source field")
                    .value_source()
                    .clone(),
                &mapping,
            )
            .with_related(RelatedDiagnostic::new(
                "external targets are declared here",
                parser_span(source_map, external_key.span()),
            )),
        );
    }

    if inverse_authoring_enabled
        && let Some(inverse) = &inverse
        && let Some(kind) = reserved_authoring_name_kind(inverse.value())
    {
        diagnostics.push(
            invalid_name_at_source(
                match kind {
                    ReservedAuthoringNameKind::BuiltIn => format!(
                        "enabled inverse name {:?} collides with an item built-in",
                        inverse.value()
                    ),
                    ReservedAuthoringNameKind::Reserved => {
                        format!("enabled inverse name {:?} is reserved", inverse.value())
                    }
                },
                inverse.value_source().clone(),
                &format!("{mapping}.inverse"),
                inverse.value(),
            )
            .with_detail("collision", kind.as_str())
            .with_detail("name", inverse.value().clone()),
        );
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(RelationDefinition::new(
        name.to_owned(),
        parser_span(source_map, name_node.span()),
        parser_span(source_map, node.span()),
        source.expect("valid relation source compilation produced a value"),
        target.expect("valid relation target compilation produced a value"),
        inverse,
        inverse_authoring,
        symmetric,
        same_flavour,
        self_reference,
        acyclic,
        cardinality,
    ))
}

fn decode_relation_source(
    relation: &str,
    node: &ParsedNode,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RelationSourceEndpoint> {
    let mapping = format!("relations.{relation}.source");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();
    let flavours_field = format!("{mapping}.flavours");
    let compiled_flavours = collect_decode(
        required_entry(entries, "flavours", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_relation_flavour_sequence(
                value,
                &flavours_field,
                flavour_namespaces,
                source_map,
            ),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    let derived = optional_entry(entries, "derived").and_then(|(key, value)| {
        collect_compilation(
            decode_derived_source_sequence(value, &format!("{mapping}.derived"), source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RelationSourceEndpoint::new(
        compiled_flavours.expect("valid source flavours compilation produced a value"),
        derived,
    ))
}

fn decode_relation_target(
    relation: &str,
    node: &ParsedNode,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RelationTargetEndpoint> {
    let mapping = format!("relations.{relation}.target");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();
    if optional_entry(entries, "flavours").is_none()
        && optional_entry(entries, "external").is_none()
    {
        diagnostics.push(invalid_declaration(
            "relation targets require flavours and/or external schemes",
            source_map,
            node.span(),
            &mapping,
        ));
    }
    let compiled_flavours = optional_entry(entries, "flavours").and_then(|(key, value)| {
        collect_compilation(
            decode_relation_flavour_sequence(
                value,
                &format!("{mapping}.flavours"),
                flavour_namespaces,
                source_map,
            ),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    let external = optional_entry(entries, "external").and_then(|(key, value)| {
        collect_compilation(
            decode_external_scheme_sequence(value, &format!("{mapping}.external"), source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RelationTargetEndpoint::new(compiled_flavours, external))
}

fn decode_relation_flavour_sequence(
    node: &ParsedNode,
    field_name: &str,
    flavour_namespaces: Option<&FlavourNamespaces>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<String>>> {
    let values =
        expect_sequence(node, field_name, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    if values.is_empty() {
        return Err(vec![invalid_declaration(
            format!("{field_name} must be a non-empty sequence"),
            source_map,
            node.span(),
            field_name,
        )]);
    }
    let mut compiled = Vec::with_capacity(values.len());
    let mut diagnostics = Vec::new();
    for value_node in values {
        let Some(value) = collect_decode(
            expect_string(value_node, field_name, source_map),
            &mut diagnostics,
        ) else {
            continue;
        };
        if !valid_snake_name(value) {
            diagnostics.push(invalid_name(
                "endpoint flavour names must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                source_map,
                value_node.span(),
                field_name,
                value,
            ));
            continue;
        }
        if flavour_namespaces.is_some_and(|namespaces| !namespaces.contains(value)) {
            diagnostics.push(
                invalid_declaration(
                    format!("endpoint flavour {value:?} is not declared by this schema"),
                    source_map,
                    value_node.span(),
                    field_name,
                )
                .with_detail("flavour", value.to_owned()),
            );
            continue;
        }
        compiled.push(SchemaValue::new(
            parser_span(source_map, value_node.span()),
            value.to_owned(),
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(compiled)
}

fn decode_derived_source_sequence(
    node: &ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<DerivedSourceKind>>> {
    let values =
        expect_sequence(node, field_name, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut compiled = Vec::with_capacity(values.len());
    let mut diagnostics = Vec::new();
    for value_node in values {
        let Some(value) = collect_decode(
            expect_string(value_node, field_name, source_map),
            &mut diagnostics,
        ) else {
            continue;
        };
        if value != "source_span" {
            diagnostics.push(
                invalid_declaration(
                    "derived source kind must be source_span in format version 1",
                    source_map,
                    value_node.span(),
                    field_name,
                )
                .with_detail("value", value.to_owned()),
            );
            continue;
        }
        compiled.push(SchemaValue::new(
            parser_span(source_map, value_node.span()),
            DerivedSourceKind::SourceSpan,
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(compiled)
}

fn decode_external_scheme_sequence(
    node: &ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<String>>> {
    let values =
        expect_sequence(node, field_name, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    if values.is_empty() {
        return Err(vec![invalid_declaration(
            format!("{field_name} must be a non-empty sequence"),
            source_map,
            node.span(),
            field_name,
        )]);
    }
    let mut compiled = Vec::with_capacity(values.len());
    let mut diagnostics = Vec::new();
    for value_node in values {
        let Some(value) = collect_decode(
            expect_string(value_node, field_name, source_map),
            &mut diagnostics,
        ) else {
            continue;
        };
        if !valid_external_scheme(value) {
            diagnostics.push(invalid_name(
                "external schemes must match [a-z][a-z0-9+.-]* and omit ://",
                source_map,
                value_node.span(),
                field_name,
                value,
            ));
            continue;
        }
        compiled.push(SchemaValue::new(
            parser_span(source_map, value_node.span()),
            value.to_owned(),
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(compiled)
}

fn decode_relation_cardinality(
    relation: &str,
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RelationCardinality> {
    let mapping = format!("relations.{relation}.cardinality");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();
    if optional_entry(entries, "outgoing").is_none()
        && optional_entry(entries, "incoming").is_none()
    {
        diagnostics.push(invalid_declaration(
            "relation cardinality requires outgoing and/or incoming bounds",
            source_map,
            node.span(),
            &mapping,
        ));
    }
    let outgoing = optional_entry(entries, "outgoing").and_then(|(key, value)| {
        collect_compilation(
            decode_cardinality_bound(value, &format!("{mapping}.outgoing"), source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    let incoming = optional_entry(entries, "incoming").and_then(|(key, value)| {
        collect_compilation(
            decode_cardinality_bound(value, &format!("{mapping}.incoming"), source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(RelationCardinality::new(outgoing, incoming))
}

fn decode_cardinality_bound(
    node: &ParsedNode,
    mapping: &str,
    source_map: &SourceMap<'_>,
) -> CompilationResult<CardinalityBound> {
    let entries =
        expect_mapping(node, mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();
    let min = optional_entry(entries, "min").and_then(|(key, value)| {
        collect_decode(
            expect_non_negative_integer(value, &format!("{mapping}.min"), source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    let max = optional_entry(entries, "max").and_then(|(key, value)| {
        collect_decode(
            expect_cardinality_maximum(value, &format!("{mapping}.max"), source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });
    if let Some(maximum) = &max
        && let CardinalityMaximum::Bounded(maximum_value) = *maximum.value()
        && maximum_value < min.as_ref().map_or(0, |minimum| *minimum.value())
    {
        diagnostics.push(
            invalid_declaration_at_source(
                "cardinality max must be greater than or equal to min",
                maximum.value_source().clone(),
                mapping,
            )
            .with_detail("max", DiagnosticValue::Unsigned(maximum_value))
            .with_detail(
                "min",
                DiagnosticValue::Unsigned(min.as_ref().map_or(0, |minimum| *minimum.value())),
            ),
        );
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(CardinalityBound::new(min, max))
}

fn expect_non_negative_integer(
    node: &ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<u64> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} must be a non-negative base-ten integer"),
            source_map,
            node.span(),
            field_name,
        )));
    };
    if resolve_scalar(value, *style, tag.as_ref()).ok() != Some(ScalarKind::Integer)
        || value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} must be a non-negative base-ten integer"),
            source_map,
            node.span(),
            field_name,
        )));
    }
    value.parse().map_err(|_| {
        Box::new(
            invalid_declaration(
                format!("{field_name} is too large to represent"),
                source_map,
                node.span(),
                field_name,
            )
            .with_detail("value", value.to_owned()),
        )
    })
}

fn expect_cardinality_maximum(
    node: &ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<CardinalityMaximum> {
    if let Ok(value) = expect_string(node, field_name, source_map) {
        if value == "many" {
            return Ok(CardinalityMaximum::Many);
        }
        return Err(Box::new(
            invalid_declaration(
                format!("{field_name} must be a non-negative integer or many"),
                source_map,
                node.span(),
                field_name,
            )
            .with_detail("value", value.to_owned()),
        ));
    }
    expect_non_negative_integer(node, field_name, source_map).map(CardinalityMaximum::Bounded)
}

#[derive(Debug, Clone, Copy)]
enum ReservedAuthoringNameKind {
    BuiltIn,
    Reserved,
}

impl ReservedAuthoringNameKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::BuiltIn => "built_in",
            Self::Reserved => "reserved",
        }
    }
}

fn reserved_authoring_name_kind(name: &str) -> Option<ReservedAuthoringNameKind> {
    match name {
        "id" | "title" | "body" => Some(ReservedAuthoringNameKind::BuiltIn),
        "mid" | "flavour" | "source_location" | "mentions" => {
            Some(ReservedAuthoringNameKind::Reserved)
        }
        _ => None,
    }
}

#[derive(Clone)]
struct AuthoringName {
    flavour: String,
    name: String,
    relation: String,
    kind: &'static str,
    source: SourceSpan,
}

fn relation_authoring_names(
    relation: &str,
    relation_node: &ParsedNode,
    node: &ParsedNode,
    flavour_namespaces: &FlavourNamespaces,
    source_map: &SourceMap<'_>,
) -> Vec<AuthoringName> {
    if !valid_snake_name(relation) || reserved_authoring_name_kind(relation).is_some() {
        return Vec::new();
    }
    let ParsedNode::Mapping { entries, .. } = node else {
        return Vec::new();
    };
    let mut names = relation_endpoint_flavours(entries, "source", flavour_namespaces)
        .into_iter()
        .map(|flavour| AuthoringName {
            flavour,
            name: relation.to_owned(),
            relation: relation.to_owned(),
            kind: "canonical",
            source: parser_span(source_map, relation_node.span()),
        })
        .collect::<Vec<_>>();

    let inverse_authoring_enabled = optional_entry(entries, "inverse_authoring")
        .and_then(|(_, value)| parsed_boolean(value))
        .unwrap_or(false);
    if inverse_authoring_enabled
        && let Some((_, inverse_node)) = optional_entry(entries, "inverse")
        && let Some(inverse) = parsed_string(inverse_node)
        && valid_snake_name(inverse)
        && reserved_authoring_name_kind(inverse).is_none()
    {
        names.extend(
            relation_endpoint_flavours(entries, "target", flavour_namespaces)
                .into_iter()
                .map(|flavour| AuthoringName {
                    flavour,
                    name: inverse.to_owned(),
                    relation: relation.to_owned(),
                    kind: "inverse",
                    source: parser_span(source_map, inverse_node.span()),
                }),
        );
    }
    names
}

fn relation_endpoint_flavours(
    relation_entries: &[(ParsedNode, ParsedNode)],
    endpoint: &str,
    flavour_namespaces: &FlavourNamespaces,
) -> BTreeSet<String> {
    let Some((_, ParsedNode::Mapping { entries, .. })) = optional_entry(relation_entries, endpoint)
    else {
        return BTreeSet::new();
    };
    let Some((_, ParsedNode::Sequence { values, .. })) = optional_entry(entries, "flavours") else {
        return BTreeSet::new();
    };
    values
        .iter()
        .filter_map(parsed_string)
        .filter(|flavour| valid_snake_name(flavour) && flavour_namespaces.contains(flavour))
        .map(str::to_owned)
        .collect()
}

fn authored_relation_flavours(
    relation_entries: &[(ParsedNode, ParsedNode)],
    endpoint: &str,
) -> Option<BTreeSet<String>> {
    let (_, endpoint_node) = optional_entry(relation_entries, endpoint)?;
    let ParsedNode::Mapping { entries, .. } = endpoint_node else {
        return None;
    };
    let Some((_, flavour_node)) = optional_entry(entries, "flavours") else {
        return (endpoint == "target").then(BTreeSet::new);
    };
    let ParsedNode::Sequence { values, .. } = flavour_node else {
        return None;
    };
    values
        .iter()
        .map(parsed_string)
        .map(|value| value.map(str::to_owned))
        .collect()
}

fn authored_external_target_key(
    relation_entries: &[(ParsedNode, ParsedNode)],
) -> Option<&ParsedNode> {
    let (_, ParsedNode::Mapping { entries, .. }) = optional_entry(relation_entries, "target")?
    else {
        return None;
    };
    optional_entry(entries, "external").map(|(key, _)| key)
}

fn parsed_string(node: &ParsedNode) -> Option<&str> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return None;
    };
    (resolve_scalar(value, *style, tag.as_ref()).ok() == Some(ScalarKind::String))
        .then_some(value)
        .map(String::as_str)
}

fn parsed_boolean(node: &ParsedNode) -> Option<bool> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return None;
    };
    (resolve_scalar(value, *style, tag.as_ref()).ok() == Some(ScalarKind::Boolean))
        .then_some(matches!(value.as_str(), "true" | "True" | "TRUE"))
}

fn validate_authoring_namespaces(
    mut names: Vec<AuthoringName>,
    flavour_namespaces: &FlavourNamespaces,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut occupied = BTreeMap::<(String, String), AuthoringName>::new();
    names.sort_by(|left, right| {
        (&left.flavour, &left.name, &left.relation, left.kind).cmp(&(
            &right.flavour,
            &right.name,
            &right.relation,
            right.kind,
        ))
    });
    names.dedup_by(|left, right| {
        left.flavour == right.flavour
            && left.name == right.name
            && left.relation == right.relation
            && left.kind == right.kind
    });
    for candidate in names {
        validate_authoring_name(
            candidate,
            flavour_namespaces,
            &mut occupied,
            &mut diagnostics,
        );
    }
    diagnostics
}

fn validate_authoring_name(
    candidate: AuthoringName,
    flavour_namespaces: &FlavourNamespaces,
    occupied: &mut BTreeMap<(String, String), AuthoringName>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(field_source) = flavour_namespaces.field(&candidate.flavour, &candidate.name) {
        diagnostics.push(
            Diagnostic::new(
                SchemaDiagnosticCode::InvalidDeclaration,
                format!(
                    "{} relation authoring name {:?} collides with a field on flavour {:?}",
                    candidate.kind, candidate.name, candidate.flavour
                ),
                Some(candidate.source.clone()),
            )
            .with_related(RelatedDiagnostic::new(
                "conflicting field is declared here",
                field_source.source.clone(),
            ))
            .with_context(DiagnosticContext::new(
                None,
                Some(candidate.relation.clone()),
                Some(candidate.flavour.clone()),
            ))
            .with_detail("collision", "field")
            .with_detail("flavour", candidate.flavour.clone())
            .with_detail("name", candidate.name.clone()),
        );
    }

    let key = (candidate.flavour.clone(), candidate.name.clone());
    if let Some(first) = occupied.get(&key) {
        diagnostics.push(
            Diagnostic::new(
                SchemaDiagnosticCode::InvalidDeclaration,
                format!(
                    "{} relation authoring name {:?} collides with another authorable relation on flavour {:?}",
                    candidate.kind, candidate.name, candidate.flavour
                ),
                Some(candidate.source),
            )
            .with_related(RelatedDiagnostic::new(
                "first authorable relation name is declared here",
                first.source.clone(),
            ))
            .with_context(DiagnosticContext::new(
                None,
                Some(candidate.relation.clone()),
                Some(candidate.flavour.clone()),
            ))
            .with_detail("collision", first.kind)
            .with_detail("first_relation", first.relation.clone())
            .with_detail("flavour", candidate.flavour.clone())
            .with_detail("name", candidate.name.clone()),
        );
    } else {
        occupied.insert(key, candidate);
    }
}

fn collect_decode<T>(result: DecodeResult<T>, diagnostics: &mut Vec<Diagnostic>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(diagnostic) => {
            diagnostics.push(*diagnostic);
            None
        }
    }
}

fn collect_compilation<T>(
    result: CompilationResult<T>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(mut found) => {
            diagnostics.append(&mut found);
            None
        }
    }
}

fn decode_flavour(
    name: &str,
    name_node: &ParsedNode,
    node: &ParsedNode,
    declared_names: &BTreeSet<String>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<FlavourDefinition> {
    let mapping = format!("flavours.{name}");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();

    let label_field = format!("{mapping}.label");
    let label = collect_decode(
        required_entry(entries, "label", &mapping, source_map, node.span()).and_then(
            |(key, value)| {
                let decoded = expect_non_empty_string(value, &label_field, source_map)?;
                Ok(field(source_map, key, value, decoded.to_owned()))
            },
        ),
        &mut diagnostics,
    );

    let description_field = format!("{mapping}.description");
    let description = collect_decode(
        required_entry(entries, "description", &mapping, source_map, node.span()).and_then(
            |(key, value)| {
                let decoded = expect_non_empty_string(value, &description_field, source_map)?;
                Ok(field(source_map, key, value, decoded.to_owned()))
            },
        ),
        &mut diagnostics,
    );

    let guidance = collect_decode(
        required_entry(entries, "guidance", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_guidance(name, value, declared_names, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let display_id = collect_decode(
        required_entry(entries, "id", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(decode_display_id(name, value, source_map), &mut diagnostics)
            .map(|decoded| field(source_map, key, value, decoded))
    });

    let title = collect_decode(
        required_entry(entries, "title", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_required_builtin(name, "title", value, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let body = collect_decode(
        required_entry(entries, "body", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_required_builtin(name, "body", value, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let fields = collect_compilation(decode_fields(name, entries, source_map), &mut diagnostics);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let (fields_source, fields) = fields.expect("valid field compilation produced a value");

    Ok(FlavourDefinition::new(
        name.to_owned(),
        parser_span(source_map, name_node.span()),
        parser_span(source_map, node.span()),
        label.expect("valid label compilation produced a value"),
        description.expect("valid description compilation produced a value"),
        guidance.expect("valid guidance compilation produced a value"),
        display_id.expect("valid display-ID compilation produced a value"),
        title.expect("valid title compilation produced a value"),
        body.expect("valid body compilation produced a value"),
        fields_source,
        fields,
    ))
}

fn decode_guidance(
    flavour: &str,
    node: &ParsedNode,
    declared_names: &BTreeSet<String>,
    source_map: &SourceMap<'_>,
) -> CompilationResult<FlavourGuidance> {
    let mapping = format!("flavours.{flavour}.guidance");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();

    let use_when_field = format!("{mapping}.use_when");
    let use_when = collect_decode(
        required_entry(entries, "use_when", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_unique_string_sequence(value, &use_when_field, true, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let avoid_when_field = format!("{mapping}.avoid_when");
    let avoid_when = collect_decode(
        required_entry(entries, "avoid_when", &mapping, source_map, node.span()),
        &mut diagnostics,
    )
    .and_then(|(key, value)| {
        collect_compilation(
            decode_unique_string_sequence(value, &avoid_when_field, true, source_map),
            &mut diagnostics,
        )
        .map(|decoded| field(source_map, key, value, decoded))
    });

    let (distinguish_from_source, distinguish_from) =
        if let Some((key, value)) = optional_entry(entries, "distinguish_from") {
            let distinction_mapping = format!("{mapping}.distinguish_from");
            let mut compiled = BTreeMap::new();
            let distinctions = collect_decode(
                expect_mapping(value, &distinction_mapping, source_map),
                &mut diagnostics,
            );
            if let Some(distinctions) = distinctions {
                for (target_node, explanation_node) in distinctions {
                    let (target, _) =
                        string_key(target_node).expect("profile validation requires string keys");
                    let mut target_valid = true;
                    if target == flavour {
                        diagnostics.push(invalid_declaration(
                            format!("flavour {flavour:?} cannot distinguish itself"),
                            source_map,
                            target_node.span(),
                            &distinction_mapping,
                        ));
                        target_valid = false;
                    } else if !declared_names.contains(target) {
                        diagnostics.push(
                            invalid_declaration(
                                format!("distinction target {target:?} is not a declared flavour"),
                                source_map,
                                target_node.span(),
                                &distinction_mapping,
                            )
                            .with_detail("target", target.to_owned()),
                        );
                        target_valid = false;
                    }
                    let explanation_field = format!("{distinction_mapping}.{target}");
                    let explanation = collect_decode(
                        expect_non_empty_string(explanation_node, &explanation_field, source_map),
                        &mut diagnostics,
                    );
                    if target_valid && let Some(explanation) = explanation {
                        compiled.insert(
                            target.to_owned(),
                            field(
                                source_map,
                                target_node,
                                explanation_node,
                                explanation.to_owned(),
                            ),
                        );
                    }
                }
            }
            (
                distinctions.map(|entries| section(source_map, key, value, entries.len())),
                compiled,
            )
        } else {
            (None, BTreeMap::new())
        };

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(FlavourGuidance::new(
        use_when.expect("valid use_when compilation produced a value"),
        avoid_when.expect("valid avoid_when compilation produced a value"),
        distinguish_from_source,
        distinguish_from,
    ))
}

fn decode_display_id(
    flavour: &str,
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> CompilationResult<DisplayIdDefinition> {
    let mapping = format!("flavours.{flavour}.id");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();
    let required = collect_decode(
        decode_optional_boolean(entries, "required", &mapping, source_map),
        &mut diagnostics,
    );
    let pattern = collect_decode(
        decode_optional_pattern(entries, "pattern", &mapping, source_map),
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(DisplayIdDefinition::new(
        required.expect("valid required compilation produced a value"),
        pattern.expect("valid pattern compilation produced a value"),
    ))
}

fn decode_required_builtin(
    flavour: &str,
    builtin: &str,
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> CompilationResult<RequiredBuiltInDefinition> {
    let mapping = format!("flavours.{flavour}.{builtin}");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let required = decode_optional_boolean(entries, "required", &mapping, source_map)
        .map_err(|diagnostic| vec![*diagnostic])?;
    Ok(RequiredBuiltInDefinition::new(required))
}

fn decode_fields(
    flavour: &str,
    flavour_entries: &[(ParsedNode, ParsedNode)],
    source_map: &SourceMap<'_>,
) -> CompilationResult<(Option<SchemaSection>, BTreeMap<String, FieldDefinition>)> {
    let Some((key, value)) = optional_entry(flavour_entries, "fields") else {
        return Ok((None, BTreeMap::new()));
    };
    let mapping = format!("flavours.{flavour}.fields");
    let entries =
        expect_mapping(value, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let source = section(source_map, key, value, entries.len());
    let mut definitions = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for (name_node, definition_node) in entries {
        let (name, _) = string_key(name_node).expect("profile validation requires string keys");
        let mut name_valid = true;
        if !valid_snake_name(name) {
            diagnostics.push(invalid_name(
                "field names must match [a-z][a-z0-9]*(?:_[a-z0-9]+)*",
                source_map,
                name_node.span(),
                &mapping,
                name,
            ));
            name_valid = false;
        }
        if is_reserved_field_name(name) {
            diagnostics.push(
                invalid_name(
                    format!("field name {name:?} is reserved"),
                    source_map,
                    name_node.span(),
                    &mapping,
                    name,
                )
                .with_detail("name", name.to_owned()),
            );
            name_valid = false;
        }
        let definition = collect_compilation(
            decode_field(flavour, name, name_node, definition_node, source_map),
            &mut diagnostics,
        );
        if name_valid && let Some(definition) = definition {
            definitions.insert(name.to_owned(), definition);
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok((Some(source), definitions))
}

fn decode_field(
    flavour: &str,
    name: &str,
    name_node: &ParsedNode,
    node: &ParsedNode,
    source_map: &SourceMap<'_>,
) -> CompilationResult<FieldDefinition> {
    let mapping = format!("flavours.{flavour}.fields.{name}");
    let entries =
        expect_mapping(node, &mapping, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    let mut diagnostics = Vec::new();

    let type_field = format!("{mapping}.type");
    let field_type = collect_decode(
        required_entry(entries, "type", &mapping, source_map, node.span()).and_then(
            |(key, value)| {
                let raw_type = expect_string(value, &type_field, source_map)?;
                let decoded = match raw_type {
                    "string" => FieldType::String,
                    "integer" => FieldType::Integer,
                    "number" => FieldType::Number,
                    "boolean" => FieldType::Boolean,
                    "enum" => FieldType::Enum,
                    _ => {
                        return Err(Box::new(
                            invalid_declaration(
                                "field type must be string, integer, number, boolean, or enum",
                                source_map,
                                value.span(),
                                &type_field,
                            )
                            .with_detail("value", raw_type.to_owned()),
                        ));
                    }
                };
                Ok(field(source_map, key, value, decoded))
            },
        ),
        &mut diagnostics,
    );
    let required = collect_decode(
        decode_optional_boolean(entries, "required", &mapping, source_map),
        &mut diagnostics,
    );
    let repeatable = collect_decode(
        decode_optional_boolean(entries, "repeatable", &mapping, source_map),
        &mut diagnostics,
    );

    let field_type_kind = field_type.as_ref().map(|field_type| *field_type.value());
    let values_entry = optional_entry(entries, "values");
    let values = match (field_type_kind, values_entry) {
        (Some(FieldType::Enum), Some((values_key, values_value))) => {
            let values_field = format!("{mapping}.values");
            collect_compilation(
                decode_unique_string_sequence(values_value, &values_field, false, source_map),
                &mut diagnostics,
            )
            .map(|decoded| field(source_map, values_key, values_value, decoded))
        }
        (Some(FieldType::Enum), None) => {
            diagnostics.push(
                invalid_declaration(
                    "enum fields require a non-empty values sequence",
                    source_map,
                    node.span(),
                    &mapping,
                )
                .with_detail("key", "values"),
            );
            None
        }
        (Some(_), Some((values_key, _))) => {
            diagnostics.push(invalid_declaration(
                "values is permitted only for enum fields",
                source_map,
                values_key.span(),
                &mapping,
            ));
            None
        }
        (Some(_), None) => None,
        (None, Some((_, values_value))) => {
            let values_field = format!("{mapping}.values");
            let _ = collect_compilation(
                decode_unique_string_sequence(values_value, &values_field, false, source_map),
                &mut diagnostics,
            );
            None
        }
        (None, None) => None,
    };

    let pattern_entry = optional_entry(entries, "pattern");
    let pattern = match (field_type_kind, pattern_entry) {
        (Some(FieldType::String), Some(_)) => collect_decode(
            decode_optional_pattern(entries, "pattern", &mapping, source_map),
            &mut diagnostics,
        )
        .flatten(),
        (Some(FieldType::String), None) => None,
        (Some(_), Some((pattern_key, _))) => {
            diagnostics.push(invalid_declaration(
                "pattern is permitted only for string fields",
                source_map,
                pattern_key.span(),
                &mapping,
            ));
            None
        }
        (Some(_), None) => None,
        (None, Some(_)) => {
            let _ = collect_decode(
                decode_optional_pattern(entries, "pattern", &mapping, source_map),
                &mut diagnostics,
            );
            None
        }
        (None, None) => None,
    };

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(FieldDefinition::new(
        name.to_owned(),
        parser_span(source_map, name_node.span()),
        parser_span(source_map, node.span()),
        field_type.expect("valid type compilation produced a value"),
        required.expect("valid required compilation produced a value"),
        repeatable.expect("valid repeatable compilation produced a value"),
        values,
        pattern,
    ))
}

fn decode_optional_boolean(
    entries: &[(ParsedNode, ParsedNode)],
    name: &str,
    mapping: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<Option<SchemaField<bool>>> {
    let Some((key, value)) = optional_entry(entries, name) else {
        return Ok(None);
    };
    let field_name = format!("{mapping}.{name}");
    let decoded = expect_boolean(value, &field_name, source_map)?;
    Ok(Some(field(source_map, key, value, decoded)))
}

fn decode_optional_pattern(
    entries: &[(ParsedNode, ParsedNode)],
    name: &str,
    mapping: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<Option<SchemaField<String>>> {
    let Some((key, value)) = optional_entry(entries, name) else {
        return Ok(None);
    };
    let field_name = format!("{mapping}.{name}");
    let pattern = expect_string(value, &field_name, source_map)?;
    // Compile the authored expression directly. Later value validation enforces
    // whole-string matching from match bounds; textual wrapping would corrupt a
    // valid verbose-mode pattern that ends in a comment.
    if UnicodeRegex::new(pattern).is_err() {
        return Err(Box::new(
            Diagnostic::new(
                SchemaDiagnosticCode::InvalidPattern,
                format!("{field_name} is not a valid Rust regex pattern"),
                Some(parser_span(source_map, value.span())),
            )
            .with_context(DiagnosticContext::new(Some(field_name), None, None))
            .with_detail("pattern", pattern.to_owned()),
        ));
    }
    Ok(Some(field(source_map, key, value, pattern.to_owned())))
}

fn decode_unique_rule_condition_sequence(
    node: &ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<RuleConditionValue>>> {
    let values =
        expect_sequence(node, field_name, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    if values.is_empty() {
        return Err(vec![invalid_declaration(
            format!("{field_name} must be a non-empty sequence"),
            source_map,
            node.span(),
            field_name,
        )]);
    }

    let mut compiled = Vec::with_capacity(values.len());
    let mut diagnostics = Vec::new();
    for value_node in values {
        let Some(value) = collect_decode(
            decode_rule_condition_value(value_node, field_name, source_map),
            &mut diagnostics,
        ) else {
            continue;
        };
        if compiled
            .iter()
            .any(|existing: &SchemaValue<RuleConditionValue>| existing.value() == &value)
        {
            diagnostics.push(
                invalid_declaration(
                    format!("{field_name} entries must be unique"),
                    source_map,
                    value_node.span(),
                    field_name,
                )
                .with_detail("value", rule_condition_value_detail(&value)),
            );
            continue;
        }
        compiled.push(SchemaValue::new(
            parser_span(source_map, value_node.span()),
            value,
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(compiled)
}

fn decode_rule_condition_value(
    node: &ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<RuleConditionValue> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} entries must be scalar values"),
            source_map,
            node.span(),
            field_name,
        )));
    };
    let decoded = match resolve_scalar(value, *style, tag.as_ref()).ok() {
        Some(ScalarKind::String) => Some(RuleConditionValue::String(value.clone())),
        Some(ScalarKind::Boolean) => parsed_boolean(node).map(RuleConditionValue::Boolean),
        Some(ScalarKind::Integer) if valid_rule_integer_syntax(value) => value
            .parse::<i64>()
            .map(RuleConditionValue::Integer)
            .ok()
            .or_else(|| parse_rule_condition_number(value).map(RuleConditionValue::Number)),
        Some(ScalarKind::Float) => {
            parse_rule_condition_number(value).map(RuleConditionValue::Number)
        }
        _ => None,
    };
    decoded.ok_or_else(|| {
        Box::new(
            invalid_declaration(
                format!(
                    "{field_name} entries must be strings, booleans, signed 64-bit integers, or finite JSON numbers"
                ),
                source_map,
                node.span(),
                field_name,
            )
            .with_detail("value", value.clone()),
        )
    })
}

fn parse_rule_condition_number(value: &str) -> Option<RuleConditionNumber> {
    serde_json::from_str::<serde_json::Number>(value)
        .ok()
        .and_then(|number| number.as_f64())
        .and_then(RuleConditionNumber::new)
}

fn decode_unique_string_sequence(
    node: &ParsedNode,
    field_name: &str,
    reject_empty_values: bool,
    source_map: &SourceMap<'_>,
) -> CompilationResult<Vec<SchemaValue<String>>> {
    let values =
        expect_sequence(node, field_name, source_map).map_err(|diagnostic| vec![*diagnostic])?;
    if values.is_empty() {
        return Err(vec![invalid_declaration(
            format!("{field_name} must be a non-empty sequence"),
            source_map,
            node.span(),
            field_name,
        )]);
    }

    let mut seen = BTreeSet::new();
    let mut compiled = Vec::with_capacity(values.len());
    let mut diagnostics = Vec::new();
    for value_node in values {
        let Some(value) = collect_decode(
            expect_string(value_node, field_name, source_map),
            &mut diagnostics,
        ) else {
            continue;
        };
        let mut value_valid = true;
        if reject_empty_values && value.is_empty() {
            diagnostics.push(invalid_declaration(
                format!("{field_name} entries must be non-empty strings"),
                source_map,
                value_node.span(),
                field_name,
            ));
            value_valid = false;
        }
        if !seen.insert(value.to_owned()) {
            diagnostics.push(
                invalid_declaration(
                    format!("{field_name} entries must be unique"),
                    source_map,
                    value_node.span(),
                    field_name,
                )
                .with_detail("value", value.to_owned()),
            );
            value_valid = false;
        }
        if value_valid {
            compiled.push(SchemaValue::new(
                parser_span(source_map, value_node.span()),
                value.to_owned(),
            ));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(compiled)
}

fn expect_mapping<'a>(
    node: &'a ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<&'a [(ParsedNode, ParsedNode)]> {
    let ParsedNode::Mapping { entries, .. } = node else {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} must be a mapping"),
            source_map,
            node.span(),
            field_name,
        )));
    };
    Ok(entries)
}

fn expect_sequence<'a>(
    node: &'a ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<&'a [ParsedNode]> {
    let ParsedNode::Sequence { values, .. } = node else {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} must be a sequence"),
            source_map,
            node.span(),
            field_name,
        )));
    };
    Ok(values)
}

fn expect_non_empty_string<'a>(
    node: &'a ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<&'a str> {
    let value = expect_string(node, field_name, source_map)?;
    if value.is_empty() {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} must be a non-empty string"),
            source_map,
            node.span(),
            field_name,
        )));
    }
    Ok(value)
}

fn expect_boolean(
    node: &ParsedNode,
    field_name: &str,
    source_map: &SourceMap<'_>,
) -> DecodeResult<bool> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} must be a boolean"),
            source_map,
            node.span(),
            field_name,
        )));
    };
    if resolve_scalar(value, *style, tag.as_ref()).ok() != Some(ScalarKind::Boolean) {
        return Err(Box::new(invalid_declaration(
            format!("{field_name} must be a boolean"),
            source_map,
            node.span(),
            field_name,
        )));
    }
    Ok(matches!(value.as_str(), "true" | "True" | "TRUE"))
}

fn is_reserved_field_name(name: &str) -> bool {
    matches!(
        name,
        "mid" | "flavour" | "id" | "title" | "body" | "source_location" | "mentions"
    )
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
    if let Some((_, ParsedNode::Mapping { entries, .. })) = optional_entry(entries, "flavours") {
        collect_flavour_unknown_key_diagnostics(entries, source_map, &mut diagnostics);
    }
    if let Some((_, ParsedNode::Mapping { entries, .. })) = optional_entry(entries, "relations") {
        collect_relation_unknown_key_diagnostics(entries, source_map, &mut diagnostics);
    }
    if let Some((_, ParsedNode::Sequence { values, .. })) = optional_entry(entries, "rules") {
        collect_rule_unknown_key_diagnostics(values, source_map, &mut diagnostics);
    }
    diagnostics
}

fn collect_flavour_unknown_key_diagnostics(
    flavours: &[(ParsedNode, ParsedNode)],
    source_map: &SourceMap<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (flavour_key, flavour_value) in flavours {
        let (flavour, _) =
            string_key(flavour_key).expect("profile validation requires string keys");
        let ParsedNode::Mapping {
            entries: flavour_entries,
            ..
        } = flavour_value
        else {
            continue;
        };
        let flavour_mapping = format!("flavours.{flavour}");
        collect_unknown_key_diagnostics(
            flavour_entries,
            &[
                "label",
                "description",
                "guidance",
                "id",
                "title",
                "body",
                "fields",
            ],
            &flavour_mapping,
            source_map,
            diagnostics,
        );

        if let Some((_, ParsedNode::Mapping { entries, .. })) =
            optional_entry(flavour_entries, "guidance")
        {
            let mapping = format!("{flavour_mapping}.guidance");
            collect_unknown_key_diagnostics(
                entries,
                &["use_when", "avoid_when", "distinguish_from"],
                &mapping,
                source_map,
                diagnostics,
            );
        }
        if let Some((_, ParsedNode::Mapping { entries, .. })) =
            optional_entry(flavour_entries, "id")
        {
            let mapping = format!("{flavour_mapping}.id");
            collect_unknown_key_diagnostics(
                entries,
                &["required", "pattern"],
                &mapping,
                source_map,
                diagnostics,
            );
        }
        for builtin in ["title", "body"] {
            if let Some((_, ParsedNode::Mapping { entries, .. })) =
                optional_entry(flavour_entries, builtin)
            {
                let mapping = format!("{flavour_mapping}.{builtin}");
                collect_unknown_key_diagnostics(
                    entries,
                    &["required"],
                    &mapping,
                    source_map,
                    diagnostics,
                );
            }
        }
        if let Some((
            _,
            ParsedNode::Mapping {
                entries: fields, ..
            },
        )) = optional_entry(flavour_entries, "fields")
        {
            for (field_key, field_value) in fields {
                let (field_name, _) =
                    string_key(field_key).expect("profile validation requires string keys");
                if let ParsedNode::Mapping { entries, .. } = field_value {
                    let mapping = format!("{flavour_mapping}.fields.{field_name}");
                    collect_unknown_key_diagnostics(
                        entries,
                        &["type", "required", "repeatable", "values", "pattern"],
                        &mapping,
                        source_map,
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn collect_relation_unknown_key_diagnostics(
    relations: &[(ParsedNode, ParsedNode)],
    source_map: &SourceMap<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (relation_key, relation_value) in relations {
        let (relation, _) =
            string_key(relation_key).expect("profile validation requires string keys");
        let ParsedNode::Mapping {
            entries: relation_entries,
            ..
        } = relation_value
        else {
            continue;
        };
        let relation_mapping = format!("relations.{relation}");
        collect_unknown_key_diagnostics(
            relation_entries,
            &[
                "source",
                "target",
                "inverse",
                "inverse_authoring",
                "symmetric",
                "same_flavour",
                "self_reference",
                "acyclic",
                "cardinality",
            ],
            &relation_mapping,
            source_map,
            diagnostics,
        );

        for (endpoint, allowed) in [
            ("source", &["flavours", "derived"][..]),
            ("target", &["flavours", "external"][..]),
        ] {
            if let Some((_, ParsedNode::Mapping { entries, .. })) =
                optional_entry(relation_entries, endpoint)
            {
                collect_unknown_key_diagnostics(
                    entries,
                    allowed,
                    &format!("{relation_mapping}.{endpoint}"),
                    source_map,
                    diagnostics,
                );
            }
        }

        if let Some((_, ParsedNode::Mapping { entries, .. })) =
            optional_entry(relation_entries, "cardinality")
        {
            let cardinality_mapping = format!("{relation_mapping}.cardinality");
            collect_unknown_key_diagnostics(
                entries,
                &["outgoing", "incoming"],
                &cardinality_mapping,
                source_map,
                diagnostics,
            );
            for direction in ["outgoing", "incoming"] {
                if let Some((_, ParsedNode::Mapping { entries, .. })) =
                    optional_entry(entries, direction)
                {
                    collect_unknown_key_diagnostics(
                        entries,
                        &["min", "max"],
                        &format!("{cardinality_mapping}.{direction}"),
                        source_map,
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn collect_rule_unknown_key_diagnostics(
    rules: &[ParsedNode],
    source_map: &SourceMap<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (index, rule) in rules.iter().enumerate() {
        let ParsedNode::Mapping { entries, .. } = rule else {
            continue;
        };
        let mapping = format!("rules[{index}]");
        let mut allowed = vec!["name", "kind", "severity", "applies_to", "when"];
        match optional_entry(entries, "kind").and_then(|(_, value)| parsed_string(value)) {
            Some("requires_relation") => {
                allowed.extend(["relation", "relation_any_of", "direction", "min", "max"]);
            }
            Some("requires_field") => {
                allowed.extend(["field", "field_any_of", "min", "max"]);
            }
            Some("orphan") => allowed.push("relations"),
            _ => {}
        }
        collect_unknown_key_diagnostics(entries, &allowed, &mapping, source_map, diagnostics);

        if let Some((_, ParsedNode::Mapping { entries, .. })) =
            optional_entry(entries, "applies_to")
        {
            collect_unknown_key_diagnostics(
                entries,
                &["flavours"],
                &format!("{mapping}.applies_to"),
                source_map,
                diagnostics,
            );
        }
        if let Some((_, ParsedNode::Mapping { entries, .. })) = optional_entry(entries, "when") {
            collect_unknown_key_diagnostics(
                entries,
                &["field", "in"],
                &format!("{mapping}.when"),
                source_map,
                diagnostics,
            );
        }
    }
}

fn collect_unknown_key_diagnostics(
    entries: &[(ParsedNode, ParsedNode)],
    allowed: &[&str],
    mapping: &str,
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
    name: &str,
    mapping: &str,
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
    field: &str,
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
    field: &str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::InvalidDeclaration,
        message,
        Some(parser_span(source_map, span)),
    )
    .with_context(DiagnosticContext::new(Some(field.to_owned()), None, None))
}

fn invalid_declaration_at_source(
    message: impl Into<String>,
    source: SourceSpan,
    field: &str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::InvalidDeclaration,
        message,
        Some(source),
    )
    .with_context(DiagnosticContext::new(Some(field.to_owned()), None, None))
}

fn invalid_name(
    message: impl Into<String>,
    source_map: &SourceMap<'_>,
    span: ParsedSpan,
    field: &str,
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

fn invalid_name_at_source(
    message: impl Into<String>,
    source: SourceSpan,
    field: &str,
    value: &str,
) -> Diagnostic {
    Diagnostic::new(SchemaDiagnosticCode::InvalidName, message, Some(source))
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

fn valid_snake_name(value: &str) -> bool {
    let mut segments = value.split('_');
    let Some(first) = segments.next() else {
        return false;
    };
    valid_lower_alphanumeric_segment(first, true)
        && segments.all(|segment| valid_lower_alphanumeric_segment(segment, false))
}

fn valid_external_scheme(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'+' | b'.' | b'-')
        })
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
    static SEMVER: OnceLock<LiteRegex> = OnceLock::new();
    SEMVER
        .get_or_init(|| {
            LiteRegex::new(concat!(
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
