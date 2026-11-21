//! The vocabulary index of typo-tolerant search (DES-SNAPSHOT): symmetric deletion, after Bocek,
//! Hunt and Stiller, "Fast Similarity Search in Large Dictionaries" (2007).
//!
//! Two words within `k` edits share a string that at most `k` deletions make of each: a
//! substitution or a swap of two adjacent characters takes one deletion from each word, an
//! insertion one from the longer word. So each word is indexed under every string that up to `k`
//! deletions make of it, and a query word looks up every string that up to `k` deletions make
//! of itself. The words found are candidates; the caller counts their edits.
//!
//! A word makes about `n²/2` strings by two deletions, so typo tolerance stops at query words of
//! [`LONGEST_TOLERANT`] characters: the index holds only the words within reach of those, and a
//! longer query word matches only exactly (REQ-TYPO-TOLERANT-SEARCH).

use std::ops::RangeInclusive;

/// The longest query word that matches a word within edits; a longer one matches only exactly.
pub const LONGEST_TOLERANT: usize = 18;

/// The lengths of the query words allowed each number of edits.
const TIERS: [(usize, RangeInclusive<usize>); 2] = [(1, 4..=7), (2, 8..=LONGEST_TOLERANT)];

// @mara implements REQ-TYPO-TOLERANT-SEARCH
/// The edits that a query word of `length` characters may be away from a word it matches: one
/// from 4 to 7 characters, two from 8 to [`LONGEST_TOLERANT`], and none otherwise.
pub fn edits_allowed(length: usize) -> usize {
    TIERS
        .iter()
        .find(|(_, lengths)| lengths.contains(&length))
        .map_or(0, |(edits, _)| *edits)
}

/// The lengths of the words within `edits` edits of a query word allowed `edits`, 1 or 2 (any
/// other number counts as 2): from the shortest such query word less `edits` to the longest plus
/// `edits`.
pub fn reachable(edits: usize) -> RangeInclusive<usize> {
    let (edits, lengths) = TIERS
        .iter()
        .find(|(allowed, _)| *allowed == edits)
        .unwrap_or(&TIERS[TIERS.len() - 1]);
    lengths.start().saturating_sub(*edits)..=lengths.end() + edits
}

/// Words indexed under the strings that up to a number of deletions make of them.
#[derive(Debug)]
pub struct Spellings {
    words: Vec<Box<str>>,
    /// For each such string, the low 32 bits of its hash and the index of its word, grouped in
    /// buckets by the top `bits` bits of the hash. Eight bytes an entry keep the index compact.
    entries: Vec<(u32, u32)>,
    /// Where each bucket starts in `entries`, and where the last ends.
    starts: Vec<usize>,
    bits: u32,
    deletions: usize,
}

impl Spellings {
    /// Indexes `words` under the strings that up to `deletions` deletions make of each.
    pub(crate) fn new<'w>(words: impl Iterator<Item = &'w str>, deletions: usize) -> Spellings {
        let words: Vec<Box<str>> = words.map(Box::from).collect();
        let chars: Vec<Vec<char>> = words.iter().map(|word| word.chars().collect()).collect();
        let total: usize = chars
            .iter()
            .map(|word| variant_count(word.len(), deletions))
            .sum();
        // About four entries per bucket. A counting sort by bucket, in two passes over the
        // strings, keeps the build linear without a second copy of the entries.
        let bits = (total / 4).max(1).next_power_of_two().trailing_zeros();
        let mut starts = vec![0; (1 << bits) + 1];
        let mut hashes = Hashes::default();
        for word in &chars {
            hashes.variants(word, deletions, |hash| starts[bucket(hash, bits) + 1] += 1);
        }
        for index in 1..starts.len() {
            starts[index] += starts[index - 1];
        }
        let mut next = starts.clone();
        let mut entries = vec![(0, 0); total];
        for (index, word) in chars.iter().enumerate() {
            // Cannot fire: the words are distinct keys of the token index in memory, far fewer
            // than 2^32.
            let index = u32::try_from(index).expect("fewer than 2^32 distinct words");
            hashes.variants(word, deletions, |hash| {
                let slot = &mut next[bucket(hash, bits)];
                entries[*slot] = (hash as u32, index);
                *slot += 1;
            });
        }
        Spellings {
            words,
            entries,
            starts,
            bits,
            deletions,
        }
    }

    /// The indexed words that share with `query` a string that up to the index's deletions make
    /// of each, each once, in no particular order. Every word within that many edits of `query`
    /// is among them.
    pub fn candidates(&self, query: &[char]) -> Vec<&str> {
        let mut found: Vec<u32> = Vec::new();
        Hashes::default().variants(query, self.deletions, |hash| {
            let at = bucket(hash, self.bits);
            let entries = &self.entries[self.starts[at]..self.starts[at + 1]];
            let same = entries.iter().filter(|&&(low, _)| low == hash as u32);
            found.extend(same.map(|&(_, index)| index));
        });
        found.sort_unstable();
        found.dedup();
        found
            .into_iter()
            .filter_map(|index| self.words.get(index as usize))
            .map(|word| &**word)
            .collect()
    }
}

