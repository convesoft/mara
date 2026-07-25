use std::{error::Error, fmt};

use ulid::Ulid;

use crate::{MidFormat, MidIdentity};

const ULID_LENGTH: usize = 26;

/// The immutable machine identity of one Mara item.
///
/// Construction always requires the project schema's identity configuration, so
/// callers cannot accidentally accept an unprefixed or differently prefixed ULID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mid(String);

impl Mid {
    /// Parses one MID using the schema-configured representation.
    pub fn parse(value: &str, identity: &MidIdentity) -> Result<Self, MidParseError> {
        match identity.format().value() {
            MidFormat::Ulid => Self::parse_ulid(value, identity.prefix().value()),
        }
    }

    /// Constructs one MID from an exact 128-bit ULID value.
    ///
    /// This is the pure adapter boundary used by generators that obtain time and
    /// randomness outside `mara-core`.
    pub fn from_ulid_value(identity: &MidIdentity, value: u128) -> Self {
        match identity.format().value() {
            MidFormat::Ulid => {
                let prefix = identity.prefix().value();
                let encoded = Ulid::from(value).to_string();
                let mut representation = String::with_capacity(prefix.len() + ULID_LENGTH);
                representation.push_str(prefix);
                representation.push_str(&encoded);
                Self(representation)
            }
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn parse_ulid(value: &str, prefix: &str) -> Result<Self, MidParseError> {
        let encoded = value
            .strip_prefix(prefix)
            .ok_or_else(|| MidParseError::PrefixMismatch {
                expected: prefix.to_owned(),
            })?;

        let actual_length = encoded.chars().count();
        if actual_length != ULID_LENGTH {
            return Err(MidParseError::InvalidUlidLength {
                actual: actual_length,
            });
        }

        for (index, character) in encoded.chars().enumerate() {
            if !is_canonical_ulid_character(character) {
                return Err(MidParseError::InvalidUlidCharacter { index, character });
            }
        }

        if encoded.as_bytes()[0] > b'7' {
            return Err(MidParseError::UlidOutOfRange);
        }

        debug_assert!(Ulid::from_string(encoded).is_ok());
        Ok(Self(value.to_owned()))
    }
}

impl AsRef<str> for Mid {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Mid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A Mara-owned explanation of why a string is not a canonical project MID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidParseError {
    PrefixMismatch { expected: String },
    InvalidUlidLength { actual: usize },
    InvalidUlidCharacter { index: usize, character: char },
    UlidOutOfRange,
}

impl fmt::Display for MidParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrefixMismatch { expected } => {
                write!(
                    formatter,
                    "MID must start with configured prefix {expected:?}"
                )
            }
            Self::InvalidUlidLength { actual } => write!(
                formatter,
                "MID ULID must contain {ULID_LENGTH} characters, found {actual}"
            ),
            Self::InvalidUlidCharacter { index, character } => write!(
                formatter,
                "MID ULID contains non-canonical character {character:?} at index {index}"
            ),
            Self::UlidOutOfRange => {
                formatter.write_str("MID ULID exceeds the canonical 128-bit range")
            }
        }
    }
}

impl Error for MidParseError {}

const fn is_canonical_ulid_character(character: char) -> bool {
    matches!(
        character,
        '0'..='9'
            | 'A'..='H'
            | 'J'
            | 'K'
            | 'M'
            | 'N'
            | 'P'..='T'
            | 'V'..='Z'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MidFormat, SchemaField, SourceIndex};

    fn identity(prefix: &str) -> MidIdentity {
        let index = SourceIndex::try_new("schema.yaml", "").unwrap();
        let span = index.try_span(0, 0, 1, 1, 1, 1).unwrap();
        MidIdentity::new(
            SchemaField::new(span.clone(), span.clone(), MidFormat::Ulid),
            SchemaField::new(span.clone(), span, prefix.to_owned()),
        )
    }

    #[test]
    fn canonical_prefixed_ulids_round_trip_without_loss() {
        let identity = identity("m_");
        for encoded in [
            "00000000000000000000000000",
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "7ZZZZZZZZZZZZZZZZZZZZZZZZZ",
        ] {
            let value = format!("m_{encoded}");
            let mid = Mid::parse(&value, &identity).unwrap();
            assert_eq!(mid.as_str(), value);
            assert_eq!(mid.to_string(), value);
            assert_eq!(mid.as_ref(), value);
        }
    }

    #[test]
    fn rejects_wrong_prefixes_lengths_characters_case_and_range() {
        let identity = identity("m_");
        assert_eq!(
            Mid::parse("x_01ARZ3NDEKTSV4RRFFQ69G5FAV", &identity),
            Err(MidParseError::PrefixMismatch {
                expected: "m_".to_owned()
            })
        );

        for actual in [25, 27] {
            let value = format!("m_{}", "0".repeat(actual));
            assert_eq!(
                Mid::parse(&value, &identity),
                Err(MidParseError::InvalidUlidLength { actual })
            );
        }

        for character in ['I', 'L', 'O', 'U', '-', 'a', 'é'] {
            let value = format!("m_{character}{}", "0".repeat(25));
            assert_eq!(
                Mid::parse(&value, &identity),
                Err(MidParseError::InvalidUlidCharacter {
                    index: 0,
                    character
                })
            );
        }

        for first in ['8', '9', 'Z'] {
            let value = format!("m_{first}{}", "0".repeat(25));
            assert_eq!(
                Mid::parse(&value, &identity),
                Err(MidParseError::UlidOutOfRange)
            );
        }
    }

    #[test]
    fn representative_ulid_values_preserve_numeric_order_and_round_trip() {
        let identity = identity("project_");
        let mut state = 0x4d59_5df4_d0f3_3173_6c8e_9cf5_7093_2bd5_u128;
        let mut values = Vec::new();

        for _ in 0..2_048 {
            state = state
                .wrapping_mul(0xda94_2042_e4dd_58b5_d2d3_116a_58bf_3ce3)
                .wrapping_add(0x9e37_79b9_7f4a_7c15_6a09_e667_f3bc_c909);
            let mid = Mid::from_ulid_value(&identity, state);
            assert_eq!(Mid::parse(mid.as_str(), &identity).unwrap(), mid);
            values.push((state, mid));
        }

        let mut by_mid = values.clone();
        by_mid.sort_by(|left, right| left.1.cmp(&right.1));
        let mut numeric = values.iter().map(|(value, _)| *value).collect::<Vec<_>>();
        numeric.sort_unstable();
        assert_eq!(
            by_mid.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
            numeric
        );
    }
}
