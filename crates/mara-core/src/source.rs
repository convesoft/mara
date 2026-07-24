use std::{error::Error, fmt};

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
        let path = path.into();
        validate_path(&path)?;
        if start_byte > end_byte {
            return Err(InvalidSourceSpan::ReversedBytes);
        }
        let start_index =
            usize::try_from(start_byte).map_err(|_| InvalidSourceSpan::ByteOutOfBounds)?;
        let end_index =
            usize::try_from(end_byte).map_err(|_| InvalidSourceSpan::ByteOutOfBounds)?;
        if end_index > source.len() {
            return Err(InvalidSourceSpan::ByteOutOfBounds);
        }
        if !source.is_char_boundary(start_index)
            || !source.is_char_boundary(end_index)
            || splits_crlf(source, start_index)
            || splits_crlf(source, end_index)
        {
            return Err(InvalidSourceSpan::InvalidBoundary);
        }
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
        let expected_start = position_at(source, start_index);
        let expected_end = position_at(source, end_index);
        if expected_start != (start_line, start_column) || expected_end != (end_line, end_column) {
            return Err(InvalidSourceSpan::CoordinateMismatch);
        }
        Ok(Self {
            path,
            start_byte,
            end_byte,
            start_line,
            start_column,
            end_line,
            end_column,
        })
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

fn splits_crlf(source: &str, byte: usize) -> bool {
    byte > 0
        && byte < source.len()
        && source.as_bytes()[byte - 1] == b'\r'
        && source.as_bytes()[byte] == b'\n'
}

fn position_at(source: &str, byte: usize) -> (u64, u64) {
    let mut line = 1;
    let mut column = 1;
    let mut characters = source[..byte].chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
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
