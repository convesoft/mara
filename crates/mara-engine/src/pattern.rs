//! Include patterns: which paths below the project root the configuration declares
//! (DES-PROJECT-CONFIGURATION in `docs/project.mara.md`).
//!
//! A pattern is matched one path part at a time, so a walk knows, at each directory, whether a
//! pattern can still match something below it.

/// Compiled include patterns.
#[derive(Clone, Debug, Default)]
pub struct Patterns {
    patterns: Vec<Vec<Part>>,
}

#[derive(Clone, Debug)]
enum Part {
    /// `**`: any number of path parts, none included.
    Any,
    /// One path part: `*` matches any run of characters, `?` one character.
    Glob(String),
}

/// Where the patterns stand after the parts of a path: for each pattern, the indices of the
/// parts it can match next. An index equal to the pattern's length means it matched the path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Positions(Vec<(usize, usize)>);

impl Patterns {
    /// Compiles patterns written as DES-PROJECT-CONFIGURATION requires: relative, with `/`
    /// between non-empty parts. No pattern is invalid.
    pub fn new<'a>(patterns: impl IntoIterator<Item = &'a str>) -> Patterns {
        let patterns = patterns
            .into_iter()
            .map(|pattern| {
                pattern
                    .split('/')
                    .map(|part| match part {
                        "**" => Part::Any,
                        glob => Part::Glob(glob.to_owned()),
                    })
                    .collect()
            })
            .collect();
        Patterns { patterns }
    }

    /// The positions at the project root, before any part.
    pub fn start(&self) -> Positions {
        let mut positions = Vec::new();
        for pattern in 0..self.patterns.len() {
            self.enter(&mut positions, pattern, 0);
        }
        Positions(positions)
    }

    /// The positions after one more path part, `name`.
    pub fn advance(&self, at: &Positions, name: &str) -> Positions {
        let mut positions = Vec::new();
        for &(pattern, index) in &at.0 {
            match self.patterns[pattern].get(index) {
                Some(Part::Any) => self.enter(&mut positions, pattern, index),
                Some(Part::Glob(glob)) if glob_matches(glob, name) => {
                    self.enter(&mut positions, pattern, index + 1);
                }
                _ => {}
            }
        }
        positions.sort_unstable();
        positions.dedup();
        Positions(positions)
    }

    /// Whether a pattern matches the path that led to `at`.
    pub fn is_match(&self, at: &Positions) -> bool {
        at.0.iter()
            .any(|&(pattern, index)| index == self.patterns[pattern].len())
    }

    /// Whether one of the patterns at the indices `patterns` matches the path that led to `at`.
    pub fn is_match_among(&self, at: &Positions, patterns: std::ops::Range<usize>) -> bool {
        at.0.iter().any(|&(pattern, index)| {
            patterns.contains(&pattern) && index == self.patterns[pattern].len()
        })
    }

    /// Whether a pattern can match a path below the one that led to `at`.
    pub fn can_continue(&self, at: &Positions) -> bool {
        at.0.iter()
            .any(|&(pattern, index)| index < self.patterns[pattern].len())
    }

    /// Whether a pattern matches `path`, a relative path with `/` between its parts.
    pub fn matches(&self, path: &str) -> bool {
        let at = path
            .split('/')
            .fold(self.start(), |at, name| self.advance(&at, name));
        self.is_match(&at)
    }

    /// Adds position `index` of `pattern`, and the positions after each `**` from there, which
    /// can match no part at all.
    fn enter(&self, positions: &mut Vec<(usize, usize)>, pattern: usize, mut index: usize) {
        let parts = &self.patterns[pattern];
        positions.push((pattern, index));
        while let Some(Part::Any) = parts.get(index) {
            index += 1;
            positions.push((pattern, index));
        }
    }
}

/// Whether `name` matches `glob`, in which `*` matches any run of characters and `?` one
/// character, and every other character matches itself.
fn glob_matches(glob: &str, name: &str) -> bool {
    let (glob, name) = (glob.as_bytes(), name.as_bytes());
    let (mut g, mut n) = (0, 0);
    // After the last `*`: the glob index after it and the name index it is matched up to.
    let mut star = None;
    while n < name.len() {
        match glob.get(g) {
            Some(b'*') => {
                g += 1;
                star = Some((g, n));
            }
            Some(b'?') => {
                g += 1;
                n += char_length(name[n]);
            }
            // A literal character is compared byte by byte; UTF-8 is prefix-free, so a match
            // ends at a character boundary.
            Some(&byte) if byte == name[n] => {
                g += 1;
                n += 1;
            }
            _ => match &mut star {
                Some((after, matched)) => {
                    *matched += char_length(name[*matched]);
                    (g, n) = (*after, *matched);
                }
                None => return false,
            },
        }
    }
    glob[g..].iter().all(|&byte| byte == b'*')
}

