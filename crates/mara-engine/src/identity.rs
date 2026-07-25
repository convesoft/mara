use std::{error::Error, fmt, time::SystemTime};

use mara_core::{Mid, MidIdentity};

const ULID_RANDOM_BYTES: usize = 10;
const ULID_TIMESTAMP_BITS: u32 = 48;
const MAX_ULID_TIMESTAMP: u128 = (1_u128 << ULID_TIMESTAMP_BITS) - 1;

/// Generates one collision-resistant MID from the current time and OS randomness.
pub fn generate_mid(identity: &MidIdentity) -> Result<Mid, MidGenerationError> {
    let mut random = [0_u8; ULID_RANDOM_BYTES];
    getrandom::fill(&mut random).map_err(MidGenerationError::Randomness)?;
    generate_mid_from_parts(identity, SystemTime::now(), random)
}

fn generate_mid_from_parts(
    identity: &MidIdentity,
    timestamp: SystemTime,
    random: [u8; ULID_RANDOM_BYTES],
) -> Result<Mid, MidGenerationError> {
    let elapsed = timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| MidGenerationError::ClockBeforeUnixEpoch)?;
    let timestamp_ms = elapsed.as_millis();
    if timestamp_ms > MAX_ULID_TIMESTAMP {
        return Err(MidGenerationError::TimestampOutOfRange);
    }

    let mut bytes = [0_u8; 16];
    let timestamp_bytes = (timestamp_ms as u64).to_be_bytes();
    bytes[..6].copy_from_slice(&timestamp_bytes[2..]);
    bytes[6..].copy_from_slice(&random);
    Ok(Mid::from_ulid_value(identity, u128::from_be_bytes(bytes)))
}

/// A structured operational failure while obtaining generation inputs.
#[derive(Debug)]
pub enum MidGenerationError {
    ClockBeforeUnixEpoch,
    TimestampOutOfRange,
    Randomness(getrandom::Error),
}

impl fmt::Display for MidGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClockBeforeUnixEpoch => {
                formatter.write_str("system clock is before the Unix epoch")
            }
            Self::TimestampOutOfRange => {
                formatter.write_str("system time exceeds the 48-bit ULID timestamp range")
            }
            Self::Randomness(error) => {
                write!(formatter, "operating-system randomness failed: {error}")
            }
        }
    }
}

impl Error for MidGenerationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Randomness(error) => Some(error),
            Self::ClockBeforeUnixEpoch | Self::TimestampOutOfRange => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, time::Duration};

    use mara_core::{MidFormat, SchemaField, SourceIndex};

    use super::*;

    fn identity(prefix: &str) -> MidIdentity {
        let index = SourceIndex::try_new("schema.yaml", "").unwrap();
        let span = index.try_span(0, 0, 1, 1, 1, 1).unwrap();
        MidIdentity::new(
            SchemaField::new(span.clone(), span.clone(), MidFormat::Ulid),
            SchemaField::new(span.clone(), span, prefix.to_owned()),
        )
    }

    #[test]
    fn deterministic_components_use_the_configured_representation() {
        let identity = identity("mid_");
        let mid = generate_mid_from_parts(
            &identity,
            SystemTime::UNIX_EPOCH + Duration::from_millis(1),
            [0; ULID_RANDOM_BYTES],
        )
        .unwrap();

        assert_eq!(mid.as_str(), "mid_00000000010000000000000000");
        assert_eq!(Mid::parse(mid.as_str(), &identity).unwrap(), mid);
    }

    #[test]
    fn generation_rejects_timestamps_outside_the_ulid_range() {
        let identity = identity("m_");
        let before_epoch = SystemTime::UNIX_EPOCH - Duration::from_millis(1);
        assert!(matches!(
            generate_mid_from_parts(&identity, before_epoch, [0; ULID_RANDOM_BYTES]),
            Err(MidGenerationError::ClockBeforeUnixEpoch)
        ));

        let after_range =
            SystemTime::UNIX_EPOCH + Duration::from_millis(MAX_ULID_TIMESTAMP as u64 + 1);
        assert!(matches!(
            generate_mid_from_parts(&identity, after_range, [0; ULID_RANDOM_BYTES]),
            Err(MidGenerationError::TimestampOutOfRange)
        ));
    }

    #[test]
    fn generated_mids_are_valid_and_collision_resistant() {
        let identity = identity("m_");
        let mut generated = BTreeSet::new();

        for _ in 0..2_048 {
            let mid = generate_mid(&identity).unwrap();
            assert_eq!(Mid::parse(mid.as_str(), &identity).unwrap(), mid);
            assert!(generated.insert(mid));
        }
    }
}
