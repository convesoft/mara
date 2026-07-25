use std::{error::Error, fmt};

/// A reusable index of every legal source-span boundary in one UTF-8 source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIndex {
    path: String,
    source_len: usize,
    positions: Vec<IndexedPosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexedPosition {
    byte: usize,
    line: u64,
    column: u64,
}

impl SourceIndex {
    /// Indexes source positions after validating the normalized project-relative path.
    pub fn try_new(path: impl Into<String>, source: &str) -> Result<Self, InvalidSourceSpan> {
        let path = path.into();
        validate_path(&path)?;

        let mut positions = vec![IndexedPosition {
            byte: 0,
            line: 1,
            column: 1,
        }];
        let mut line = 1;
        let mut column = 1;
        let mut characters = source.char_indices().peekable();
        while let Some((byte, character)) = characters.next() {
            let mut end_byte = byte + character.len_utf8();
            match character {
                '\r' => {
                    if characters
                        .peek()
                        .is_some_and(|(_, character)| *character == '\n')
                    {
                        let (byte, character) = characters
                            .next()
                            .expect("the peeked CRLF line feed is present");
                        end_byte = byte + character.len_utf8();
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
            positions.push(IndexedPosition {
                byte: end_byte,
                line,
                column,
            });
        }

        Ok(Self {
            path,
            source_len: source.len(),
            positions,
        })
    }

    /// Constructs a span by validating byte boundaries and coordinates against
    /// the indexed source positions.
    #[allow(clippy::too_many_arguments)]
    pub fn try_span(
        &self,
        start_byte: u64,
        end_byte: u64,
        start_line: u64,
        start_column: u64,
        end_line: u64,
        end_column: u64,
    ) -> Result<SourceSpan, InvalidSourceSpan> {
        if start_byte > end_byte {
            return Err(InvalidSourceSpan::ReversedBytes);
        }
        let start_index =
            usize::try_from(start_byte).map_err(|_| InvalidSourceSpan::ByteOutOfBounds)?;
        let end_index =
            usize::try_from(end_byte).map_err(|_| InvalidSourceSpan::ByteOutOfBounds)?;
        if end_index > self.source_len {
            return Err(InvalidSourceSpan::ByteOutOfBounds);
        }
        let expected_start = self.position(start_index)?;
        let expected_end = self.position(end_index)?;
        if start_line == 0 || start_column == 0 || end_line == 0 || end_column == 0 {
            return Err(InvalidSourceSpan::ZeroCoordinate);
        }
        if start_line > end_line || (start_line == end_line && start_column > end_column) {
            return Err(InvalidSourceSpan::ReversedCoordinates);
        }
        let empty_bytes = start_byte == end_byte;
        let empty_coordinates = start_line == end_line && start_column == end_column;
        if empty_bytes != empty_coordinates {
            return Err(InvalidSourceSpan::InconsistentRange);
        }
        if (expected_start.line, expected_start.column) != (start_line, start_column)
            || (expected_end.line, expected_end.column) != (end_line, end_column)
        {
            return Err(InvalidSourceSpan::CoordinateMismatch);
        }
        Ok(SourceSpan {
            path: self.path.clone(),
            start_byte,
            end_byte,
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }

    /// Returns the one-based line and Unicode-scalar column at a legal byte boundary.
    pub fn coordinates_at(&self, byte: u64) -> Result<(u64, u64), InvalidSourceSpan> {
        let byte = usize::try_from(byte).map_err(|_| InvalidSourceSpan::ByteOutOfBounds)?;
        if byte > self.source_len {
            return Err(InvalidSourceSpan::ByteOutOfBounds);
        }
        let position = self.position(byte)?;
        Ok((position.line, position.column))
    }

    /// Returns the exact half-open span of the complete indexed source.
    pub fn document_span(&self) -> SourceSpan {
        let end = *self
            .positions
            .last()
            .expect("a source index always contains its initial position");
        SourceSpan {
            path: self.path.clone(),
            start_byte: 0,
            end_byte: self.source_len as u64,
            start_line: 1,
            start_column: 1,
            end_line: end.line,
            end_column: end.column,
        }
    }

    fn position(&self, byte: usize) -> Result<IndexedPosition, InvalidSourceSpan> {
        self.positions
            .binary_search_by_key(&byte, |position| position.byte)
            .map(|index| self.positions[index])
            .map_err(|_| InvalidSourceSpan::InvalidBoundary)
    }
}

/// The line-ending style retained for one complete source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LineEnding {
    Lf,
    CrLf,
    Mixed,
    None,
}

impl LineEnding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::CrLf => "crlf",
            Self::Mixed => "mixed",
            Self::None => "none",
        }
    }
}

/// Complete, unnormalized UTF-8 source text retained before structural parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceText {
    text: String,
    line_ending: LineEnding,
}

