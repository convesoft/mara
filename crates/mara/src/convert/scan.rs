//! Reading a 0.3.0 document: its `:::mara` blocks, and the Markdown around and inside them that
//! the conversion rewrites (DES-CONVERSION-MAPPING in `docs/migration.mara.md`).
//!
//! A block opens with `:::mara <flavour> <id>` at the start of a line outside fenced code, holds
//! `:key: value` metadata lines, then its body, and closes at a line `:::` outside fenced code in
//! the body, or at the end of the file.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// A line: where it starts, where its text ends and where it ends after its line ending (LF,
/// CRLF or a lone CR).
#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub start: usize,
    pub text_end: usize,
    pub end: usize,
}

pub fn lines(text: &str) -> Vec<Line> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let mut text_end = start;
        while text_end < bytes.len() && !matches!(bytes[text_end], b'\n' | b'\r') {
            text_end += 1;
        }
        let end = match bytes.get(text_end) {
            Some(b'\r') if bytes.get(text_end + 1) == Some(&b'\n') => text_end + 2,
            Some(_) => text_end + 1,
            None => text_end,
        };
        out.push(Line {
            start,
            text_end,
            end,
        });
        start = end;
    }
    out
}

/// A `:::mara` block.
#[derive(Debug)]
pub struct Block {
    /// From the start of the opening line to the end of the closing line, its line ending
    /// included.
    pub range: Range<usize>,
    /// The opening line's number.
    pub line: usize,
    pub flavour: String,
    pub id: String,
    pub metadata: Vec<Meta>,
    /// The body: the lines after the metadata, before the closing line.
    pub body: Range<usize>,
}

/// A `:key: value` line.
#[derive(Debug)]
pub struct Meta {
    pub key: String,
    /// Without the spaces around it.
    pub value: String,
    pub line: usize,
}

/// The blocks of a document, and the lines that start with `:::mara` but open none.
#[derive(Debug, Default)]
pub struct Scan {
    pub blocks: Vec<Block>,
    pub malformed: Vec<usize>,
}

/// Finds the blocks of `text`.
pub fn scan(text: &str) -> Scan {
    let mut lines = lines(text);
    // A byte order mark is no part of the first line.
    if let Some(first) = lines.first_mut()
        && text.starts_with('\u{feff}')
    {
        first.start = '\u{feff}'.len_utf8();
    }
    let line_text = |line: &Line| &text[line.start..line.text_end];
    let mut scan = Scan::default();
    let mut fence: Option<Fence> = None;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let content = line_text(&line);
        index += 1;
        if let Some(open) = &fence {
            if open.closes(content) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = Fence::opens(content) {
            fence = Some(open);
            continue;
        }
        let Some(rest) = content.strip_prefix(":::mara") else {
            continue;
        };
        if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
            continue;
        }
        let words: Vec<&str> = rest.split_whitespace().collect();
        let [flavour, id] = words[..] else {
            scan.malformed.push(index);
            continue;
        };
        let opening = index;
        let mut metadata = Vec::new();
        while let Some(line) = lines.get(index)
            && let Some(meta) = meta(line_text(line))
        {
            metadata.push(Meta {
                line: index + 1,
                ..meta
            });
            index += 1;
        }
        let body_start = lines.get(index).map_or(text.len(), |line| line.start);
        let mut body_fence: Option<Fence> = None;
        let mut close = None;
        while let Some(line) = lines.get(index) {
            let content = line_text(line);
            index += 1;
            match &body_fence {
                Some(open) if open.closes(content) => body_fence = None,
                Some(_) => {}
                None if content.trim_end_matches([' ', '\t']) == ":::" => {
                    close = Some(*line);
                    break;
                }
                None => body_fence = Fence::opens(content),
            }
        }
        let (body_end, end) = match close {
            Some(line) => (line.start, line.end),
            None => (text.len(), text.len()),
        };
        scan.blocks.push(Block {
            range: lines[opening - 1].start..end,
            line: opening,
            flavour: flavour.to_owned(),
            id: id.to_owned(),
            metadata,
            body: body_start..body_end,
        });
    }
    scan
}

