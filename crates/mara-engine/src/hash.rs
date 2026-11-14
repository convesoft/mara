//! Content hashes and the keys built from them (DES-INPUTS-AND-KEYS in `docs/engine.mara.md`).
//!
//! The engine hashes, so every host derives the same keys from the same contents.

use std::fmt;

use crate::work::WorkCounts;

/// A BLAKE3 hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Hashes the contents of one input file and counts one hash in `work`.
    pub fn of_file(contents: &[u8], work: &mut WorkCounts) -> Hash {
        work.hashes += 1;
        Hash(*blake3::hash(contents).as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The hash whose lowercase hexadecimal form ([`fmt::Display`]) is `text`.
    pub fn from_hex(text: &str) -> Option<Hash> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let mut hash = [0; 32];
        for (index, pair) in bytes.chunks(2).enumerate() {
            hash[index] = digit(pair[0])? << 4 | digit(pair[1])?;
        }
        Some(Hash(hash))
    }
}

impl fmt::Display for Hash {
    /// Lowercase hexadecimal.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({self})")
    }
}

/// Builds a key from the hashes and names it covers. Each key kind has its own name and
/// version, so keys of different kinds or derivation versions never collide. Not a file hash,
/// so it counts nothing.
pub(crate) struct KeyHasher(blake3::Hasher);

impl KeyHasher {
    pub(crate) fn new(kind: &str, version: u32) -> KeyHasher {
        let mut hasher = KeyHasher(blake3::Hasher::new());
        hasher.bytes(kind.as_bytes());
        hasher.0.update(&version.to_le_bytes());
        hasher
    }

    /// Adds `bytes` with their length, so no two sequences of parts give the same input.
    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.0.update(&(bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }

    pub(crate) fn hash(&mut self, hash: &Hash) {
        self.0.update(&hash.0);
    }

    /// Adds a hash that may be absent, distinctly from every present one.
    pub(crate) fn optional(&mut self, hash: Option<&Hash>) {
        match hash {
            Some(hash) => {
                self.0.update(&[1]);
                self.hash(hash);
            }
            None => {
                self.0.update(&[0]);
            }
        }
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.0.update(&value.to_le_bytes());
    }

    pub(crate) fn finish(&self) -> Hash {
        Hash(*self.0.finalize().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_contents_with_blake3_and_counts_each_hash() {
        let mut work = WorkCounts::default();
        let empty = Hash::of_file(b"", &mut work);
        assert_eq!(
            empty.to_string(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(
            Hash::of_file(b"a", &mut work),
            Hash::of_file(b"a", &mut work)
        );
        assert_ne!(Hash::of_file(b"a", &mut work), empty);
        assert_eq!(
            work,
            WorkCounts {
                hashes: 4,
                ..WorkCounts::default()
            }
        );
    }

    #[test]
    fn keys_separate_kinds_versions_and_parts() {
        let key = |kind: &str, version: u32, parts: &[&[u8]]| {
            let mut hasher = KeyHasher::new(kind, version);
            parts.iter().for_each(|part| hasher.bytes(part));
            hasher.finish()
        };
        let base = key("a", 1, &[b"xy"]);
        assert_eq!(base, key("a", 1, &[b"xy"]));
        assert_ne!(base, key("b", 1, &[b"xy"]));
        assert_ne!(base, key("a", 2, &[b"xy"]));
        assert_ne!(base, key("a", 1, &[b"x", b"y"]));
        let optional = |hash: Option<&Hash>| {
            let mut hasher = KeyHasher::new("a", 1);
            hasher.optional(hash);
            hasher.finish()
        };
        let zero = Hash([0; 32]);
        assert_ne!(optional(None), optional(Some(&zero)));
    }

    #[test]
    fn reads_back_its_hexadecimal_form() {
        let hash = Hash::of_file(b"a", &mut WorkCounts::default());
        assert_eq!(Hash::from_hex(&hash.to_string()), Some(hash));
        assert_eq!(Hash::from_hex(&hash.to_string().to_uppercase()), None);
        assert_eq!(Hash::from_hex("ab"), None);
        assert_eq!(Hash::from_hex(&"g".repeat(64)), None);
    }
}
