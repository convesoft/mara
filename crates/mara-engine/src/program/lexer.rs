//! Tokens of the admitted language, read as Soufflé 2.5 reads them with `--no-preprocessor`.
//!
//! Every byte is accounted for: what Soufflé could read differently is refused here, before any
//! grammar sees it. Strings, numbers, comments and directives follow the behaviour the reviews
//! observed (EVD-SOUFFLE-PROGRAM-REACH in `docs/rules-evaluation.mara.md`).

use super::Fault;

/// One token and the byte offset where it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub at: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A name: `[A-Za-z_][A-Za-z0-9_]*`, but not `_` alone.
    Ident(String),
    /// An integer literal's magnitude, at most 2^31; `decimal` tells `0x` and `0b` apart.
    Number {
        value: u32,
        decimal: bool,
    },
    /// A string literal, decoded.
    Str(String),
    /// A directive, `.` and a name Soufflé reserves (`decl` for `.decl`).
    Directive(&'static str),
    /// An outer doc comment (`///…` or `/**…*/`), which Soufflé reads as an annotation.
    Doc,
    Punct(Punct),
    End,
    /// Text the lexer refuses; the last token. The parser reports it when it gets there, so the
    /// first refusal in the text is the one reported.
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Punct {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,
    If,
    Semicolon,
    Bang,
    Pipe,
    Underscore,
    Plus,
    Minus,
    Star,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Subtype,
}

impl Punct {
    pub fn text(self) -> &'static str {
        match self {
            Punct::LParen => "(",
            Punct::RParen => ")",
            Punct::LBracket => "[",
            Punct::RBracket => "]",
            Punct::LBrace => "{",
            Punct::RBrace => "}",
            Punct::Comma => ",",
            Punct::Dot => ".",
            Punct::Colon => ":",
            Punct::If => ":-",
            Punct::Semicolon => ";",
            Punct::Bang => "!",
            Punct::Pipe => "|",
            Punct::Underscore => "_",
            Punct::Plus => "+",
            Punct::Minus => "-",
            Punct::Star => "*",
            Punct::Eq => "=",
            Punct::Ne => "!=",
            Punct::Lt => "<",
            Punct::Le => "<=",
            Punct::Gt => ">",
            Punct::Ge => ">=",
            Punct::Subtype => "<:",
        }
    }
}

/// The directive names Soufflé 2.5's scanner reserves after a `.`, whether admitted or not.
const DIRECTIVES: &[&str] = &[
    "decl",
    "functor",
    "input",
    "output",
    "printsize",
    "limitsize",
    "type",
    "comp",
    "init",
    "number_type",
    "symbol_type",
    "override",
    "pragma",
    "plan",
    "lattice",
    "include",
    "once",
];

/// The largest magnitude of an integer literal: that of −2^31.
pub const MAX_MAGNITUDE: u32 = 1 << 31;

/// Splits `text` into tokens, the last one `End`, or `Error` where the text is refused.
pub fn tokens(text: &str) -> Vec<Token> {
    let mut lexer = Lexer {
        bytes: text.as_bytes(),
        at: 0,
        tokens: Vec::new(),
    };
    if let Err(fault) = lexer.run() {
        lexer.tokens.push(Token {
            kind: Kind::Error(fault.message),
            at: fault.at as u32,
        });
    }
    lexer.tokens
}

struct Lexer<'a> {
    bytes: &'a [u8],
    at: usize,
    tokens: Vec<Token>,
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

