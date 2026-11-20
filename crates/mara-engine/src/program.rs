//! Mara's parser for the admitted program language (DES-PROGRAM-PROFILE in
//! `docs/rules/index.mara.md`).
//!
//! A program is admitted only if this module understands all of it: its files, read as Soufflé
//! reads them, follow the grammar of the admitted language, its types resolve to `symbol`,
//! `number` and records of them, and every relation that crosses the boundary holds only
//! symbols and numbers. Anything else is refused at the token where the reading stopped, like a
//! compile error.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;

mod lexer;
mod parser;
mod resolve;
mod syntax;

#[cfg(test)]
mod tests;

/// The path of Mara's declarations file as programs include it, and as refusals in it name it.
pub const DECLARATIONS: &str = "mara.dl";

/// The text a program may read: Mara's declarations and every declared program file.
#[derive(Clone, Copy, Debug)]
pub struct Files<'a> {
    /// The contents of `mara.dl`.
    pub declarations: &'a [u8],
    /// Every file that a program pattern declares, by project path.
    pub programs: &'a BTreeMap<String, Vec<u8>>,
}

/// A program that Mara admits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// The files it reads: the declared program first, then each included file once.
    /// `DECLARATIONS` stands for Mara's declarations.
    pub files: Vec<String>,
    /// Every relation it declares, by name, with components' relations under their qualified
    /// names (`c.r`).
    pub relations: BTreeMap<String, Relation>,
    /// Every `.include` that names one of its files, in the order they were read.
    pub includes: Vec<Include>,
    /// Every fact the program states, a clause without a body, with components' facts under
    /// their qualified relation names.
    pub facts: Vec<Fact>,
    /// The text of each file and the offsets where its lines start.
    texts: Vec<(String, Vec<usize>)>,
}

/// One `.include` directive of a program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Include {
    /// The file that holds it, an index into `Program::files`.
    pub file: usize,
    /// The byte offset of its quoted path, which holds only the characters the parser admits
    /// in a path, so its text is the path in quotes.
    pub at: usize,
    /// The file it includes, an index into `Program::files`.
    pub target: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    pub columns: Vec<Column>,
    pub inline: bool,
    /// Where it is declared.
    pub place: Place,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub kind: ColumnKind,
    /// Where its type is named.
    pub place: Place,
}

/// A fact of a program: the relation of one head of a clause without a body, and where that
/// head starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub relation: String,
    pub place: Place,
}

/// A byte offset in one of a program's files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// An index into `Program::files`.
    pub file: usize,
    pub offset: usize,
}

/// What a column's type resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnKind {
    Symbol,
    Number,
    Record,
}

/// A place in a program file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// The project path, or `DECLARATIONS`.
    pub path: String,
    /// 1-based.
    pub line: usize,
    /// 1-based, in characters.
    pub column: usize,
    /// The text of the line, without its line break.
    pub source_line: String,
}

/// Why a program is refused, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub message: String,
    pub location: Location,
}

impl Location {
    /// The source line, then a caret under the column on the next line: how a compile error's
    /// message ends (DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`).
    pub fn excerpt(&self) -> String {
        let caret: String = self
            .source_line
            .chars()
            .take(self.column - 1)
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .chain(['^'])
            .collect();
        format!("{}\n{caret}", self.source_line)
    }
}

impl Refusal {
    /// The error diagnostic of a refusal: a compile error's form, its message ending with the
    /// source line and a caret under the column (DES-RULE-EXECUTION in
    /// `docs/rules/evaluation.mara.md`).
    pub fn diagnostic(&self) -> Diagnostic {
        let location = &self.location;
        Diagnostic::error(
            "program-refused",
            location.path.clone(),
            location.line,
            format!("program refused: {}\n{}", self.message, location.excerpt()),
        )
    }
}

/// Why a relation cannot cross the boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundaryError {
    /// The program declares no relation of that name.
    Undeclared,
    /// It declares one that cannot cross.
    Refused(Refusal),
}

impl Program {
    /// The text of the file at `file`, an index into `files`.
    pub fn text(&self, file: usize) -> &str {
        &self.texts[file].0
    }

