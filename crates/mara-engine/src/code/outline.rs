//! Markers at the grammar level (DES-CODE-MARKERS in `docs/code/index.mara.md`): from the
//! comments, attributes and declarations that a grammar found in a file, the markers of its
//! comments and the declaration that owns each marker and each line.

use std::ops::Range;

/// What a grammar found in one file, as byte ranges of its contents. The host parses; the
/// engine reads markers and owners from the result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Syntax {
    /// The comment nodes (`comment`, `line_comment`, `block_comment`), none inside another.
    pub comments: Vec<Range<usize>>,
    /// The attribute, decorator and annotation nodes.
    pub attributes: Vec<Range<usize>>,
    /// The declarations that the grammar's `tags.scm` defines.
    pub declarations: Vec<SyntaxDeclaration>,
}

/// A node that `tags.scm` captures as a definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDeclaration {
    /// The text of its `@name` capture.
    pub name: String,
    pub range: Range<usize>,
    /// Where the node's parent starts, `None` for the file's root.
    pub parent_start: Option<usize>,
}

/// What owns a marker or a line (DES-CODE-MARKERS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    /// At the basic level: the marker's line, or the line named.
    Line(usize),
    /// At the grammar level: the whole file.
    File,
    /// At the grammar level: the declaration at this index of the file's [`Outline`].
    Declaration(usize),
}

/// A declaration of a file at the grammar level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    /// 1-based.
    pub first_line: usize,
    pub last_line: usize,
    range: Range<usize>,
    /// The declaration that contains this one.
    parent: Option<usize>,
}

/// A comment or attribute node with its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Run {
    range: Range<usize>,
    owner: Owner,
}

/// The declarations of a file at the grammar level, and what tells the owner of each line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outline {
    /// By start, the outer one first where two start together.
    pub declarations: Vec<Declaration>,
    /// The comment and attribute nodes, by start.
    runs: Vec<Run>,
    /// Per line, the offset of its first byte that is no space or tab, else of its end.
    first_text: Vec<usize>,
    /// Where each declaration's parent node starts.
    parent_starts: Vec<Option<usize>>,
}

/// The offsets at which the lines of `contents` start; a last line without a line feed counts,
/// and empty contents have none.
pub(super) fn line_starts(contents: &[u8]) -> Vec<usize> {
    let mut starts = Vec::new();
    if !contents.is_empty() {
        starts.push(0);
    }
    for (at, &byte) in contents.iter().enumerate() {
        if byte == b'\n' && at + 1 < contents.len() {
            starts.push(at + 1);
        }
    }
    starts
}