/// The length of the UTF-8 character that starts with `byte`.
fn char_length(byte: u8) -> usize {
    match byte {
        0xF0.. => 4,
        0xE0.. => 3,
        0xC0.. => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_documented_example() {
        let patterns = Patterns::new(["docs/**/*.mara.md"]);
        for path in [
            "docs/a.mara.md",
            "docs/x/a.mara.md",
            "docs/x/y/.mara.md",
            "docs/.hidden/a.mara.md",
        ] {
            assert!(patterns.matches(path), "{path}");
        }
        for path in [
            "a.mara.md",
            "notes/a.mara.md",
            "docs/a.md",
            "docs/a.mara.md/b",
            "documents/a.mara.md",
            "docs",
        ] {
            assert!(!patterns.matches(path), "{path}");
        }
    }

    #[test]
    fn a_star_stays_within_one_part() {
        let patterns = Patterns::new(["*.mara.md", "docs/*/*.mara.md"]);
        assert!(patterns.matches("a.mara.md"));
        assert!(patterns.matches("docs/x/a.mara.md"));
        assert!(!patterns.matches("docs/a.mara.md"));
        assert!(!patterns.matches("docs/x/y/a.mara.md"));
        assert!(!patterns.matches("x/a.mara.md"));
    }

    #[test]
    fn double_star_matches_any_number_of_parts() {
        let leading = Patterns::new(["**/*.mara.md"]);
        assert!(leading.matches("a.mara.md"));
        assert!(leading.matches("x/y/z/a.mara.md"));
        let trailing = Patterns::new(["docs/**"]);
        assert!(trailing.matches("docs/a"));
        assert!(trailing.matches("docs/a/b/c"));
        let middle = Patterns::new(["a/**/**/b"]);
        assert!(middle.matches("a/b"));
        assert!(middle.matches("a/x/y/b"));
        assert!(!middle.matches("a/x/y/c"));
        // `**` inside a part is two stars.
        let inside = Patterns::new(["a**.md"]);
        assert!(inside.matches("abc.md"));
        assert!(!inside.matches("a/b.md"));
    }

    #[test]
    fn wildcards_match_characters_and_everything_else_is_literal() {
        let patterns = Patterns::new(["d?cs/[a]{b,c}.mara.md", "é?/*ü*.mara.md"]);
        assert!(patterns.matches("docs/[a]{b,c}.mara.md"));
        assert!(patterns.matches("dxcs/[a]{b,c}.mara.md"));
        assert!(!patterns.matches("docs/ab.mara.md"));
        assert!(patterns.matches("éñ/xüy.mara.md"));
        assert!(patterns.matches("éa/ü.mara.md"));
        assert!(!patterns.matches("é/ü.mara.md"));
        assert!(!patterns.matches("éñ/xu.mara.md"));
        let stars = Patterns::new(["*a*a*b"]);
        assert!(stars.matches("aaab"));
        assert!(stars.matches("xaxxaxb"));
        assert!(!stars.matches("aab "));
        assert!(!stars.matches("ab"));
    }

    #[test]
    fn tells_whether_a_directory_can_hold_a_match() {
        let patterns = Patterns::new(["docs/**/*.mara.md", "notes/*.mara.md"]);
        let at = |path: &str| {
            path.split('/')
                .fold(patterns.start(), |at, name| patterns.advance(&at, name))
        };
        assert!(patterns.can_continue(&patterns.start()));
        assert!(patterns.can_continue(&at("docs")));
        assert!(patterns.can_continue(&at("docs/x/y")));
        assert!(patterns.can_continue(&at("notes")));
        assert!(!patterns.can_continue(&at("notes/x")));
        assert!(!patterns.can_continue(&at("src")));
        assert!(!patterns.can_continue(&at(".git")));
        assert!(!Patterns::new([]).can_continue(&Patterns::new([]).start()));
    }

    #[test]
    fn a_path_without_wildcards_matches_itself() {
        let patterns = Patterns::new(["docs/index.mara.md"]);
        assert!(patterns.matches("docs/index.mara.md"));
        assert!(!patterns.matches("docs/index.mara.mdx"));
        assert!(!patterns.matches("docs/Index.mara.md"));
    }
}
