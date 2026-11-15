//! A YAML file as a tree whose nodes keep their lines, read by `granit-parser`, the parser that
//! 0.3.0 read its schema with.

use std::collections::HashMap;

use granit_parser::{Event, Parser};

/// A node and the line where it starts.
#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub line: usize,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub(crate) enum Value {
    Scalar(String),
    Sequence(Vec<Node>),
    /// Each key with its line and its value, in source order.
    Mapping(Vec<(String, usize, Node)>),
}

/// The YAML merge key, which the conversion does not apply.
pub(crate) const MERGE: &str = "<<";

impl Node {
    /// The value of `key` in a mapping.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entries()
            .find(|(name, _, _)| name == key)
            .map(|(_, _, node)| node)
    }

    /// The entries of a mapping but its merge keys; none for any other node.
    pub fn entries(&self) -> impl Iterator<Item = &(String, usize, Node)> {
        let entries = match &self.value {
            Value::Mapping(entries) => entries.as_slice(),
            _ => &[],
        };
        entries.iter().filter(|(name, _, _)| name != MERGE)
    }

    /// The line of each merge key in the node and below it, in order.
    pub fn merges(&self) -> Vec<usize> {
        let mut lines = Vec::new();
        let mut pending = vec![self];
        while let Some(node) = pending.pop() {
            match &node.value {
                Value::Scalar(_) => {}
                Value::Sequence(items) => pending.extend(items.iter().rev()),
                Value::Mapping(entries) => {
                    for (name, line, _) in entries {
                        if name == MERGE {
                            lines.push(*line);
                        }
                    }
                    pending.extend(entries.iter().rev().map(|(_, _, node)| node));
                }
            }
        }
        lines.sort_unstable();
        lines
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::Scalar(text) => Some(text),
            _ => None,
        }
    }

    /// The boolean of a scalar as 0.3.0 read it: the YAML 1.1 spellings `true`, `yes`, `y` and
    /// `on`, and `false`, `no`, `n` and `off`, in any case.
    pub fn boolean(&self) -> Option<bool> {
        let text = self.as_str()?.trim();
        let is = |spellings: [&str; 4]| spellings.iter().any(|s| text.eq_ignore_ascii_case(s));
        if is(["true", "yes", "y", "on"]) {
            Some(true)
        } else if is(["false", "no", "n", "off"]) {
            Some(false)
        } else {
            None
        }
    }

    /// Whether the node holds anything: a scalar that is not empty or null, or a collection
    /// with an entry.
    pub fn is_filled(&self) -> bool {
        match &self.value {
            Value::Scalar(text) => !matches!(text.trim(), "" | "~" | "null"),
            Value::Sequence(items) => !items.is_empty(),
            Value::Mapping(entries) => !entries.is_empty(),
        }
    }
}

/// Parses the first document of `text`. An error has its line and the parser's reason; a key
/// that is not a scalar or that repeats is one too.
pub(crate) fn parse(text: &str) -> Result<Node, (usize, String)> {
    enum Frame {
        Sequence(usize, usize, Vec<Node>),
        Mapping(
            usize,
            usize,
            Vec<(String, usize, Node)>,
            Option<(String, usize)>,
        ),
    }
    let mut stack: Vec<Frame> = Vec::new();
    let mut anchors: HashMap<usize, Node> = HashMap::new();
    let mut root: Option<Node> = None;
    for event in Parser::new_from_str(text) {
        let (event, span) = event.map_err(|error| (error.marker().line(), error.to_string()))?;
        let line = span.start.line();
        let (node, anchor) = match event {
            Event::Scalar(text, _, anchor, _) => (
                Node {
                    line,
                    value: Value::Scalar(text.into_owned()),
                },
                anchor,
            ),
            Event::Alias(anchor) => match anchors.get(&anchor) {
                Some(node) => (node.clone(), 0),
                None => return Err((line, "an alias names no anchor".to_owned())),
            },
            Event::SequenceStart(_, anchor, _) => {
                stack.push(Frame::Sequence(line, anchor, Vec::new()));
                continue;
            }
            Event::MappingStart(_, anchor, _) => {
                stack.push(Frame::Mapping(line, anchor, Vec::new(), None));
                continue;
            }
            Event::SequenceEnd | Event::MappingEnd => match stack.pop() {
                Some(Frame::Sequence(line, anchor, items)) => (
                    Node {
                        line,
                        value: Value::Sequence(items),
                    },
                    anchor,
                ),
                Some(Frame::Mapping(line, anchor, entries, _)) => (
                    Node {
                        line,
                        value: Value::Mapping(entries),
                    },
                    anchor,
                ),
                None => continue,
            },
            Event::DocumentEnd if stack.is_empty() && root.is_some() => break,
            _ => continue,
        };
        if anchor != 0 {
            anchors.insert(anchor, node.clone());
        }
        match stack.last_mut() {
            None => {
                root.get_or_insert(node);
            }
            Some(Frame::Sequence(_, _, items)) => items.push(node),
            Some(Frame::Mapping(_, _, entries, key)) => match key.take() {
                None => match node.value {
                    Value::Scalar(name) => {
                        if entries.iter().any(|(other, _, _)| *other == name) {
                            return Err((node.line, format!("the key `{name}` repeats")));
                        }
                        *key = Some((name, node.line));
                    }
                    _ => return Err((node.line, "a key must be a scalar".to_owned())),
                },
                Some((name, line)) => entries.push((name, line, node)),
            },
        }
    }
    root.ok_or((1, "the file holds no YAML document".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(node: &Node) -> Vec<&str> {
        match &node.value {
            Value::Sequence(items) => items.iter().map(|item| item.as_str().unwrap()).collect(),
            _ => panic!("not a sequence"),
        }
    }

    #[test]
    fn keeps_lines_order_flow_collections_and_anchors() {
        let text = "a: 1\nb:\n  - x\n  - \"y z\"\nc: {d: [e, f], g: true}\nh: &h one\ni: *h\n";
        let node = parse(text).unwrap();
        let keys: Vec<(&str, usize)> = node
            .entries()
            .map(|(key, line, _)| (key.as_str(), *line))
            .collect();
        assert_eq!(keys, [("a", 1), ("b", 2), ("c", 5), ("h", 6), ("i", 7)]);
        assert_eq!(strings(node.get("b").unwrap()), ["x", "y z"]);
        let c = node.get("c").unwrap();
        assert_eq!(strings(c.get("d").unwrap()), ["e", "f"]);
        assert_eq!(c.get("g").unwrap().boolean(), Some(true));
        assert_eq!(node.get("i").unwrap().as_str(), Some("one"));
    }

    #[test]
    fn reads_the_booleans_that_0_3_0_read_and_finds_merge_keys() {
        let node =
            parse("a: True\nb: NO\nc: on\nd: maybe\ne: &e {f: 1}\ng: {<<: *e, h: 2}\n").unwrap();
        let flags: Vec<Option<bool>> = ["a", "b", "c", "d"]
            .iter()
            .map(|key| node.get(key).unwrap().boolean())
            .collect();
        assert_eq!(flags, [Some(true), Some(false), Some(true), None]);
        assert_eq!(node.merges(), [6]);
        assert_eq!(node.get("g").unwrap().entries().count(), 1);
    }

    #[test]
    fn refuses_repeated_keys_and_syntax_errors_with_their_lines() {
        assert_eq!(parse("a: 1\nb: 2\na: 3\n").unwrap_err().0, 3);
        assert_eq!(parse("a: [1, 2\nb: 3\n").unwrap_err().0, 2);
    }
}
