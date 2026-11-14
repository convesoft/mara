//! The JSON form of results, which every host prints alike (REQ-CLI-MCP-PARITY in
//! `docs/hosts.mara.md`). Records are written once, so a page's size is the sum of its records'
//! sizes (DES-PAGING in `docs/retrieval.mara.md`).

use std::fmt::Write as _;

/// A JSON object being appended to a string. [`Object::end`] closes it.
pub struct Object<'o> {
    out: &'o mut String,
    first: bool,
}

impl<'o> Object<'o> {
    pub fn new(out: &'o mut String) -> Object<'o> {
        out.push('{');
        Object { out, first: true }
    }

    /// Starts the member `name` and returns the string its value is appended to.
    pub fn key(&mut self, name: &str) -> &mut String {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        string(self.out, name);
        self.out.push(':');
        self.out
    }

    pub fn string(&mut self, name: &str, value: &str) {
        string(self.key(name), value);
    }

    pub fn optional_string(&mut self, name: &str, value: Option<&str>) {
        match value {
            Some(value) => self.string(name, value),
            None => self.key(name).push_str("null"),
        }
    }

    pub fn number(&mut self, name: &str, value: u64) {
        write!(self.key(name), "{value}").expect("writing to a String cannot fail");
    }

    pub fn bool(&mut self, name: &str, value: bool) {
        self.key(name)
            .push_str(if value { "true" } else { "false" });
    }

    /// Appends `values`, each already in JSON form, as an array.
    pub fn array<'v>(&mut self, name: &str, values: impl IntoIterator<Item = &'v str>) {
        let out = self.key(name);
        out.push('[');
        for (index, value) in values.into_iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(value);
        }
        out.push(']');
    }

    pub fn end(self) {
        self.out.push('}');
    }
}

/// Appends `text` as a JSON string. Quotes, backslashes and control characters are escaped;
/// everything else is written as UTF-8.
pub fn string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                write!(out, "\\u{:04x}", c as u32).expect("writing to a String cannot fail")
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_what_json_requires_and_nothing_else() {
        let mut out = String::new();
        string(&mut out, "a\"b\\c\nd\re\tf\u{1}g\u{7f}é\u{2028}");
        assert_eq!(out, "\"a\\\"b\\\\c\\nd\\re\\tf\\u0001g\u{7f}é\u{2028}\"");
    }

    #[test]
    fn writes_objects_with_every_kind_of_member() {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.string("s", "x");
        object.optional_string("none", None);
        object.number("n", 7);
        object.bool("b", false);
        object.array("a", ["1", "{}"]);
        object.end();
        assert_eq!(out, r#"{"s":"x","none":null,"n":7,"b":false,"a":[1,{}]}"#);
    }
}
