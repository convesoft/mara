//! Pages and cursors: REQ-BOUNDED-PAGES, REQ-PAGE-LIMITS and DES-PAGING in
//! `docs/retrieval.mara.md`.

use std::fmt;

use crate::hash::Hash;

/// Records per page when the request names no number.
pub const DEFAULT_LIMIT: usize = 20;
/// The most records a request may ask for per page.
pub const MAX_LIMIT: usize = 100;
/// The most bytes of a page's JSON form.
pub const PAGE_BYTES: usize = 65_536;
/// What a page's JSON form holds besides its records, at most: the result's other members.
const ENVELOPE_BYTES: usize = 1_024;

/// Reads a requested number of records per page: 1 to [`MAX_LIMIT`].
pub fn parse_limit(text: &str) -> Result<usize, String> {
    match text.parse::<usize>() {
        Ok(limit) if (1..=MAX_LIMIT).contains(&limit) => Ok(limit),
        _ => Err(format!(
            "a page holds 1 to {MAX_LIMIT} records, not `{text}`"
        )),
    }
}

/// Where the next page of a result starts, with the key of the result, so that a cursor of a
/// result that has changed since is rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub key: Hash,
    /// Records of the result before the next page.
    pub offset: usize,
}

impl Cursor {
    /// Reads a cursor in the form [`fmt::Display`] writes.
    pub fn parse(text: &str) -> Option<Cursor> {
        let (key, offset) = text.split_once('.')?;
        let offset = offset
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| offset.parse().ok())??;
        Some(Cursor {
            key: Hash::from_hex(key)?,
            offset,
        })
    }
}

impl fmt::Display for Cursor {
    /// The key in hexadecimal, a `.` and the offset in decimal.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.key, self.offset)
    }
}

/// The records of one page, each in JSON form, and whether records remain after them.
#[derive(Debug, PartialEq, Eq)]
pub struct Filled {
    pub records: Vec<String>,
    pub more: bool,
}

/// Takes records from `records` until `limit` are taken or the next would take the page past
/// [`PAGE_BYTES`]. Each record is serialized once (DES-PAGING). A first record that alone
/// cannot fit fails the page with its index in `records`.
pub fn fill(mut records: impl Iterator<Item = String>, limit: usize) -> Result<Filled, usize> {
    let budget = PAGE_BYTES - ENVELOPE_BYTES;
    let mut filled = Filled {
        records: Vec::new(),
        more: false,
    };
    let mut bytes = 0;
    for record in records.by_ref() {
        // Each record after the first takes a comma.
        let size = record.len() + usize::from(!filled.records.is_empty());
        if filled.records.len() == limit || bytes + size > budget {
            if filled.records.is_empty() {
                return Err(0);
            }
            filled.more = true;
            return Ok(filled);
        }
        bytes += size;
        filled.records.push(record);
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::WorkCounts;

    #[test]
    fn reads_limits_from_1_to_100() {
        assert_eq!(parse_limit("1"), Ok(1));
        assert_eq!(parse_limit("100"), Ok(100));
        for text in ["0", "101", "-1", "x", "", " 5"] {
            assert!(parse_limit(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn reads_back_the_cursors_it_writes() {
        let key = Hash::of_file(b"result", &mut WorkCounts::default());
        let cursor = Cursor { key, offset: 40 };
        assert_eq!(Cursor::parse(&cursor.to_string()), Some(cursor));
        for text in [
            "",
            "40",
            ".40",
            &format!("{key}"),
            &format!("{key}."),
            &format!("{key}.+4"),
        ] {
            assert_eq!(Cursor::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn fills_a_page_up_to_its_limit_and_its_bytes() {
        let records = |count: usize, size: usize| (0..count).map(move |_| "x".repeat(size));
        let filled = fill(records(3, 10), 2).unwrap();
        assert_eq!((filled.records.len(), filled.more), (2, true));
        let filled = fill(records(2, 10), 2).unwrap();
        assert_eq!((filled.records.len(), filled.more), (2, false));
        let size = 10_000;
        let filled = fill(records(100, size), 100).unwrap();
        let bytes: usize = filled.records.iter().map(|r| r.len() + 1).sum();
        assert!(filled.more && bytes + ENVELOPE_BYTES <= PAGE_BYTES + 1);
        assert_eq!(
            filled.records.len(),
            (PAGE_BYTES - ENVELOPE_BYTES) / (size + 1)
        );
        assert_eq!(fill(records(1, PAGE_BYTES), 20), Err(0));
    }
}
