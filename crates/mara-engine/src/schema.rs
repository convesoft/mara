//! Strict loading of the configured v1 Mara schema document and identity.

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use mara_core::{
    Diagnostic, DiagnosticContext, DiagnosticSeverity, IdentityConfiguration, MidFormat,
    MidIdentity, RelatedDiagnostic, SchemaDiagnosticCode, SchemaDocument, SchemaField,
    SchemaIdentity, SchemaSection, SourceSpan,
};
use saphyr_parser::{Event, Marker, Parser, ScalarStyle, Span, SpannedEventReceiver, Tag};
use semver::Version;

use crate::project::{LoadedProject, open_loaded_schema};

type DecodeResult<T> = Result<T, Box<Diagnostic>>;

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
    let relative = project
        .schema_path
        .strip_prefix(&project.root)
        .map_err(|_| io::Error::other("loaded schema path is outside the project root"))?;
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| io::Error::other("loaded schema path is not UTF-8"))?;
                if value.contains('\\') {
                    return Err(io::Error::other(
                        "loaded schema path cannot be represented by a Mara source span",
                    ));
                }
                components.push(value);
            }
            _ => {
                return Err(io::Error::other(
                    "loaded schema path is not normalized and project-relative",
                ));
            }
        }
    }
    if components.is_empty() {
        return Err(io::Error::other("loaded schema path has no file name"));
    }
    Ok(components.join("/"))
}

