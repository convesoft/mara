//! The lines of one HTML block: MID anchor lines (DES-MID-ANCHOR), and the end tag of a
//! `script`, `pre`, `style` or `textarea` block written other than in lowercase (DES-REGIONS).

use super::anchor_line;

/// The tag names of the blocks that end at a line holding their end tag, with that end tag.
const RAW: [(&str, &str); 4] = [
    ("script", "</script>"),
    ("pre", "</pre>"),
    ("style", "</style>"),
    ("textarea", "</textarea>"),
];

/// One HTML block, read line by line.
#[derive(Default)]
pub(super) struct HtmlBlock {
    started: bool,
    /// The lowercase end tag of a `script`, `pre`, `style` or `textarea` block, until a line
    /// holds it.
    end_tag: Option<&'static str>,
    /// A line held the end tag in another case: `pulldown-cmark` reads on as HTML anyway.
    swallowed: bool,
}

/// What one line of an HTML block holds.
pub(super) enum HtmlLine<'l> {
    Other,
    /// The line is a MID anchor; its MID.
    Anchor(&'l str),
    /// The line holds the block's end tag, which is this, in another case.
    EndTagCase(&'static str),
}

impl HtmlBlock {
    pub(super) fn line<'l>(&mut self, line: &'l str) -> HtmlLine<'l> {
        if !self.started {
            self.started = true;
            self.end_tag = raw_end_tag(line);
        }
        if self.swallowed {
            return HtmlLine::Other;
        }
        if let Some(end) = self.end_tag {
            if line.contains(end) {
                self.end_tag = None;
            } else if contains_ignoring_case(line, end) {
                self.swallowed = true;
                return HtmlLine::EndTagCase(end);
            }
        }
        anchor_line(line).map_or(HtmlLine::Other, HtmlLine::Anchor)
    }
}

/// The lowercase end tag of a block whose first line opens `script`, `pre`, `style` or
/// `textarea`, as CommonMark recognizes those blocks.
fn raw_end_tag(line: &str) -> Option<&'static str> {
    let rest = line.trim_start_matches(' ').strip_prefix('<')?;
    RAW.iter().find_map(|&(name, end)| {
        let opens = rest
            .as_bytes()
            .get(..name.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(name.as_bytes()));
        let after = rest.as_bytes().get(name.len()).copied();
        (opens && after.is_none_or(|byte| byte.is_ascii_whitespace() || byte == b'>'))
            .then_some(end)
    })
}

fn contains_ignoring_case(text: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    text.as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}