    /// The values of `fact` as its text writes them, when each argument is a literal: a
    /// string, decoded, or an integer, with an optional `-`. `None` for any other argument,
    /// such as a variable of a component's parameter, a sum or a record.
    pub fn fact_values(&self, fact: &Fact) -> Option<Vec<crate::facts::Value>> {
        use crate::facts::Value;
        use lexer::{Kind, Punct};
        let text = &self.text(fact.place.file)[fact.place.offset..];
        let tokens = lexer::tokens(text);
        let mut kinds = tokens.iter().map(|token| &token.kind);
        // The relation's name, qualified or not, then its arguments.
        loop {
            match kinds.next()? {
                Kind::Ident(_) | Kind::Punct(Punct::Dot) => {}
                Kind::Punct(Punct::LParen) => break,
                _ => return None,
            }
        }
        let mut values = Vec::new();
        loop {
            let negative = matches!(kinds.clone().next(), Some(Kind::Punct(Punct::Minus)));
            if negative {
                kinds.next();
            }
            let value = match kinds.next()? {
                Kind::Str(text) if !negative => Value::Symbol(text.clone()),
                Kind::Number { value, .. } => {
                    let magnitude = i64::from(*value);
                    let number = if negative { -magnitude } else { magnitude };
                    Value::Number(i32::try_from(number).ok()?)
                }
                Kind::Punct(Punct::RParen) if values.is_empty() && !negative => {
                    return Some(values);
                }
                _ => return None,
            };
            values.push(value);
            match kinds.next()? {
                Kind::Punct(Punct::Comma) => {}
                Kind::Punct(Punct::RParen) => return Some(values),
                _ => return None,
            }
        }
    }

    /// The value of every string literal in the program's files, as the lexer decodes its
    /// escapes. Soufflé decodes them the same way and prints them so, without escapes.
    pub fn string_constants(&self) -> BTreeSet<String> {
        let mut constants = BTreeSet::new();
        for (text, _) in &self.texts {
            for token in lexer::tokens(text) {
                if let lexer::Kind::Str(value) = token.kind {
                    constants.insert(value);
                }
            }
        }
        constants
    }

    /// The location of byte `offset` of the file at index `file`.
    pub fn locate(&self, file: usize, offset: usize) -> Location {
        let (text, lines) = &self.texts[file];
        locate(&self.files[file], text, lines, offset)
    }

    /// The start of line `line`, 1-based, of the file at index `file`; `None` past its end.
    pub fn line_start(&self, file: usize, line: usize) -> Option<usize> {
        self.texts[file].1.get(line.checked_sub(1)?).copied()
    }

    /// The location of `place`.
    pub fn location(&self, place: Place) -> Location {
        let (text, lines) = &self.texts[place.file];
        locate(&self.files[place.file], text, lines, place.offset)
    }

    /// The relation `name` as one that crosses the boundary: a fact relation, a supplied input or
    /// a consumed relation. It holds only symbols and numbers, also through subtypes, unions and
    /// component parameters, and is not `inline`, which Soufflé cannot read or write.
    pub fn boundary(&self, name: &str) -> Result<&Relation, BoundaryError> {
        let relation = self.relations.get(name).ok_or(BoundaryError::Undeclared)?;
        if let Some(column) = relation
            .columns
            .iter()
            .find(|column| column.kind == ColumnKind::Record)
        {
            return Err(BoundaryError::Refused(Refusal {
                message: format!(
                    "`{name}` crosses the boundary, so its column `{}` must hold symbols or numbers, not records",
                    column.name
                ),
                location: self.location(column.place),
            }));
        }
        if relation.inline {
            return Err(BoundaryError::Refused(Refusal {
                message: format!("`{name}` crosses the boundary, so it cannot be `inline`"),
                location: self.location(relation.place),
            }));
        }
        Ok(relation)
    }
}

/// Parses the declared program at `path` with every file it includes, resolves its types and
/// admits it, or refuses it at the place the reading stopped. Every relation that `mara.dl`
/// declares is checked as one that crosses the boundary.
pub fn admit(path: &str, files: &Files) -> Result<Program, Refusal> {
    let program = resolve::resolve(path, files)?;
    for (name, relation) in &program.relations {
        if program.files[relation.place.file] == DECLARATIONS
            && let Err(BoundaryError::Refused(refusal)) = program.boundary(name)
        {
            return Err(refusal);
        }
    }
    Ok(program)
}

/// A refusal inside one file, at a byte offset.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fault {
    at: usize,
    message: String,
}

impl Fault {
    fn at(at: usize, message: impl Into<String>) -> Fault {
        Fault {
            at,
            message: message.into(),
        }
    }
}

/// The offsets where the lines of `text` start.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(at, _)| at + 1))
        .collect()
}

/// The location of byte offset `at` of `text`, a file at `path` whose lines start at `lines`.
fn locate(path: &str, text: &str, lines: &[usize], at: usize) -> Location {
    let at = at.min(text.len());
    let line = lines.partition_point(|&start| start <= at);
    let start = lines[line - 1];
    let end = text[start..]
        .find('\n')
        .map_or(text.len(), |newline| start + newline);
    let source_line = &text[start..end];
    Location {
        path: path.to_owned(),
        line,
        column: text[start..at].chars().count() + 1,
        source_line: source_line
            .strip_suffix('\r')
            .unwrap_or(source_line)
            .to_owned(),
    }
}
