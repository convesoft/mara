//! Where a link writes the parts that a move or a rename rewrites: the path of its destination,
//! in the link itself or in the link reference definition it uses, and its text
//! (DES-MOVE-AND-RENAME). Each place is checked against what the parser read, so a link written
//! in a way these functions do not follow is refused instead of rewritten wrongly.

use std::ops::Range;

/// The bytes of the path of the destination that the link at `span` of `contents` writes
/// itself, `[text](<path>#<fragment>)` or `[text](<<path>#<fragment>>)`, where `path` is the path
/// the parser read. `None` when no such bytes read as `path`.
pub(super) fn inline_path(
    contents: &str,
    span: &Range<usize>,
    path: &str,
    fragment: &str,
) -> Option<Range<usize>> {
    let source = &contents[span.clone()];
    let needle = format!("#{fragment}");
    // The destination comes last, before an optional title that might hold the same text.
    for (at, _) in source.rmatch_indices(&needle) {
        let after = source[at + needle.len()..].bytes().next();
        let pointy = match after {
            Some(b'>') => true,
            Some(b')' | b' ' | b'\t' | b'\n' | b'\r') => false,
            _ => continue,
        };
        let Some(start) = destination_start(&source.as_bytes()[..at], pointy) else {
            continue;
        };
        if unescape(&source[start..at]) == path {
            return Some(span.start + start..span.start + at);
        }
    }
    None
}

/// Where a destination ending at the end of `before` starts: after the `<` of a pointy one, and
/// otherwise after the space, line break or unbalanced `(` before it.
fn destination_start(before: &[u8], pointy: bool) -> Option<usize> {
    let mut depth = 0usize;
    let mut at = before.len();
    while at > 0 {
        let byte = before[at - 1];
        let escaped = escaped(before, at - 1);
        match byte {
            b'<' if pointy && !escaped => return Some(at),
            b'\n' | b'\r' if pointy => return None,
            b' ' | b'\t' | b'\n' | b'\r' if !pointy => return Some(at),
            b'(' if !pointy && !escaped => match depth.checked_sub(1) {
                Some(outer) => depth = outer,
                None => return Some(at),
            },
            b')' if !pointy && !escaped => depth += 1,
            _ => {}
        }
        at -= 1;
    }
    None
}

/// Whether the byte at `at` follows an odd number of backslashes.
fn escaped(bytes: &[u8], at: usize) -> bool {
    bytes[..at]
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'\\')
        .count()
        % 2
        == 1
}

/// The bytes of the path of the destination of the link reference definition at `definition`
/// of `contents`, `[label]: <path>#<fragment>`, where `path` is the path the parser read.
pub(super) fn definition_path(
    contents: &str,
    definition: &Range<usize>,
    path: &str,
    fragment: &str,
) -> Option<Range<usize>> {
    let source = &contents[definition.clone()];
    let bytes = source.as_bytes();
    let close = label_end(bytes)?;
    let rest = source[close + 1..].strip_prefix(':')?;
    let start = source.len() - rest.trim_start().len();
    let (start, end) = match bytes.get(start) {
        Some(b'<') => {
            let length = source[start + 1..].find('>')?;
            (start + 1, start + 1 + length)
        }
        _ => {
            let length = source[start..]
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(source.len() - start);
            (start, start + length)
        }
    };
    let written = source[start..end]
        .strip_suffix(fragment)?
        .strip_suffix('#')?;
    let path_end = start + written.len();
    (unescape(written) == path).then_some(definition.start + start..definition.start + path_end)
}

/// `text` with each backslash escape of ASCII punctuation read as the character, as CommonMark
/// reads a link destination.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match chars.peek() {
            Some(&next) if c == '\\' && next.is_ascii_punctuation() => {
                out.push(next);
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
}

/// The index of the `]` that closes the bracket opening `bytes`, past backslash escapes, code
/// spans and nested brackets.
fn label_end(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let mut depth = 0usize;
    let mut at = 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => {
                at += 2;
                continue;
            }
            b'`' => {
                let run = bytes[at..].iter().take_while(|&&byte| byte == b'`').count();
                at += run;
                // A code span ends at the next run of as many backticks; without one, the
                // backticks are text.
                let mut scan = at;
                while scan < bytes.len() {
                    if bytes[scan] == b'`' {
                        let next = bytes[scan..]
                            .iter()
                            .take_while(|&&byte| byte == b'`')
                            .count();
                        if next == run {
                            at = scan + next;
                            break;
                        }
                        scan += next;
                    } else {
                        scan += 1;
                    }
                }
                continue;
            }
            b'[' => depth += 1,
            b']' => match depth.checked_sub(1) {
                Some(outer) => depth = outer,
                None => return Some(at),
            },
            _ => {}
        }
        at += 1;
    }
    None
}