impl Lexer<'_> {
    fn peek(&self, offset: usize) -> u8 {
        self.bytes.get(self.at + offset).copied().unwrap_or(0)
    }

    fn push(&mut self, kind: Kind, at: usize) {
        self.tokens.push(Token {
            kind,
            at: at as u32,
        });
    }

    fn run(&mut self) -> Result<(), Fault> {
        while self.at < self.bytes.len() {
            let start = self.at;
            let byte = self.bytes[start];
            match byte {
                b' ' | b'\t' | b'\n' | b'\r' => self.at += 1,
                b'/' if self.peek(1) == b'/' => self.line_comment()?,
                b'/' if self.peek(1) == b'*' => self.block_comment()?,
                b'"' => self.string()?,
                b'0'..=b'9' => self.number()?,
                b'.' => self.dot()?,
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.ident()?,
                _ => self.punct()?,
            }
        }
        self.push(Kind::End, self.bytes.len());
        Ok(())
    }

    fn line_comment(&mut self) -> Result<(), Fault> {
        let start = self.at;
        match self.peek(2) {
            b'/' => self.push(Kind::Doc, start),
            b'!' => return Err(inner_doc(start)),
            _ => {}
        }
        while self.at < self.bytes.len() && self.bytes[self.at] != b'\n' {
            check_control(self.bytes[self.at], self.at)?;
            self.at += 1;
        }
        Ok(())
    }

    fn block_comment(&mut self) -> Result<(), Fault> {
        let start = self.at;
        match (self.peek(2), self.peek(3)) {
            (b'*', b'/') => {}
            (b'*', _) => self.push(Kind::Doc, start),
            (b'!', _) => return Err(inner_doc(start)),
            _ => {}
        }
        let rest = &self.bytes[start + 2..];
        let end = rest.windows(2).position(|pair| pair == b"*/");
        let scanned = end.unwrap_or(rest.len());
        for (offset, &byte) in rest[..scanned].iter().enumerate() {
            if byte != b'\n' {
                check_control(byte, start + 2 + offset)?;
            }
        }
        match end {
            Some(end) => {
                self.at = start + 2 + end + 2;
                Ok(())
            }
            None => Err(Fault::at(start, "this comment has no `*/`")),
        }
    }

    fn string(&mut self) -> Result<(), Fault> {
        let start = self.at;
        self.at += 1;
        let mut value = String::new();
        loop {
            let Some(&byte) = self.bytes.get(self.at) else {
                return Err(Fault::at(start, "this string has no closing `\"`"));
            };
            match byte {
                b'"' => {
                    self.at += 1;
                    self.push(Kind::Str(value), start);
                    return Ok(());
                }
                b'\\' => {
                    let decoded = match self.peek(1) {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'\'' => '\'',
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        _ => {
                            return Err(Fault::at(
                                self.at,
                                "a string admits only the escapes `\\\"`, `\\\\`, `\\'`, `\\n`, `\\t` and `\\r`",
                            ));
                        }
                    };
                    value.push(decoded);
                    self.at += 2;
                }
                0..=0x1f | 0x7f => {
                    return Err(Fault::at(
                        self.at,
                        "a string cannot hold a control character or a line break: write `\\n` or `\\t`",
                    ));
                }
                _ => {
                    // The text is valid UTF-8, so a character's bytes run until the next one.
                    let width = utf8_width(byte);
                    let text = &self.bytes[self.at..self.at + width];
                    value.push_str(std::str::from_utf8(text).expect("valid UTF-8"));
                    self.at += width;
                }
            }
        }
    }

    fn number(&mut self) -> Result<(), Fault> {
        let start = self.at;
        let (radix, decimal, digits_from) = match (self.peek(0), self.peek(1)) {
            (b'0', b'x') => (16, false, start + 2),
            (b'0', b'b') => (2, false, start + 2),
            _ => (10, true, start),
        };
        self.at = digits_from;
        let mut value: u64 = 0;
        while let Some(digit) = (self.peek(0) as char).to_digit(radix) {
            value = (value * radix as u64 + digit as u64).min(u64::from(u32::MAX) + 1);
            self.at += 1;
        }
        if self.at == digits_from {
            return Err(Fault::at(start, "this number has no digits"));
        }
        if self.peek(0) == b'.' && self.peek(1).is_ascii_digit() {
            return Err(Fault::at(
                start,
                "a number with a `.` is a float or an IP address, which the admitted language leaves out",
            ));
        }
        if is_ident(self.peek(0)) {
            return Err(Fault::at(
                start,
                "a number cannot be followed by a letter: unsigned literals and suffixes are not admitted",
            ));
        }
        if value > u64::from(MAX_MAGNITUDE) {
            return Err(Fault::at(start, "this number is outside signed 32 bits"));
        }
        self.push(
            Kind::Number {
                value: value as u32,
                decimal,
            },
            start,
        );
        Ok(())
    }

    fn dot(&mut self) -> Result<(), Fault> {
        let start = self.at;
        let rest = &self.bytes[start + 1..];
        let directive = DIRECTIVES
            .iter()
            .filter(|name| rest.starts_with(name.as_bytes()))
            .max_by_key(|name| name.len());
        if let Some(name) = directive {
            self.at = start + 1 + name.len();
            if is_ident(self.peek(0)) {
                return Err(Fault::at(start, "unknown directive"));
            }
            if *name != "once" && !matches!(self.peek(0), b' ' | b'\t') {
                return Err(Fault::at(
                    start,
                    format!("Soufflé reads `.{name}` only before a space or a tab"),
                ));
            }
            self.push(Kind::Directive(name), start);
            return Ok(());
        }
        if self.peek(1).is_ascii_digit() {
            return Err(Fault::at(
                start,
                "a number with a `.` is a float, which the admitted language leaves out",
            ));
        }
        self.at += 1;
        self.push(Kind::Punct(Punct::Dot), start);
        Ok(())
    }

    fn ident(&mut self) -> Result<(), Fault> {
        let start = self.at;
        while is_ident(self.peek(0)) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).expect("ASCII");
        if text == "_" {
            self.push(Kind::Punct(Punct::Underscore), start);
            return Ok(());
        }
        if text.starts_with("__") {
            return Err(Fault::at(
                start,
                "a name cannot start with `__`: Soufflé reserves such names",
            ));
        }
        if text == "choice" && self.bytes[self.at..].starts_with(b"-domain") {
            return Err(Fault::at(start, "`choice-domain` is not admitted"));
        }
        self.push(Kind::Ident(text.to_owned()), start);
        Ok(())
    }

    fn punct(&mut self) -> Result<(), Fault> {
        let start = self.at;
        let (punct, width) = match (self.peek(0), self.peek(1)) {
            (b'(', _) => (Punct::LParen, 1),
            (b')', _) => (Punct::RParen, 1),
            (b'[', _) => (Punct::LBracket, 1),
            (b']', _) => (Punct::RBracket, 1),
            (b'{', _) => (Punct::LBrace, 1),
            (b'}', _) => (Punct::RBrace, 1),
            (b',', _) => (Punct::Comma, 1),
            (b':', b'-') => (Punct::If, 2),
            (b':', b':') => return Err(Fault::at(start, "`::` is not admitted")),
            (b':', _) => (Punct::Colon, 1),
            (b';', _) => (Punct::Semicolon, 1),
            (b'!', b'=') => (Punct::Ne, 2),
            (b'!', _) => (Punct::Bang, 1),
            (b'|', _) => (Punct::Pipe, 1),
            (b'+', _) => (Punct::Plus, 1),
            (b'-', b'>') => return Err(Fault::at(start, "`->` is not admitted")),
            (b'-', _) => (Punct::Minus, 1),
            (b'*', _) => (Punct::Star, 1),
            (b'=', _) => (Punct::Eq, 1),
            (b'<', b'=') => (Punct::Le, 2),
            (b'<', b':') => (Punct::Subtype, 2),
            (b'<', _) => (Punct::Lt, 1),
            (b'>', b'=') => (Punct::Ge, 2),
            (b'>', _) => (Punct::Gt, 1),
            (byte, _) => return Err(Fault::at(start, refused_character(byte))),
        };
        self.at += width;
        self.push(Kind::Punct(punct), start);
        Ok(())
    }
}