/// A `:key: value` line, its line number unset.
fn meta(line: &str) -> Option<Meta> {
    let rest = line.strip_prefix(':')?;
    let colon = rest.find(':')?;
    let key = &rest[..colon];
    let mut bytes = key.bytes();
    let starts = bytes.next().is_some_and(|byte| byte.is_ascii_lowercase());
    if !starts
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return None;
    }
    let value = &rest[colon + 1..];
    if !value.is_empty() && !value.starts_with([' ', '\t']) {
        return None;
    }
    Some(Meta {
        key: key.to_owned(),
        value: value.trim_matches([' ', '\t']).to_owned(),
        line: 0,
    })
}

/// An open fenced code block.
struct Fence {
    mark: u8,
    length: usize,
}

impl Fence {
    /// The fence that `line` opens: up to three spaces, then three or more backticks or tildes;
    /// an info string after backticks holds none.
    fn opens(line: &str) -> Option<Fence> {
        let rest = indented(line)?;
        let mark = *rest
            .as_bytes()
            .first()
            .filter(|mark| matches!(mark, b'`' | b'~'))?;
        let length = rest.bytes().take_while(|&byte| byte == mark).count();
        let info = &rest[length..];
        (length >= 3 && !(mark == b'`' && info.contains('`'))).then_some(Fence { mark, length })
    }

    fn closes(&self, line: &str) -> bool {
        indented(line).is_some_and(|rest| {
            let length = rest.bytes().take_while(|&byte| byte == self.mark).count();
            length >= self.length && rest[length..].trim_matches([' ', '\t']).is_empty()
        })
    }
}

/// `line` without up to three leading spaces; `None` when it is indented more.
fn indented(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches(' ');
    (line.len() - rest.len() <= 3).then_some(rest)
}

/// What the conversion needs to know of a Markdown fragment, at offsets in it.
#[derive(Debug, Default)]
pub struct Markdown {
    /// Code spans, code blocks and HTML, where `[[…]]` is text.
    pub raw: Vec<Range<usize>>,
    /// Thematic breaks.
    pub rules: Vec<Range<usize>>,
    /// Setext headings, with their levels.
    pub setext: Vec<(Range<usize>, usize)>,
    /// The level of each heading, in order.
    pub headings: Vec<usize>,
}

pub fn markdown(text: &str) -> Markdown {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let mut out = Markdown::default();
    let mut skip_until: Option<usize> = None;
    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        if skip_until.is_some_and(|end| range.start < end) {
            continue;
        }
        skip_until = None;
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => {
                skip_until = Some(range.end);
                out.raw.push(range);
            }
            Event::Code(_) | Event::Html(_) | Event::InlineHtml(_) => out.raw.push(range),
            Event::Rule => out.rules.push(range),
            Event::Start(Tag::Heading { level, .. }) => {
                let level = level as usize;
                out.headings.push(level);
                if !text[range.clone()]
                    .trim_start_matches([' ', '\t'])
                    .starts_with('#')
                {
                    out.setext.push((range, level));
                }
            }
            Event::End(TagEnd::CodeBlock | TagEnd::HtmlBlock) => {}
            _ => {}
        }
    }
    out
}

/// An inline token of 0.3.0: `[[ID]]`, `[[MID]]` or `[[relation:target]]`.
#[derive(Debug)]
pub struct Token<'t> {
    pub range: Range<usize>,
    /// Between the brackets.
    pub inner: &'t str,
    /// The relation name of a typed token.
    pub relation: Option<&'t str>,
    /// The ID or MID that a mention names, or the target of a typed token.
    pub target: &'t str,
}