/// The replacement that gives the link at `span` of `contents`, whose text reads `old`, the text
/// `new`: the bytes to replace and their new text. Within the brackets, a single `old` written
/// as it reads is replaced, so emphasis around it stays; otherwise the whole text is. A
/// collapsed `[label][]` or shortcut `[label]` reference, whose text is also its label, becomes
/// the full reference `[new][label]`, which uses the same definition. `None` for a link that does
/// not start with its bracketed text, such as an autolink.
pub(super) fn relabel(
    contents: &str,
    span: &Range<usize>,
    old: &str,
    new: &str,
) -> Option<(Range<usize>, String)> {
    let source = &contents[span.clone()];
    let close = label_end(source.as_bytes())?;
    let label = &source[1..close];
    let rest = &source[close + 1..];
    if rest.starts_with('(') || (rest.starts_with('[') && !rest.starts_with("[]")) {
        let mut found = label.match_indices(old);
        let within = match (found.next(), found.next()) {
            (Some((at, _)), None) if !old.is_empty() => 1 + at..1 + at + old.len(),
            _ => 1..close,
        };
        return Some((
            span.start + within.start..span.start + within.end,
            new.to_owned(),
        ));
    }
    Some((span.clone(), format!("[{new}][{label}]")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = "01M7D8DZ2663WP3A037SR2HJH2";

    fn inline(link: &str, path: &str) -> Option<String> {
        let text = format!("See {link}.");
        let span = 4..4 + link.len();
        inline_path(&text, &span, path, M).map(|range| text[range].to_owned())
    }

    #[test]
    fn finds_the_path_that_an_inline_link_writes() {
        for (link, path, written) in [
            (format!("[a](#{M})"), "", ""),
            (format!("[a](b.mara.md#{M})"), "b.mara.md", "b.mara.md"),
            (
                format!("[a](../x/b.mara.md#{M} \"t\")"),
                "../x/b.mara.md",
                "../x/b.mara.md",
            ),
            (format!("[a](  b.mara.md#{M}\n)"), "b.mara.md", "b.mara.md"),
            (
                format!("[a](<my b.mara.md#{M}>)"),
                "my b.mara.md",
                "my b.mara.md",
            ),
            (
                format!("[a](d(1)/b.mara.md#{M})"),
                "d(1)/b.mara.md",
                "d(1)/b.mara.md",
            ),
            (
                format!("[a](d\\(/b.mara.md#{M})"),
                "d(/b.mara.md",
                "d\\(/b.mara.md",
            ),
            (
                format!("[x\\](y)](b.mara.md#{M})"),
                "b.mara.md",
                "b.mara.md",
            ),
            (
                format!("[a](b.mara.md#{M} \"see #{M})\")"),
                "b.mara.md",
                "b.mara.md",
            ),
        ] {
            assert_eq!(inline(&link, path).as_deref(), Some(written), "{link}");
        }
        // A reference link writes no destination, and what is found must read as the path.
        assert_eq!(inline("[a][r]", ""), None);
        assert_eq!(inline(&format!("[a](b&amp;c#{M})"), "b&c"), None);
    }

    #[test]
    fn finds_the_path_that_a_definition_writes() {
        for (definition, path, written) in [
            (format!("[r]: #{M}"), "", ""),
            (format!("[r]:b.mara.md#{M}"), "b.mara.md", "b.mara.md"),
            (
                format!("[r\\]x]:\n  b.mara.md#{M} \"t\""),
                "b.mara.md",
                "b.mara.md",
            ),
            (
                format!("[r]: <a b.mara.md#{M}> 't'"),
                "a b.mara.md",
                "a b.mara.md",
            ),
        ] {
            let span = 0..definition.len();
            let found = definition_path(&definition, &span, path, M);
            assert_eq!(
                found.map(|range| &definition[range]),
                Some(written),
                "{definition}"
            );
        }
        let definition = format!("[r]: other#{M}");
        assert_eq!(
            definition_path(&definition, &(0..definition.len()), "b", M),
            None
        );
    }

    #[test]
    fn relabels_each_form_of_link() {
        for (link, new) in [
            (format!("[REQ-OLD](#{M})"), format!("[REQ-NEW](#{M})")),
            (
                format!("[**REQ-OLD**](#{M})"),
                format!("[**REQ-NEW**](#{M})"),
            ),
            (format!("[REQ\\-OLD](#{M})"), format!("[REQ-NEW](#{M})")),
            ("[REQ-OLD][r]".to_owned(), "[REQ-NEW][r]".to_owned()),
            ("[REQ-OLD][]".to_owned(), "[REQ-NEW][REQ-OLD]".to_owned()),
            ("[REQ-OLD]".to_owned(), "[REQ-NEW][REQ-OLD]".to_owned()),
            (
                format!("[`a]`REQ-OLD](#{M})"),
                format!("[`a]`REQ-NEW](#{M})"),
            ),
        ] {
            let span = 0..link.len();
            let (range, text) = relabel(&link, &span, "REQ-OLD", "REQ-NEW").unwrap();
            let mut out = link.clone();
            out.replace_range(range, &text);
            assert_eq!(out, new, "{link}");
        }
        assert_eq!(relabel("<https://x>", &(0..11), "a", "b"), None);
    }
}