impl SourceText {
    pub fn new(text: String) -> Self {
        let line_ending = detect_line_ending(text.as_bytes());
        Self { text, line_ending }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub const fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn into_string(self) -> String {
        self.text
    }
}

impl From<String> for SourceText {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// One complete project source document with stable logical provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    source: SourceText,
    source_index: SourceIndex,
    span: SourceSpan,
}

impl SourceDocument {
    pub fn try_new(path: impl Into<String>, source: SourceText) -> Result<Self, InvalidSourceSpan> {
        let source_index = SourceIndex::try_new(path, source.as_str())?;
        let span = source_index.document_span();
        Ok(Self {
            source,
            source_index,
            span,
        })
    }

    pub fn path(&self) -> &str {
        self.span.path()
    }

    pub const fn source(&self) -> &SourceText {
        &self.source
    }

    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

fn detect_line_ending(bytes: &[u8]) -> LineEnding {
    let mut saw_lf = false;
    let mut saw_crlf = false;
    let mut saw_other_cr = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                saw_crlf = true;
                index += 2;
            }
            b'\r' => {
                saw_other_cr = true;
                index += 1;
            }
            b'\n' => {
                saw_lf = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
    match (saw_lf, saw_crlf, saw_other_cr) {
        (false, false, false) => LineEnding::None,
        (true, false, false) => LineEnding::Lf,
        (false, true, false) => LineEnding::CrLf,
        _ => LineEnding::Mixed,
    }
}

/// An exact half-open range in one normalized project-relative UTF-8 source path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceSpan {
    path: String,
    start_byte: u64,
    end_byte: u64,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
}

impl SourceSpan {
    /// Constructs a span after validating its path, byte boundaries, and
    /// coordinates against the original UTF-8 source text.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        path: impl Into<String>,
        source: &str,
        start_byte: u64,
        end_byte: u64,
        start_line: u64,
        start_column: u64,
        end_line: u64,
        end_column: u64,
    ) -> Result<Self, InvalidSourceSpan> {
        SourceIndex::try_new(path, source)?.try_span(
            start_byte,
            end_byte,
            start_line,
            start_column,
            end_line,
            end_column,
        )
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn start_byte(&self) -> u64 {
        self.start_byte
    }

    pub const fn end_byte(&self) -> u64 {
        self.end_byte
    }

    pub const fn start_line(&self) -> u64 {
        self.start_line
    }

    pub const fn start_column(&self) -> u64 {
        self.start_column
    }

    pub const fn end_line(&self) -> u64 {
        self.end_line
    }

    pub const fn end_column(&self) -> u64 {
        self.end_column
    }

    pub const fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }
}