fn decode_schema(bytes: &[u8], path: &str) -> Result<SchemaDocument, SchemaLoadError> {
    let source = std::str::from_utf8(bytes).map_err(|error| {
        let offset = error.valid_up_to();
        let (line, column) = position_at_valid_prefix(&bytes[..offset]);
        let primary = source_span(path, offset, offset, line, column, line, column);
        SchemaLoadError::invalid(vec![
            Diagnostic::new(
                SchemaDiagnosticCode::Syntax,
                "schema source is not valid UTF-8",
                Some(primary),
            )
            .with_detail("feature", "invalid_utf8"),
        ])
    })?;

    if let Some(diagnostic) = validate_document_directives(source, path) {
        return Err(SchemaLoadError::invalid(vec![diagnostic]));
    }

    let source_map = SourceMap::new(source);
    let mut receiver = TreeBuilder::new(&source_map);
    if let Err(error) = Parser::new_from_str(source).load(&mut receiver, true) {
        let primary = marker_span(path, source_map.position(*error.marker()));
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
        return Err(SchemaLoadError::invalid(vec![Diagnostic::new(
            SchemaDiagnosticCode::Syntax,
            message,
            Some(parser_span(path, span)),
        )]));
    }
    if receiver.documents.len() != 1 {
        let (primary, feature) = if receiver.documents.is_empty() {
            let (line, column) = position_at_valid_prefix(source.as_bytes());
            (
                Some(source_span(
                    path,
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
                    .map(|span| parser_span(path, span)),
                "multiple_documents",
            )
        };
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
    validate_profile(&root, path, &mut profile);
    if !profile.is_empty() {
        return Err(SchemaLoadError::invalid(profile));
    }

    decode_v1_document(source, path, root)
        .map_err(|diagnostic| SchemaLoadError::invalid(vec![*diagnostic]))
}

fn validate_document_directives(source: &str, path: &str) -> Option<Diagnostic> {
    let mut byte_offset = 0;
    let mut line = 1;
    for source_line in source.split_inclusive('\n') {
        let line_without_lf = source_line.strip_suffix('\n').unwrap_or(source_line);
        let content = line_without_lf
            .strip_suffix('\r')
            .unwrap_or(line_without_lf);
        let content = if byte_offset == 0 {
            content.strip_prefix('\u{feff}').unwrap_or(content)
        } else {
            content
        };
        if content.trim().is_empty() || content.trim_start().starts_with('#') {
            byte_offset += source_line.len();
            line += 1;
            continue;
        }
        if !content.starts_with('%') {
            break;
        }

        let feature = if let Some(rest) = content.strip_prefix("%YAML") {
            let version = rest.split_whitespace().next().unwrap_or("");
            if version == "1.2" {
                byte_offset += source_line.len();
                line += 1;
                continue;
            }
            "unsupported_yaml_version"
        } else if content.starts_with("%TAG") {
            "custom_tag"
        } else {
            "unsupported_directive"
        };
        let bom_bytes = line_without_lf.len() - content.len();
        let start_byte = byte_offset + bom_bytes;
        let start_column = line_without_lf[..bom_bytes].chars().count() + 1;
        let primary = source_span(
            path,
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
    char_to_byte: Vec<usize>,
}

impl<'source> SourceMap<'source> {
    fn new(source: &'source str) -> Self {
        let mut char_to_byte = source
            .char_indices()
            .map(|(byte, _)| byte)
            .collect::<Vec<_>>();
        char_to_byte.push(source.len());
        Self {
            source,
            char_to_byte,
        }
    }

    fn position(&self, marker: Marker) -> ParsedPosition {
        let byte = *self
            .char_to_byte
            .get(marker.index())
            .expect("YAML parser marker is inside its UTF-8 input");
        ParsedPosition {
            byte,
            line: marker.line(),
            column: marker.col() + 1,
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
                parsed.start.byte,
                if style == ScalarStyle::Literal {
                    '|'
                } else {
                    '>'
                },
            )
        {
            let (line, column) = position_at_valid_prefix(&self.source.as_bytes()[..start_byte]);
            parsed.start = ParsedPosition {
                byte: start_byte,
                line,
                column,
            };
        }
        self.expand_tag(&mut parsed, tag);
        parsed
    }

    fn expand_tag(&self, parsed: &mut ParsedSpan, tag: Option<&Tag>) {
        if tag.is_none() {
            return;
        }
        if let Some(start_byte) = preceding_tag_start(self.source, parsed.start.byte) {
            let (line, column) = position_at_valid_prefix(&self.source.as_bytes()[..start_byte]);
            parsed.start = ParsedPosition {
                byte: start_byte,
                line,
                column,
            };
        }
    }
}

fn preceding_block_scalar_start(
    source: &str,
    content_start: usize,
    indicator: char,
) -> Option<usize> {
    let mut search_end = content_start;
    while let Some(start) = source[..search_end].rfind(indicator) {
        if block_scalar_prefix_only(&source[start + indicator.len_utf8()..content_start]) {
            return Some(start);
        }
        search_end = start;
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

fn preceding_tag_start(source: &str, node_start: usize) -> Option<usize> {
    let mut search_end = node_start;
    while let Some(start) = source[..search_end].rfind('!') {
        if tag_start_boundary(source, start)
            && let Some(end) = raw_tag_end(source, start, node_start)
            && yaml_separation_only(&source[end..node_start])
        {
            return Some(start);
        }
        search_end = start;
    }
    None
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

fn yaml_separation_only(source: &str) -> bool {
    let mut comment = false;
    for character in source.chars() {
        if comment {
            if matches!(character, '\r' | '\n') {
                comment = false;
            }
        } else if character == '#' {
            comment = true;
        } else if !character.is_whitespace() {
            return false;
        }
    }
    true
}

#[derive(Debug)]
enum ParsedNode {
    Scalar {
        value: String,
        style: ScalarStyle,
        anchor: usize,
        tag: Option<Tag>,
        span: ParsedSpan,
    },
    Sequence {
        values: Vec<Self>,
        anchor: usize,
        tag: Option<Tag>,
        span: ParsedSpan,
    },
    Mapping {
        entries: Vec<(Self, Self)>,
        anchor: usize,
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

#[derive(Debug)]
enum PendingContainer {
    Sequence {
        values: Vec<ParsedNode>,
        anchor: usize,
        tag: Option<Tag>,
        start: ParsedSpan,
    },
    Mapping {
        values: Vec<ParsedNode>,
        anchor: usize,
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
}

impl<'input> SpannedEventReceiver<'input> for TreeBuilder<'_, 'input> {
    fn on_event(&mut self, event: Event<'input>, span: Span) {
        let parser_span = self.source_map.span(span);
        match event {
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd | Event::Nothing => {}
            Event::DocumentStart(_) => self.document_starts.push(parser_span),
            Event::Alias(_) => self.attach(ParsedNode::Alias { span: parser_span }),
            Event::Scalar(value, style, anchor, tag) => {
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
                let span = self.source_map.node_span(span, tag.as_deref());
                self.stack.push(PendingContainer::Sequence {
                    values: Vec::new(),
                    anchor,
                    tag: tag.map(|tag| tag.into_owned()),
                    start: span,
                });
            }
            Event::MappingStart(anchor, tag) => {
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

fn validate_profile(node: &ParsedNode, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    match node {
        ParsedNode::Alias { span } => diagnostics.push(profile_diagnostic(
            "YAML aliases are not permitted in schema documents",
            path,
            *span,
            "alias",
        )),
        ParsedNode::Scalar {
            value,
            style,
            anchor,
            tag,
            span,
        } => {
            validate_anchor(*anchor, path, *span, diagnostics);
            match resolve_scalar(value, *style, tag.as_ref()) {
                Ok(ScalarKind::Null) => diagnostics.push(profile_diagnostic(
                    "null values are not permitted in schema documents",
                    path,
                    *span,
                    "null",
                )),
                Ok(_) => {}
                Err(message) => {
                    diagnostics.push(profile_diagnostic(message, path, *span, "custom_tag"))
                }
            }
        }
        ParsedNode::Sequence {
            values,
            anchor,
            tag,
            span,
        } => {
            validate_anchor(*anchor, path, *span, diagnostics);
            validate_collection_tag(tag.as_ref(), "seq", path, *span, diagnostics);
            for value in values {
                validate_profile(value, path, diagnostics);
            }
        }
        ParsedNode::Mapping {
            entries,
            anchor,
            tag,
            span,
        } => {
            validate_anchor(*anchor, path, *span, diagnostics);
            validate_collection_tag(tag.as_ref(), "map", path, *span, diagnostics);
            let mut seen = HashMap::<String, SourceSpan>::new();
            for (key, value) in entries {
                let before = diagnostics.len();
                validate_profile(key, path, diagnostics);
                if diagnostics.len() == before {
                    match string_key(key) {
                        Ok((_name, is_merge)) if is_merge => diagnostics.push(profile_diagnostic(
                            "YAML merge keys are not permitted in schema documents",
                            path,
                            key.span(),
                            "merge_key",
                        )),
                        Ok((name, _)) => {
                            let key_source = parser_span(path, key.span());
                            if let Some(first) = seen.get(name) {
                                diagnostics.push(
                                    Diagnostic::new(
                                        SchemaDiagnosticCode::DuplicateKey,
                                        format!("mapping key {name:?} is declared more than once"),
                                        Some(key_source),
                                    )
                                    .with_related(RelatedDiagnostic::new(
                                        "first declaration of this key",
                                        first.clone(),
                                    ))
                                    .with_detail("key", name.to_owned()),
                                );
                            } else {
                                seen.insert(name.to_owned(), key_source);
                            }
                        }
                        Err(()) => diagnostics.push(profile_diagnostic(
                            "every YAML mapping key must resolve to a string",
                            path,
                            key.span(),
                            "non_string_key",
                        )),
                    }
                }
                validate_profile(value, path, diagnostics);
            }
        }
    }
}

fn validate_anchor(anchor: usize, path: &str, span: ParsedSpan, diagnostics: &mut Vec<Diagnostic>) {
    if anchor != 0 {
        diagnostics.push(profile_diagnostic(
            "YAML anchors are not permitted in schema documents",
            path,
            span,
            "anchor",
        ));
    }
}

fn validate_collection_tag(
    tag: Option<&Tag>,
    expected: &'static str,
    path: &str,
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
        path,
        span,
        "custom_tag",
    ));
}

fn profile_diagnostic(
    message: impl Into<String>,
    path: &str,
    span: ParsedSpan,
    feature: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::Syntax,
        message,
        Some(parser_span(path, span)),
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

fn decode_v1_document(source: &str, path: &str, root: ParsedNode) -> DecodeResult<SchemaDocument> {
    let ParsedNode::Mapping {
        entries,
        span: root_span,
        ..
    } = root
    else {
        return Err(Box::new(invalid_declaration(
            "schema document root must be a mapping",
            path,
            root.span(),
            "root",
        )));
    };

    let (format_key, format_value) =
        required_entry(&entries, "format_version", "root", path, root_span)?;
    let format_version = decode_format_version(format_key, format_value, path)?;

    reject_unknown_keys(
        &entries,
        &[
            "format_version",
            "schema",
            "identity",
            "flavours",
            "relations",
            "rules",
        ],
        "root",
        path,
    )?;

    let (schema_key, schema_value) = required_entry(&entries, "schema", "root", path, root_span)?;
    let schema_identity = decode_schema_identity(schema_value, path)?;
    let schema = field(path, schema_key, schema_value, schema_identity);

    let (identity_key, identity_value) =
        required_entry(&entries, "identity", "root", path, root_span)?;
    let identity_configuration = decode_identity(identity_value, path)?;
    let identity = field(path, identity_key, identity_value, identity_configuration);

    let (flavours_key, flavours_value) =
        required_entry(&entries, "flavours", "root", path, root_span)?;
    let flavours_len = mapping_len(flavours_value).ok_or_else(|| {
        invalid_declaration(
            "root.flavours must be a mapping",
            path,
            flavours_value.span(),
            "flavours",
        )
    })?;
    let flavours = section(path, flavours_key, flavours_value, flavours_len);

    let relations = if let Some((key, value)) = optional_entry(&entries, "relations") {
        let Some(len) = mapping_len(value) else {
            return Err(Box::new(invalid_declaration(
                "root.relations must be a mapping",
                path,
                value.span(),
                "relations",
            )));
        };
        Some(section(path, key, value, len))
    } else {
        None
    };

    let rules = if let Some((key, value)) = optional_entry(&entries, "rules") {
        let Some(len) = sequence_len(value) else {
            return Err(Box::new(invalid_declaration(
                "root.rules must be a sequence",
                path,
                value.span(),
                "rules",
            )));
        };
        Some(section(path, key, value, len))
    } else {
        None
    };

    let (end_line, end_column) = position_at_valid_prefix(source.as_bytes());
    let document_source = source_span(path, 0, source.len(), 1, 1, end_line, end_column);
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
    path: &str,
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
            path,
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
            path,
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
                Some(parser_span(path, *span)),
            )
            .with_context(DiagnosticContext::new(
                Some("format_version".to_owned()),
                None,
                None,
            ))
            .with_detail("format_version", raw.to_owned()),
        ));
    }
    Ok(field(path, key, value, 1))
}

fn decode_schema_identity(node: &ParsedNode, path: &str) -> DecodeResult<SchemaIdentity> {
    let ParsedNode::Mapping { entries, span, .. } = node else {
        return Err(Box::new(invalid_declaration(
            "root.schema must be a mapping",
            path,
            node.span(),
            "schema",
        )));
    };
    reject_unknown_keys(entries, &["name", "version"], "schema", path)?;
    let (name_key, name_value) = required_entry(entries, "name", "schema", path, *span)?;
    let name = expect_string(name_value, "schema.name", path)?;
    if !valid_kebab_name(name) {
        return Err(Box::new(invalid_name(
            "schema.name must match [a-z][a-z0-9]*(?:-[a-z0-9]+)*",
            path,
            name_value.span(),
            "schema.name",
            name,
        )));
    }
    let name = field(path, name_key, name_value, name.to_owned());

    let (version_key, version_value) = required_entry(entries, "version", "schema", path, *span)?;
    let version = expect_string(version_value, "schema.version", path)?;
    Version::parse(version).map_err(|_| {
        Box::new(
            invalid_declaration(
                "schema.version must use SemVer 2.0.0 syntax",
                path,
                version_value.span(),
                "schema.version",
            )
            .with_detail("value", version.to_owned()),
        )
    })?;
    let version = field(path, version_key, version_value, version.to_owned());
    Ok(SchemaIdentity::new(name, version))
}

fn decode_identity(node: &ParsedNode, path: &str) -> DecodeResult<IdentityConfiguration> {
    let ParsedNode::Mapping { entries, span, .. } = node else {
        return Err(Box::new(invalid_declaration(
            "root.identity must be a mapping",
            path,
            node.span(),
            "identity",
        )));
    };
    reject_unknown_keys(entries, &["mid"], "identity", path)?;
    let (mid_key, mid_value) = required_entry(entries, "mid", "identity", path, *span)?;
    let ParsedNode::Mapping {
        entries: mid_entries,
        span: mid_span,
        ..
    } = mid_value
    else {
        return Err(Box::new(invalid_declaration(
            "identity.mid must be a mapping",
            path,
            mid_value.span(),
            "identity.mid",
        )));
    };
    reject_unknown_keys(mid_entries, &["format", "prefix"], "identity.mid", path)?;

    let (format_key, format_value) =
        required_entry(mid_entries, "format", "identity.mid", path, *mid_span)?;
    let format = expect_string(format_value, "identity.mid.format", path)?;
    if format != "ulid" {
        return Err(Box::new(
            invalid_declaration(
                "identity.mid.format must be \"ulid\" in format version 1",
                path,
                format_value.span(),
                "identity.mid.format",
            )
            .with_detail("value", format.to_owned()),
        ));
    }
    let format = field(path, format_key, format_value, MidFormat::Ulid);

    let (prefix_key, prefix_value) =
        required_entry(mid_entries, "prefix", "identity.mid", path, *mid_span)?;
    let prefix = expect_string(prefix_value, "identity.mid.prefix", path)?;
    if !valid_mid_prefix(prefix) {
        return Err(Box::new(invalid_name(
            "identity.mid.prefix must match [a-z][a-z0-9]*_",
            path,
            prefix_value.span(),
            "identity.mid.prefix",
            prefix,
        )));
    }
    let prefix = field(path, prefix_key, prefix_value, prefix.to_owned());
    let mid = field(path, mid_key, mid_value, MidIdentity::new(format, prefix));
    Ok(IdentityConfiguration::new(mid))
}

fn reject_unknown_keys(
    entries: &[(ParsedNode, ParsedNode)],
    allowed: &[&str],
    mapping: &'static str,
    path: &str,
) -> DecodeResult<()> {
    for (key, _) in entries {
        let (name, _) = string_key(key).expect("profile validation requires string keys");
        if !allowed.contains(&name) {
            return Err(Box::new(
                Diagnostic::new(
                    SchemaDiagnosticCode::UnknownKey,
                    format!("{mapping} contains unknown key {name:?}"),
                    Some(parser_span(path, key.span())),
                )
                .with_context(DiagnosticContext::new(Some(name.to_owned()), None, None))
                .with_detail("key", name.to_owned())
                .with_detail("mapping", mapping),
            ));
        }
    }
    Ok(())
}

fn required_entry<'a>(
    entries: &'a [(ParsedNode, ParsedNode)],
    name: &'static str,
    mapping: &'static str,
    path: &str,
    mapping_span: ParsedSpan,
) -> DecodeResult<(&'a ParsedNode, &'a ParsedNode)> {
    optional_entry(entries, name).ok_or_else(|| {
        Box::new(
            invalid_declaration(
                format!("{mapping} is missing required key {name:?}"),
                path,
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
    path: &str,
) -> DecodeResult<&'a str> {
    let ParsedNode::Scalar {
        value, style, tag, ..
    } = node
    else {
        return Err(Box::new(invalid_declaration(
            format!("{field} must be a string"),
            path,
            node.span(),
            field,
        )));
    };
    if resolve_scalar(value, *style, tag.as_ref()).ok() != Some(ScalarKind::String) {
        return Err(Box::new(invalid_declaration(
            format!("{field} must be a string"),
            path,
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

fn field<T>(path: &str, key: &ParsedNode, value: &ParsedNode, decoded: T) -> SchemaField<T> {
    SchemaField::new(
        parser_span(path, key.span()),
        parser_span(path, value.span()),
        decoded,
    )
}

fn section(path: &str, key: &ParsedNode, value: &ParsedNode, len: usize) -> SchemaSection {
    SchemaSection::new(
        parser_span(path, key.span()),
        parser_span(path, value.span()),
        len,
    )
}

fn invalid_declaration(
    message: impl Into<String>,
    path: &str,
    span: ParsedSpan,
    field: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::InvalidDeclaration,
        message,
        Some(parser_span(path, span)),
    )
    .with_context(DiagnosticContext::new(Some(field.to_owned()), None, None))
}

fn invalid_name(
    message: impl Into<String>,
    path: &str,
    span: ParsedSpan,
    field: &'static str,
    value: &str,
) -> Diagnostic {
    Diagnostic::new(
        SchemaDiagnosticCode::InvalidName,
        message,
        Some(parser_span(path, span)),
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

fn parser_span(path: &str, span: ParsedSpan) -> SourceSpan {
    source_span(
        path,
        span.start.byte,
        span.end.byte,
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}

fn marker_span(path: &str, marker: ParsedPosition) -> SourceSpan {
    source_span(
        path,
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
    path: &str,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
) -> SourceSpan {
    SourceSpan::try_new(
        path,
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
    });
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
    }

    #[test]
    fn source_position_counts_unicode_scalars_and_crlf_once() {
        assert_eq!(position_at_valid_prefix("aé\r\nb".as_bytes()), (2, 2));
    }
}
