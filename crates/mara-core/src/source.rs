use std::{error::Error, fmt};

/// A reusable index of every legal source-span boundary in one UTF-8 source.
#[derive(Debug, Clone)]
pub struct SourceIndex {
    path: String,
    source_len: usize,
    positions: Vec<IndexedPosition>,
}

#[derive(Debug, Clone, Copy)]
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

    fn position(&self, byte: usize) -> Result<IndexedPosition, InvalidSourceSpan> {
        self.positions
            .binary_search_by_key(&byte, |position| position.byte)
            .map(|index| self.positions[index])
            .map_err(|_| InvalidSourceSpan::InvalidBoundary)
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
}