impl Outline {
    /// The outline of `contents`, whose lines start at `starts`, from what a grammar found in
    /// it. Ranges outside the contents are cut to them, and empty ones are dropped.
    pub(super) fn new(contents: &[u8], starts: &[usize], syntax: &Syntax) -> Outline {
        let length = contents.len();
        let cut = |range: &Range<usize>| range.start.min(length)..range.end.min(length);
        let first_text = starts
            .iter()
            .map(|&start| {
                let line = &contents[start..];
                let end = line.iter().position(|&b| b == b'\n').unwrap_or(line.len());
                let text = line[..end].iter().position(|&b| !matches!(b, b' ' | b'\t'));
                start + text.unwrap_or(end)
            })
            .collect();
        let line_of = |at: usize| starts.partition_point(|&start| start <= at).max(1);

        let mut found: Vec<(Range<usize>, &SyntaxDeclaration)> = syntax
            .declarations
            .iter()
            .map(|declaration| (cut(&declaration.range), declaration))
            .filter(|(range, _)| !range.is_empty())
            .collect();
        found.sort_by_key(|(range, _)| (range.start, std::cmp::Reverse(range.end)));
        found.dedup_by(|later, earlier| later.0 == earlier.0);
        // Each declaration's parent, by a sweep over the nesting of their ranges.
        let mut declarations = Vec::with_capacity(found.len());
        let mut parent_starts = Vec::with_capacity(found.len());
        let mut open: Vec<usize> = Vec::new();
        for (range, declaration) in found {
            while open.last().is_some_and(|&index: &usize| {
                let open: &Declaration = &declarations[index];
                open.range.end <= range.start
            }) {
                open.pop();
            }
            let index = declarations.len();
            declarations.push(Declaration {
                name: declaration.name.clone(),
                first_line: line_of(range.start),
                last_line: line_of(range.end.saturating_sub(1).max(range.start)),
                parent: open.last().copied(),
                range,
            });
            parent_starts.push(declaration.parent_start);
            open.push(index);
        }

        let mut runs: Vec<Run> = (syntax.comments.iter())
            .chain(&syntax.attributes)
            .map(cut)
            .filter(|range| !range.is_empty())
            .map(|range| Run {
                range,
                owner: Owner::File,
            })
            .collect();
        runs.sort_by_key(|run| (run.range.start, std::cmp::Reverse(run.range.end)));
        runs.dedup_by(|later, earlier| later.range == earlier.range);
        let mut outline = Outline {
            declarations,
            runs,
            first_text,
            parent_starts,
        };
        // Where the text after each run continues: the first byte that is no blank space and
        // in no comment or attribute. Resolved from the last run back, each run once, so that
        // a long run of comments costs linear work.
        let count = outline.runs.len();
        let mut next = vec![0; count];
        for index in (0..count).rev() {
            let mut at = outline.runs[index].range.end;
            while contents.get(at).is_some_and(u8::is_ascii_whitespace) {
                at += 1;
            }
            let following = outline.runs.partition_point(|run| run.range.start < at);
            next[index] = match outline.runs.get(following) {
                Some(run) if run.range.start == at && following > index => next[following],
                _ => at,
            };
        }
        for index in 0..count {
            let range = outline.runs[index].range.clone();
            let line = line_of(range.start);
            let owner = if outline.first_text[line - 1] != range.start {
                // After code on its line: that code's owner.
                outline.containing(outline.first_text[line - 1])
            } else {
                let next = next[index];
                outline
                    .preceded(next, line_of(next))
                    .unwrap_or_else(|| outline.containing(range.start))
            };
            outline.runs[index].owner = owner;
        }
        outline
    }

    /// The declaration that starts at `at`, on line `line`, or whose parent node starts there
    /// and which starts on the same line, as a function in an `export` statement does.
    fn preceded(&self, at: usize, line: usize) -> Option<Owner> {
        let index = (self.declarations).partition_point(|declaration| declaration.range.start < at);
        let declaration = self.declarations.get(index)?;
        let in_parent = self.parent_starts[index] == Some(at) && declaration.first_line == line;
        (declaration.range.start == at || in_parent).then_some(Owner::Declaration(index))
    }

    /// The innermost declaration that contains the byte at `at`, else the file. Declarations
    /// nest, so it is the last one that starts at or before `at`, or one of its ancestors.
    fn containing(&self, at: usize) -> Owner {
        let after =
            (self.declarations).partition_point(|declaration| declaration.range.start <= at);
        let mut current = after.checked_sub(1);
        while let Some(index) = current {
            if self.declarations[index].range.end > at {
                return Owner::Declaration(index);
            }
            current = self.declarations[index].parent;
        }
        Owner::File
    }

    /// The owner of a marker in the comment that starts at `start`.
    pub(super) fn comment_owner(&self, start: usize) -> Owner {
        let index = self.runs.partition_point(|run| run.range.start < start);
        match self.runs.get(index) {
            Some(run) if run.range.start == start => run.owner,
            _ => self.containing(start),
        }
    }

    /// The owner of line `line`, 1-based, which the file has: that of the comment or attribute
    /// its text starts in, else the declaration its text starts, or starts the parent of on that
    /// line, else the innermost declaration that contains its text, else the file.
    pub fn owner(&self, line: usize) -> Owner {
        let Some(&at) = self.first_text.get(line.wrapping_sub(1)) else {
            return Owner::File;
        };
        let index = self.runs.partition_point(|run| run.range.start <= at);
        match index.checked_sub(1).map(|index| &self.runs[index]) {
            Some(run) if run.range.end > at => run.owner,
            _ => (self.preceded(at, line)).unwrap_or_else(|| self.containing(at)),
        }
    }
}
