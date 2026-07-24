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
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        path: impl Into<String>,
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
        if start_line == 0 || start_column == 0 || end_line == 0 || end_column == 0 {
            return Err(InvalidSourceSpan::ZeroCoordinate);
        }
        if start_line > end_line || (start_line == end_line && start_column > end_column) {
            return Err(InvalidSourceSpan::ReversedCoordinates);
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

fn validate_path(path: &str) -> Result<(), InvalidSourceSpan> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSourceSpan {
    InvalidPath,
    ReversedBytes,
    ZeroCoordinate,
    ReversedCoordinates,
}

impl fmt::Display for InvalidSourceSpan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidPath => "source path is not normalized and project-relative",
            Self::ReversedBytes => "source byte range is reversed",
            Self::ZeroCoordinate => "source lines and columns are one-based",
            Self::ReversedCoordinates => "source coordinate range is reversed",
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
        let span = SourceSpan::try_new(".mara/schema.yaml", 0, 4, 1, 1, 1, 5).unwrap();
        assert_eq!(span.path(), ".mara/schema.yaml");
        assert_eq!(span.start_byte(), 0);
        assert_eq!(span.end_column(), 5);
        assert!(!span.is_empty());

        assert_eq!(
            SourceSpan::try_new("../schema.yaml", 0, 0, 1, 1, 1, 1),
            Err(InvalidSourceSpan::InvalidPath)
        );
        assert_eq!(
            SourceSpan::try_new("schema.yaml", 2, 1, 1, 3, 1, 2),
            Err(InvalidSourceSpan::ReversedBytes)
        );
    }
}