/// The tokens of `text` outside the `raw` ranges, whose opening brackets no backslash escapes.
pub fn tokens<'t>(text: &'t str, raw: &[Range<usize>]) -> Vec<Token<'t>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("[[") {
        let start = from + found;
        from = start + 2;
        if raw.iter().any(|range| range.contains(&start)) {
            continue;
        }
        let escapes = text[..start]
            .bytes()
            .rev()
            .take_while(|&byte| byte == b'\\')
            .count();
        if escapes % 2 == 1 {
            continue;
        }
        let Some(length) = text[start + 2..].find("]]") else {
            break;
        };
        let inner = &text[start + 2..start + 2 + length];
        if inner.is_empty() || inner.contains(['[', ']', '\n', '\r']) {
            continue;
        }
        let (relation, target) = match inner.split_once(':') {
            Some((name, target)) if is_relation_name(name) && !target.is_empty() => {
                (Some(name), target)
            }
            Some(_) => continue,
            None if is_id(inner) || mara_engine::document::is_mid(inner) => (None, inner),
            None => continue,
        };
        let end = start + 2 + length + 2;
        out.push(Token {
            range: start..end,
            inner,
            relation,
            target,
        });
        from = end;
    }
    out
}

fn is_relation_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Whether `text` has the form of an ID: `[A-Z][A-Z0-9]*(-[A-Z0-9]+)+` (DES-ITEM-IDS).
pub fn is_id(text: &str) -> bool {
    let mut parts = text.split('-');
    let first = parts.next().unwrap_or_default();
    let upper = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    };
    let rest: Vec<&str> = parts.collect();
    upper(first)
        && first.as_bytes()[0].is_ascii_uppercase()
        && !rest.is_empty()
        && rest.iter().all(|part| upper(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_blocks_outside_fences_and_closes_them_outside_body_fences() {
        let text = "# T\n\n```\n:::mara requirement REQ-IN-CODE\n```\n\n:::mara requirement REQ-A\r\n:mid: 01M1PXP2KGFTACAY742WPSWRK7\n:title: A\n\nBody\n```\n:::\n```\nmore\n:::\nAfter\n:::mara bad\n:::mara design DES-B\n:title: B\n";
        let scan = scan(text);
        assert_eq!(scan.blocks.len(), 2);
        let a = &scan.blocks[0];
        assert_eq!(
            (a.flavour.as_str(), a.id.as_str(), a.line),
            ("requirement", "REQ-A", 7)
        );
        assert_eq!(
            a.metadata.iter().map(|meta| meta.line).collect::<Vec<_>>(),
            [8, 9]
        );
        assert_eq!(&text[a.body.clone()], "\nBody\n```\n:::\n```\nmore\n");
        assert!(text[a.range.clone()].ends_with("more\n:::\n"));
        assert_eq!(scan.malformed, [18]);
        let b = &scan.blocks[1];
        assert_eq!(
            (b.id.as_str(), b.range.end, b.body.clone()),
            ("DES-B", text.len(), text.len()..text.len())
        );
    }

    #[test]
    fn opens_a_block_on_the_first_line_after_a_byte_order_mark() {
        let text = "\u{feff}:::mara requirement REQ-A\n:title: A\n\nBody\n:::\n";
        let scan = scan(text);
        assert_eq!(scan.blocks.len(), 1);
        assert_eq!(scan.blocks[0].range, 3..text.len());
    }

    #[test]
    fn finds_tokens_outside_code_and_escapes() {
        let text = "See [[REQ-A]], `[[REQ-B]]`, \\[[REQ-C]], [[satisfies:DES-D]], [[not a token]] and [[01M1PXP2KGFTACAY742WPSWRK7]].\n\n```\n[[REQ-E]]\n```\n";
        let raw = markdown(text).raw;
        let found: Vec<(&str, Option<&str>)> = tokens(text, &raw)
            .iter()
            .map(|token| (token.target, token.relation))
            .collect();
        assert_eq!(
            found,
            [
                ("REQ-A", None),
                ("DES-D", Some("satisfies")),
                ("01M1PXP2KGFTACAY742WPSWRK7", None)
            ]
        );
    }

    #[test]
    fn finds_rules_and_setext_headings() {
        let text = "Title\n=====\n\npara\n\n---\n\n***\n\nSub\n---\n\n# Atx\n";
        let found = markdown(text);
        assert_eq!(found.rules.len(), 2);
        let setext: Vec<(&str, usize)> = found
            .setext
            .iter()
            .map(|(range, level)| (&text[range.clone()], *level))
            .collect();
        assert_eq!(setext, [("Title\n=====\n", 1), ("Sub\n---\n", 2)]);
        assert_eq!(found.headings, [1, 2, 1]);
    }
}