/// How many strings [`Hashes::variants`] gives for a word of `length` characters.
fn variant_count(length: usize, deletions: usize) -> usize {
    let mut count = 1;
    if deletions >= 1 {
        count += length;
    }
    if deletions >= 2 {
        count += length * length.saturating_sub(1) / 2;
    }
    count
}

/// The bucket of `hash`: its top `bits` bits.
fn bucket(hash: u64, bits: u32) -> usize {
    match bits {
        0 => 0,
        bits => (hash >> (64 - bits)) as usize,
    }
}

/// The hashes of a word and of the strings that deletions make of it, each in constant time
/// from prefix and suffix sums of a polynomial hash, with buffers kept between words.
#[derive(Default)]
struct Hashes {
    /// `BASE` to the power of each index.
    powers: Vec<u64>,
    /// The polynomial hash of each prefix, and of each suffix.
    prefixes: Vec<u64>,
    suffixes: Vec<u64>,
}

/// The odd base of the polynomial hash, so that its powers never vanish modulo 2⁶⁴.
const BASE: u64 = 0x9E37_79B9_7F4A_7C15;

impl Hashes {
    /// Calls `each` with the hash of `chars` and of every string that up to `deletions` (at most
    /// 2) deletions make of it.
    fn variants(&mut self, chars: &[char], deletions: usize, mut each: impl FnMut(u64)) {
        let n = chars.len();
        let value = |c: char| u64::from(c) + 1;
        self.powers.clear();
        self.powers.push(1);
        self.prefixes.clear();
        self.prefixes.push(0);
        for (index, &c) in chars.iter().enumerate() {
            self.powers.push(self.powers[index].wrapping_mul(BASE));
            let prefix = self.prefixes[index]
                .wrapping_mul(BASE)
                .wrapping_add(value(c));
            self.prefixes.push(prefix);
        }
        self.suffixes.clear();
        self.suffixes.resize(n + 1, 0);
        for index in (0..n).rev() {
            let term = value(chars[index]).wrapping_mul(self.powers[n - 1 - index]);
            self.suffixes[index] = term.wrapping_add(self.suffixes[index + 1]);
        }
        let (powers, prefixes, suffixes) = (&self.powers, &self.prefixes, &self.suffixes);
        // The hash of `x` then `y` is the hash of `x` times `BASE` to the length of `y`, plus the
        // hash of `y`.
        each(mix(prefixes[n]));
        if deletions >= 1 {
            for i in 0..n {
                each(mix(prefixes[i]
                    .wrapping_mul(powers[n - 1 - i])
                    .wrapping_add(suffixes[i + 1])));
            }
        }
        if deletions >= 2 {
            for i in 0..n {
                for j in i + 1..n {
                    let middle =
                        prefixes[j].wrapping_sub(prefixes[i + 1].wrapping_mul(powers[j - i - 1]));
                    let hash = prefixes[i]
                        .wrapping_mul(powers[n - 2 - i])
                        .wrapping_add(middle.wrapping_mul(powers[n - 1 - j]))
                        .wrapping_add(suffixes[j + 1]);
                    each(mix(hash));
                }
            }
        }
    }
}

/// Spreads the bits of a polynomial hash over all 64, so that its top bits choose a bucket
/// evenly: the finalizer of SplitMix64. Equal strings hash alike; a collision only adds a
/// candidate.
fn mix(hash: u64) -> u64 {
    let mut z = hash;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