fn inner_doc(at: usize) -> Fault {
    Fault::at(
        at,
        "an inner doc comment (`//!` or `/*!`) is not admitted: write `//` or `/*`",
    )
}

fn refused_character(byte: u8) -> &'static str {
    match byte {
        b'#' => {
            "`#` is not admitted outside comments and strings: Soufflé reads it as a line marker"
        }
        b'@' => "`@` is not admitted: user-defined functors and annotations are left out",
        b'$' => "`$` is not admitted: counters and ADT constructors are left out",
        b'/' => "`/` is not admitted: division is left out",
        b'%' => "`%` is not admitted: the remainder is left out",
        b'^' => "`^` is not admitted: exponentiation is left out",
        0..=0x1f | 0x7f => "a control character is not admitted",
        0x80.. => "a character outside ASCII is admitted only in strings and comments",
        _ => "this character is not admitted",
    }
}

fn utf8_width(first: u8) -> usize {
    match first {
        0xf0.. => 4,
        0xe0.. => 3,
        0xc0.. => 2,
        _ => 1,
    }
}

/// Refuses a control character other than tab and carriage return in a comment. Soufflé's
/// scanner treats some of them, such as NUL, specially.
fn check_control(byte: u8, at: usize) -> Result<(), Fault> {
    if matches!(byte, 0..=0x08 | 0x0a..=0x0c | 0x0e..=0x1f | 0x7f) {
        return Err(Fault::at(at, "a control character is not admitted"));
    }
    Ok(())
}