fn validate_path(path: &str) -> Result<(), InvalidSourceSpan> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path.contains('\\')
        || has_windows_drive_prefix(path)
        || has_uri_scheme(path)
    {
        return Err(InvalidSourceSpan::InvalidPath);
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(InvalidSourceSpan::InvalidPath);
    }
    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn has_uri_scheme(path: &str) -> bool {
    let Some((scheme, _)) = path.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSourceSpan {
    InvalidPath,
    ReversedBytes,
    ByteOutOfBounds,
    InvalidBoundary,
    ZeroCoordinate,
    ReversedCoordinates,
    InconsistentRange,
    CoordinateMismatch,
}

impl fmt::Display for InvalidSourceSpan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidPath => "source path is not normalized and project-relative",
            Self::ReversedBytes => "source byte range is reversed",
            Self::ByteOutOfBounds => "source byte range exceeds the source text",
            Self::InvalidBoundary => {
                "source span boundary splits a UTF-8 code point or CRLF sequence"
            }
            Self::ZeroCoordinate => "source lines and columns are one-based",
            Self::ReversedCoordinates => "source coordinate range is reversed",
            Self::InconsistentRange => {
                "source byte and coordinate ranges disagree about whether the span is empty"
            }
            Self::CoordinateMismatch => {
                "source coordinates do not correspond to the source byte range"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for InvalidSourceSpan {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_spans_enforce_wire_shape_invariants() {
        let source = "é\r\nx";
        let span = SourceSpan::try_new(".mara/schema.yaml", source, 0, 5, 1, 1, 2, 2).unwrap();
        assert_eq!(span.path(), ".mara/schema.yaml");
        assert_eq!(span.start_byte(), 0);
        assert_eq!(span.end_column(), 2);
        assert!(!span.is_empty());

        let index = SourceIndex::try_new(".mara/schema.yaml", source).unwrap();
        assert_eq!(index.coordinates_at(4), Ok((2, 1)));
        assert_eq!(
            index.try_span(2, 4, 1, 2, 2, 1).unwrap(),
            SourceSpan::try_new(".mara/schema.yaml", source, 2, 4, 1, 2, 2, 1).unwrap()
        );

        assert_eq!(
            SourceSpan::try_new("../schema.yaml", "", 0, 0, 1, 1, 1, 1),
            Err(InvalidSourceSpan::InvalidPath)
        );
        assert_eq!(
            SourceSpan::try_new("C:/schema.yaml", "", 0, 0, 1, 1, 1, 1),
            Err(InvalidSourceSpan::InvalidPath)
        );
        assert_eq!(
            SourceSpan::try_new("https:schema.yaml", "", 0, 0, 1, 1, 1, 1),
            Err(InvalidSourceSpan::InvalidPath)
        );
        assert_eq!(
            SourceSpan::try_new("schema\0.yaml", "", 0, 0, 1, 1, 1, 1),
            Err(InvalidSourceSpan::InvalidPath)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", "ab", 2, 1, 1, 3, 1, 2),
            Err(InvalidSourceSpan::ReversedBytes)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", "ab", 1, 1, 1, 2, 1, 3),
            Err(InvalidSourceSpan::InconsistentRange)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", "ab", 1, 2, 1, 2, 1, 2),
            Err(InvalidSourceSpan::InconsistentRange)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", source, 0, 6, 1, 1, 2, 3),
            Err(InvalidSourceSpan::ByteOutOfBounds)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", source, 1, 1, 1, 2, 1, 2),
            Err(InvalidSourceSpan::InvalidBoundary)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", source, 3, 3, 1, 2, 1, 2),
            Err(InvalidSourceSpan::InvalidBoundary)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", source, 0, 2, 1, 1, 1, 3),
            Err(InvalidSourceSpan::CoordinateMismatch)
        );
    }

    #[test]
    fn source_documents_retain_text_provenance_and_line_endings() {
        for (source, expected) in [
            ("plain", LineEnding::None),
            ("a\nb\n", LineEnding::Lf),
            ("a\r\nb\r\n", LineEnding::CrLf),
            ("a\r\nb\n", LineEnding::Mixed),
        ] {
            let document =
                SourceDocument::try_new("docs/example.mara.md", SourceText::new(source.to_owned()))
                    .unwrap();
            assert_eq!(document.path(), "docs/example.mara.md");
            assert_eq!(document.source().as_str(), source);
            assert_eq!(document.source().line_ending(), expected);
            assert_eq!(document.span().end_byte(), source.len() as u64);
            assert_eq!(
                document
                    .source_index()
                    .coordinates_at(source.len() as u64)
                    .unwrap(),
                (document.span().end_line(), document.span().end_column())
            );
        }
    }
}
