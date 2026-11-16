//! The grammar of the admitted language (DES-PROGRAM-PROFILE in `docs/rules/index.mara.md`).
//!
//! A recursive descent over the tokens of one file. Whatever the grammar does not name is
//! refused at the token where it stopped; no construct is admitted by default.

use super::Fault;
use super::lexer::{self, Kind, MAX_MAGNITUDE, Punct, Token};
use super::syntax::*;

/// How deep terms and expressions may nest. Deeper text is refused before it can exhaust the
/// stack.
const MAX_DEPTH: u32 = 100;

/// Every word Soufflé 2.5's scanner reads as a keyword. None of them can name anything.
const KEYWORDS: &[&str] = &[
    "as",
    "autoinc",
    "band",
    "bnot",
    "bor",
    "brie",
    "bshl",
    "bshr",
    "bshru",
    "btree",
    "btree_delete",
    "bxor",
    "cat",
    "contains",
    "count",
    "debug_delta",
    "eqrel",
    "false",
    "inline",
    "input",
    "land",
    "lnot",
    "lor",
    "lxor",
    "magic",
    "match",
    "max",
    "mean",
    "min",
    "nil",
    "no_inline",
    "no_magic",
    "ord",
    "output",
    "overridable",
    "printsize",
    "range",
    "recursive_iteration_cnt",
    "stateful",
    "strlen",
    "substr",
    "sum",
    "to_float",
    "to_number",
    "to_string",
    "to_unsigned",
    "true",
];

/// Names of Soufflé's own types, which name nothing else.
const TYPE_WORDS: &[&str] = &["symbol", "number", "float", "unsigned"];

pub fn parse(text: &str) -> Result<File, Fault> {
    let tokens = lexer::tokens(text);
    let mut parser = Parser {
        text,
        tokens,
        next: 0,
        depth: 0,
        term_start: usize::MAX,
        aggregate_bodies: 0,
    };
    parser.file()
}

struct Parser<'a> {
    text: &'a str,
    tokens: Vec<Token>,
    next: usize,
    depth: u32,
    /// The token where the current term starts: only there can a name with `.` start an atom.
    /// Elsewhere a `.` after a name ends the clause, as in Soufflé's grammar.
    term_start: usize,
    /// How many aggregate bodies in braces enclose the current token.
    aggregate_bodies: u32,
}

/// A term, or an expression that is not yet one: `(x + 1)` is a value only once its group closes
/// around it alone.
enum Piece {
    Term(Term),
    Bare(Expr, u32),
}

/// What a primary can be before its context decides.
enum Operand {
    Value(Expr),
    Atom(Atom),
    Group(Body),
    Boolean,
    Contains(Expr, Expr),
}

fn binary(kind: &Kind) -> Option<u8> {
    match kind {
        Kind::Ident(word) => match word.as_str() {
            "lor" => Some(1),
            "lxor" => Some(2),
            "land" => Some(3),
            "bor" => Some(4),
            "bxor" => Some(5),
            "band" => Some(6),
            "bshl" | "bshr" | "bshru" => Some(7),
            _ => None,
        },
        Kind::Punct(Punct::Plus | Punct::Minus) => Some(8),
        Kind::Punct(Punct::Star) => Some(9),
        _ => None,
    }
}

fn is_comparison(kind: &Kind) -> bool {
    matches!(
        kind,
        Kind::Punct(Punct::Eq | Punct::Ne | Punct::Lt | Punct::Le | Punct::Gt | Punct::Ge)
    )
}

fn describe(kind: &Kind) -> String {
    match kind {
        Kind::Ident(word) => format!("`{word}`"),
        Kind::Number { .. } => "a number".to_owned(),
        Kind::Str(_) => "a string".to_owned(),
        Kind::Directive(name) => format!("`.{name}`"),
        Kind::Doc => "a doc comment".to_owned(),
        Kind::Punct(punct) => format!("`{}`", punct.text()),
        Kind::End => "the end of the file".to_owned(),
        Kind::Error(_) => "text that is not admitted".to_owned(),
    }
}

/// Why a directive is refused where it stands, unless it is an admitted one out of place.
fn directive_refusal(name: &str) -> Option<String> {
    Some(match name {
        "input" | "output" | "printsize" | "limitsize" => {
            format!("`.{name}` is not admitted: Mara generates the input and output directives")
        }
        "once" => "`.once` is admitted only before everything else in a file".to_owned(),
        "plan" => "`.plan` is admitted only right after a rule".to_owned(),
        "include" => "`.include` is admitted only outside components".to_owned(),
        "override" => "`.override` is admitted only in a component".to_owned(),
        "decl" | "type" | "comp" | "init" => return None,
        _ => format!("`.{name}` is not admitted"),
    })
}

impl Parser<'_> {
    fn peek(&self) -> &Kind {
        &self.tokens[self.next].kind
    }

    fn peek_at(&self, offset: usize) -> &Kind {
        let index = (self.next + offset).min(self.tokens.len() - 1);
        &self.tokens[index].kind
    }

    fn at(&self) -> u32 {
        self.tokens[self.next].at
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.next].clone();
        if self.next + 1 < self.tokens.len() {
            self.next += 1;
        }
        token
    }

    fn unexpected(&self, expected: &str) -> Fault {
        let found = self.peek();
        let message = match found {
            Kind::Error(message) => message.clone(),
            Kind::Doc => {
                "a doc comment is admitted only before a declaration, a fact or a rule".to_owned()
            }
            Kind::Directive(name) if let Some(refusal) = directive_refusal(name) => refusal,
            _ => format!("expected {expected}, found {}", describe(found)),
        };
        Fault::at(self.at() as usize, message)
    }

    fn is(&self, punct: Punct) -> bool {
        *self.peek() == Kind::Punct(punct)
    }

    fn eat(&mut self, punct: Punct) -> bool {
        let found = self.is(punct);
        if found {
            self.advance();
        }
        found
    }

    fn expect(&mut self, punct: Punct) -> Result<(), Fault> {
        if self.eat(punct) {
            Ok(())
        } else {
            Err(self.unexpected(&format!("`{}`", punct.text())))
        }
    }

    fn is_word(&self, word: &str) -> bool {
        matches!(self.peek(), Kind::Ident(found) if found == word)
    }

    /// One level deeper, refused past `MAX_DEPTH`.
    fn enter(&mut self) -> Result<(), Fault> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Fault::at(
                self.at() as usize,
                format!(
                    "the program nests more than {MAX_DEPTH} levels deep: each term, value, operator, group, record, call, aggregate, negation and component takes one or more"
                ),
            ));
        }
        Ok(())
    }

    /// Runs `parse` one level deeper. Every function that the parser can reach again before it
    /// returns (`term`, `operand`, `unary` and `component`; every cycle of calls passes through
    /// one of them) starts here, so no text recurses past `MAX_DEPTH`.
    fn nested<T>(&mut self, parse: impl FnOnce(&mut Self) -> Result<T, Fault>) -> Result<T, Fault> {
        self.enter()?;
        let result = parse(self);
        self.depth -= 1;
        result
    }

    /// A name that the program defines: no keyword and no name of a built-in type.
    fn name(&mut self, what: &str) -> Result<Name, Fault> {
        let at = self.at();
        match self.peek() {
            Kind::Ident(word) if KEYWORDS.contains(&word.as_str()) => Err(Fault::at(
                at as usize,
                format!("`{word}` is a reserved word and cannot name {what}"),
            )),
            Kind::Ident(word) if TYPE_WORDS.contains(&word.as_str()) => Err(Fault::at(
                at as usize,
                format!("`{word}` names a built-in type and cannot name {what}"),
            )),
            Kind::Ident(word) => {
                let text = word.clone();
                self.advance();
                Ok(Name { text, at })
            }
            _ => Err(self.unexpected(&format!("a name for {what}"))),
        }
    }

    /// One part of a type's name: a declared name, `symbol` or `number`.
    fn type_part(&mut self) -> Result<String, Fault> {
        match self.peek() {
            Kind::Ident(word) if word == "float" || word == "unsigned" => Err(Fault::at(
                self.at() as usize,
                format!(
                    "`{word}` is not admitted: types are `symbol`, `number`, and subtypes, unions and records of them"
                ),
            )),
            Kind::Ident(word) if TYPE_WORDS.contains(&word.as_str()) => {
                let word = word.clone();
                self.advance();
                Ok(word)
            }
            _ => Ok(self.name("a type")?.text),
        }
    }

    /// Parts joined by `.`, as a type or a relation is named.
    fn qualified(&mut self, what: &str) -> Result<QualifiedName, Fault> {
        let at = self.at();
        let mut parts = Vec::new();
        loop {
            let part = if what == "a type" {
                self.type_part()?
            } else {
                self.name(what)?.text
            };
            parts.push(part);
            if !self.eat(Punct::Dot) {
                return Ok(QualifiedName { parts, at });
            }
        }
    }

    fn file(&mut self) -> Result<File, Fault> {
        let once = *self.peek() == Kind::Directive("once");
        if once {
            self.advance();
        }
        let items = self.items(true)?;
        Ok(File { once, items })
    }

    fn items(&mut self, top: bool) -> Result<Vec<Item>, Fault> {
        let mut items = Vec::new();
        loop {
            let mut doc = None;
            while *self.peek() == Kind::Doc {
                doc = Some(self.advance().at);
            }
            let item = match self.peek() {
                Kind::End if top => None,
                Kind::Punct(Punct::RBrace) if !top => None,
                Kind::Directive("include") if top && doc.is_none() => {
                    Some(Item::Include(self.include()?))
                }
                Kind::Directive("type") => Some(Item::Type(self.type_decl()?)),
                Kind::Directive("decl") => Some(Item::Relation(self.relation_decl()?)),
                Kind::Directive("comp") => Some(Item::Component(self.component()?)),
                Kind::Directive("init") => Some(Item::Init(self.init()?)),
                Kind::Directive("override") if !top => {
                    self.advance();
                    self.name("a relation")?;
                    Some(Item::Override)
                }
                Kind::Ident(_) => Some(Item::Clause(self.clause()?)),
                Kind::Directive(_) if doc.is_none() => return Err(self.unexpected("")),
                _ => {
                    return Err(match doc {
                        Some(at) => Fault::at(
                            at as usize,
                            "a doc comment is admitted only before a declaration, a fact or a rule",
                        ),
                        None => self.unexpected("a declaration, a fact or a rule"),
                    });
                }
            };
            match item {
                Some(item) => items.push(item),
                None if doc.is_some() => {
                    return Err(Fault::at(
                        doc.unwrap_or_default() as usize,
                        "a doc comment is admitted only before a declaration, a fact or a rule",
                    ));
                }
                None => return Ok(items),
            }
        }
    }

    fn include(&mut self) -> Result<Include, Fault> {
        let directive = self.advance().at as usize;
        let at = self.at();
        let Kind::Str(path) = self.peek().clone() else {
            return Err(self.unexpected("the path to include, in quotes"));
        };
        let between = &self.text[directive + ".include".len()..at as usize];
        if !between.bytes().all(|byte| byte == b' ' || byte == b'\t') {
            return Err(Fault::at(
                at as usize,
                "only spaces and tabs are admitted between `.include` and its path",
            ));
        }
        let safe = path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte));
        let parts_valid = path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
        if !safe || !parts_valid {
            return Err(Fault::at(
                at as usize,
                "an included path is relative, without `.` or `..` parts, and holds only ASCII letters, digits, `.`, `_`, `-` and `/`",
            ));
        }
        self.advance();
        Ok(Include { path, at })
    }

    fn type_decl(&mut self) -> Result<TypeDecl, Fault> {
        self.advance();
        let name = self.name("a type")?;
        let definition = if self.eat(Punct::Subtype) {
            TypeDefinition::Subtype(self.qualified("a type")?)
        } else if self.eat(Punct::Eq) {
            if self.eat(Punct::LBracket) {
                let mut fields = Vec::new();
                if !self.eat(Punct::RBracket) {
                    loop {
                        fields.push(self.attribute()?);
                        if self.eat(Punct::RBracket) {
                            break;
                        }
                        self.expect(Punct::Comma)?;
                    }
                }
                TypeDefinition::Record(fields)
            } else {
                let mut members = vec![self.union_member()?];
                while self.eat(Punct::Pipe) {
                    members.push(self.union_member()?);
                }
                TypeDefinition::Union(members)
            }
        } else {
            return Err(self.unexpected("`<:` or `=` after the type's name"));
        };
        Ok(TypeDecl { name, definition })
    }

    fn union_member(&mut self) -> Result<QualifiedName, Fault> {
        if matches!(self.peek_at(1), Kind::Punct(Punct::LBrace)) {
            return Err(Fault::at(
                self.at() as usize,
                "algebraic data types are not admitted",
            ));
        }
        self.qualified("a type")
    }

    fn attribute(&mut self) -> Result<Attribute, Fault> {
        let name = self.name("an attribute")?;
        self.expect(Punct::Colon)?;
        let ty = self.qualified("a type")?;
        if self.is(Punct::Lt) {
            return Err(Fault::at(
                self.at() as usize,
                "lattice types are not admitted",
            ));
        }
        Ok(Attribute { name, ty })
    }

    fn relation_decl(&mut self) -> Result<RelationDecl, Fault> {
        self.advance();
        let mut names = vec![self.name("a relation")?];
        while self.eat(Punct::Comma) {
            names.push(self.name("a relation")?);
        }
        self.expect(Punct::LParen)?;
        let mut attributes = Vec::new();
        if !self.eat(Punct::RParen) {
            loop {
                attributes.push(self.attribute()?);
                if self.eat(Punct::RParen) {
                    break;
                }
                self.expect(Punct::Comma)?;
            }
        }
        let mut decl = RelationDecl {
            names,
            attributes,
            inline: false,
            overridable: false,
        };
        while let Kind::Ident(word) = self.peek() {
            match word.as_str() {
                "inline" => decl.inline = true,
                "overridable" => decl.overridable = true,
                "no_inline" | "brie" | "btree" | "eqrel" => {}
                "input" | "output" | "printsize" => {
                    return Err(Fault::at(
                        self.at() as usize,
                        format!(
                            "the `{word}` qualifier is not admitted: Mara generates the input and output directives"
                        ),
                    ));
                }
                "magic" | "no_magic" | "btree_delete" => {
                    return Err(Fault::at(
                        self.at() as usize,
                        format!("the `{word}` qualifier is not admitted"),
                    ));
                }
                _ => break,
            }
            self.advance();
        }
        if self.is(Punct::LParen) {
            return Err(Fault::at(
                self.at() as usize,
                "functional dependencies are not admitted",
            ));
        }
        Ok(decl)
    }

    fn component_ref(&mut self) -> Result<ComponentRef, Fault> {
        let name = self.name("a component")?;
        let mut args = Vec::new();
        if self.eat(Punct::Lt) {
            loop {
                let at = self.at();
                let part = self.type_part()?;
                if self.is(Punct::Dot) {
                    return Err(Fault::at(
                        self.at() as usize,
                        "a component's type argument is a single name: Soufflé reads no `.` in it",
                    ));
                }
                args.push(QualifiedName {
                    parts: vec![part],
                    at,
                });
                if self.eat(Punct::Gt) {
                    break;
                }
                self.expect(Punct::Comma)?;
            }
        }
        Ok(ComponentRef { name, args })
    }

    fn component(&mut self) -> Result<Component, Fault> {
        self.nested(Self::component_unguarded)
    }

    fn component_unguarded(&mut self) -> Result<Component, Fault> {
        self.advance();
        let name = self.name("a component")?;
        let mut params = Vec::new();
        if self.eat(Punct::Lt) {
            loop {
                params.push(self.name("a type parameter")?);
                if self.eat(Punct::Gt) {
                    break;
                }
                self.expect(Punct::Comma)?;
            }
        }
        let mut bases = Vec::new();
        if self.eat(Punct::Colon) {
            loop {
                bases.push(self.component_ref()?);
                if !self.eat(Punct::Comma) {
                    break;
                }
            }
        }
        self.expect(Punct::LBrace)?;
        let body = self.items(false)?;
        self.expect(Punct::RBrace)?;
        Ok(Component {
            name,
            params,
            bases,
            body,
        })
    }

    fn init(&mut self) -> Result<Init, Fault> {
        self.advance();
        let name = self.name("a component instance")?;
        self.expect(Punct::Eq)?;
        let component = self.component_ref()?;
        Ok(Init { name, component })
    }

    fn clause(&mut self) -> Result<Clause, Fault> {
        let mut heads = vec![self.atom()?];
        while self.eat(Punct::Comma) {
            heads.push(self.atom()?);
        }
        let body = if self.eat(Punct::If) {
            Some(self.body()?)
        } else if heads.len() == 1 && self.is(Punct::Dot) {
            None
        } else if self.is(Punct::Le) {
            return Err(Fault::at(self.at() as usize, "subsumption is not admitted"));
        } else {
            return Err(self.unexpected("`:-`"));
        };
        self.expect(Punct::Dot)?;
        let plan = if *self.peek() == Kind::Directive("plan") {
            if body.is_none() {
                return Err(self.unexpected(""));
            }
            Some(self.plan()?)
        } else {
            None
        };
        Ok(Clause { heads, body, plan })
    }

    fn plan_number(&mut self) -> Result<u32, Fault> {
        match *self.peek() {
            Kind::Number { value, decimal } if decimal && value < MAX_MAGNITUDE => {
                self.advance();
                Ok(value)
            }
            Kind::Number { .. } => Err(Fault::at(
                self.at() as usize,
                "a plan's numbers are decimal and below 2^31",
            )),
            _ => Err(self.unexpected("a number")),
        }
    }

    fn plan(&mut self) -> Result<Plan, Fault> {
        let at = self.advance().at;
        let mut entries = Vec::new();
        loop {
            let version = self.plan_number()?;
            self.expect(Punct::Colon)?;
            self.expect(Punct::LParen)?;
            let mut order = Vec::new();
            if !self.eat(Punct::RParen) {
                loop {
                    order.push(self.plan_number()?);
                    if self.eat(Punct::RParen) {
                        break;
                    }
                    self.expect(Punct::Comma)?;
                }
            }
            entries.push((version, order));
            if !self.eat(Punct::Comma) {
                return Ok(Plan { at, entries });
            }
        }
    }

    fn atom(&mut self) -> Result<Atom, Fault> {
        let name = self.qualified("a relation")?;
        if !self.is(Punct::LParen) {
            return Err(self.unexpected("`(`"));
        }
        let args = self.args()?;
        Ok(Atom { name, args })
    }

    /// `(` values separated by `,` `)`.
    fn args(&mut self) -> Result<Vec<Expr>, Fault> {
        self.expect(Punct::LParen)?;
        let mut args = Vec::new();
        if !self.eat(Punct::RParen) {
            loop {
                args.push(self.value()?);
                if self.eat(Punct::RParen) {
                    break;
                }
                self.expect(Punct::Comma)?;
            }
        }
        Ok(args)
    }

    fn body(&mut self) -> Result<Body, Fault> {
        let pieces = self.pieces()?;
        finish(pieces)
    }

    fn pieces(&mut self) -> Result<Vec<Vec<Piece>>, Fault> {
        let mut disjuncts = vec![self.conjunction()?];
        while self.is(Punct::Semicolon) {
            if self.aggregate_bodies > 0 {
                return Err(Fault::at(
                    self.at() as usize,
                    "a disjunction in an aggregate is not admitted",
                ));
            }
            self.advance();
            disjuncts.push(self.conjunction()?);
        }
        Ok(disjuncts)
    }

    fn conjunction(&mut self) -> Result<Vec<Piece>, Fault> {
        let mut terms = vec![self.term()?];
        while self.eat(Punct::Comma) {
            terms.push(self.term()?);
        }
        Ok(terms)
    }

    fn term(&mut self) -> Result<Piece, Fault> {
        self.nested(Self::term_unguarded)
    }

    fn term_unguarded(&mut self) -> Result<Piece, Fault> {
        let piece = if self.eat(Punct::Bang) {
            match self.term()? {
                Piece::Term(term) => Piece::Term(Term::Negation(Box::new(term))),
                Piece::Bare(_, at) => return Err(expected_constraint(at)),
            }
        } else {
            let at = self.at();
            self.term_start = self.next;
            let operand = self.operand(0)?;
            if is_comparison(self.peek()) {
                let left = into_value(operand, at)?;
                let operator = self.advance();
                let ordered = !matches!(operator.kind, Kind::Punct(Punct::Eq | Punct::Ne));
                let right = self.value()?;
                Piece::Term(Term::Compare(left, right, ordered.then_some(operator.at)))
            } else {
                match operand {
                    Operand::Atom(atom) => Piece::Term(Term::Atom(atom)),
                    Operand::Group(body) => Piece::Term(Term::Group(body)),
                    Operand::Boolean => Piece::Term(Term::Boolean),
                    Operand::Contains(a, b) => Piece::Term(Term::Contains(a, b)),
                    // A value alone is a term only as the whole of a group: `(x + 1) < y`.
                    Operand::Value(expr) if self.is(Punct::RParen) => Piece::Bare(expr, at),
                    Operand::Value(_) => return Err(self.unexpected("a comparison")),
                }
            }
        };
        Ok(piece)
    }

    fn value(&mut self) -> Result<Expr, Fault> {
        let at = self.at();
        let operand = self.operand(0)?;
        into_value(operand, at)
    }

    /// Precedence climbing over the binary operators.
    fn operand(&mut self, min: u8) -> Result<Operand, Fault> {
        self.nested(|parser| {
            let at = parser.at();
            let left = parser.unary(false)?;
            parser.binary_operators(left, at, min)
        })
    }

    /// The binary operators after `left`, which starts at `at`. Each one nests the tree one
    /// level deeper and counts as one.
    fn binary_operators(&mut self, mut left: Operand, at: u32, min: u8) -> Result<Operand, Fault> {
        let depth = self.depth;
        while let Some(precedence) = binary(self.peek()) {
            if precedence < min {
                break;
            }
            self.enter()?;
            let plus = self.is(Punct::Plus);
            let left_value = into_value(left, at)?;
            self.advance();
            let right_at = self.at();
            let right = into_value(self.operand(precedence + 1)?, right_at)?;
            left = Operand::Value(if plus {
                Expr::Plus(Box::new(left_value), Box::new(right))
            } else {
                Expr::Arithmetic(vec![left_value, right])
            });
        }
        self.depth = depth;
        Ok(left)
    }

    /// A unary operator or a primary. `negated` is set right under a `-`, where the literal
    /// 2147483648 is admitted.
    fn unary(&mut self, negated: bool) -> Result<Operand, Fault> {
        self.nested(|parser| parser.unary_unguarded(negated))
    }

    fn unary_unguarded(&mut self, negated: bool) -> Result<Operand, Fault> {
        let at = self.at();
        if self.is(Punct::Minus) {
            self.advance();
            let inner_at = self.at();
            let inner = into_value(self.unary(true)?, inner_at)?;
            return Ok(Operand::Value(match inner {
                Expr::Number(value) if -value > i64::from(i32::MAX) => {
                    return Err(Fault::at(
                        at as usize,
                        "this number is outside signed 32 bits",
                    ));
                }
                Expr::Number(value) => Expr::Number(-value),
                inner => Expr::Arithmetic(vec![inner]),
            }));
        }
        if self.is_word("bnot") || self.is_word("lnot") {
            self.advance();
            let inner_at = self.at();
            let inner = into_value(self.unary(false)?, inner_at)?;
            return Ok(Operand::Value(Expr::Arithmetic(vec![inner])));
        }
        if let Kind::Number { value, .. } = *self.peek() {
            self.advance();
            if value == MAX_MAGNITUDE && !negated {
                return Err(Fault::at(
                    at as usize,
                    "this number is outside signed 32 bits",
                ));
            }
            return Ok(Operand::Value(Expr::Number(i64::from(value))));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Operand, Fault> {
        let at = self.at();
        match self.peek().clone() {
            Kind::Str(_) => {
                self.advance();
                Ok(Operand::Value(Expr::Str))
            }
            Kind::Punct(Punct::Underscore) => {
                self.advance();
                Ok(Operand::Value(Expr::Wildcard))
            }
            Kind::Punct(Punct::LBracket) => {
                self.advance();
                let mut fields = Vec::new();
                if !self.eat(Punct::RBracket) {
                    loop {
                        fields.push(self.value()?);
                        if self.eat(Punct::RBracket) {
                            break;
                        }
                        self.expect(Punct::Comma)?;
                    }
                }
                Ok(Operand::Value(Expr::Record(fields)))
            }
            Kind::Punct(Punct::LParen) => {
                self.advance();
                let mut pieces = self.pieces()?;
                self.expect(Punct::RParen)?;
                let alone = pieces.len() == 1
                    && pieces[0].len() == 1
                    && matches!(pieces[0][0], Piece::Bare(..));
                if alone && let Some(Piece::Bare(expr, _)) = pieces[0].pop() {
                    return Ok(Operand::Value(expr));
                }
                Ok(Operand::Group(finish(pieces)?))
            }
            Kind::Ident(word) => self.word(&word, at),
            _ => Err(self.unexpected("a value")),
        }
    }

    /// A primary that starts with a name: a built-in, a variable or an atom.
    fn word(&mut self, word: &str, at: u32) -> Result<Operand, Fault> {
        let functor = match word {
            "cat" => Some((Functor::Cat, 0, usize::MAX)),
            "strlen" => Some((Functor::Strlen, 1, 1)),
            "substr" => Some((Functor::Substr, 3, 3)),
            "to_string" => Some((Functor::ToString, 1, 1)),
            "to_number" => Some((Functor::ToNumber, 1, 1)),
            "range" => Some((Functor::Range, 2, 3)),
            _ => None,
        };
        if let Some((functor, least, most)) = functor {
            self.advance();
            if !self.is(Punct::LParen) {
                return Err(self.unexpected("`(`"));
            }
            let args = self.args()?;
            if args.len() < least || args.len() > most {
                return Err(Fault::at(
                    at as usize,
                    format!("`{word}` takes a different number of arguments"),
                ));
            }
            return Ok(Operand::Value(Expr::Call(functor, args)));
        }
        match word {
            "nil" => {
                self.advance();
                Ok(Operand::Value(Expr::Nil))
            }
            "true" | "false" => {
                self.advance();
                Ok(Operand::Boolean)
            }
            "contains" => {
                self.advance();
                let args = self.args()?;
                let [a, b]: [Expr; 2] = args
                    .try_into()
                    .map_err(|_| Fault::at(at as usize, "`contains` takes two arguments"))?;
                Ok(Operand::Contains(a, b))
            }
            "min" | "max" if matches!(self.peek_at(1), Kind::Punct(Punct::LParen)) => {
                // `min(a, b, …)` is the function; `min (x + 1) : a(x)` an aggregate whose
                // value starts with a parenthesis. The token after the first value decides.
                self.advance();
                let paren = self.at();
                self.advance();
                let first = self.value()?;
                if self.eat(Punct::Comma) {
                    let mut args = vec![first];
                    loop {
                        args.push(self.value()?);
                        if self.eat(Punct::RParen) {
                            break;
                        }
                        self.expect(Punct::Comma)?;
                    }
                    let functor = if word == "min" {
                        Functor::Min
                    } else {
                        Functor::Max
                    };
                    return Ok(Operand::Value(Expr::Call(functor, args)));
                }
                self.expect(Punct::RParen)?;
                let target = self.binary_operators(Operand::Value(first), paren, 0)?;
                let target = into_value(target, paren)?;
                let aggregator = if word == "min" {
                    Aggregator::Min
                } else {
                    Aggregator::Max
                };
                self.aggregate_body(aggregator, Some(Box::new(target)), at)
            }
            "count" | "sum" | "min" | "max" => self.aggregate(word, at),
            _ if KEYWORDS.contains(&word) => Err(Fault::at(
                at as usize,
                format!("`{word}` is not admitted here"),
            )),
            _ => {
                let name = if self.next == self.term_start {
                    self.qualified("a relation or a variable")?
                } else {
                    let name = self.name("a variable")?;
                    QualifiedName {
                        parts: vec![name.text],
                        at,
                    }
                };
                if self.is(Punct::LParen) {
                    let args = self.args()?;
                    return Ok(Operand::Atom(Atom { name, args }));
                }
                if name.parts.len() > 1 {
                    return Err(self.unexpected("`(` after a qualified relation name"));
                }
                let text = name.parts.into_iter().next().unwrap_or_default();
                Ok(Operand::Value(Expr::Var(Name { text, at })))
            }
        }
    }

    fn aggregate(&mut self, word: &str, at: u32) -> Result<Operand, Fault> {
        self.advance();
        let aggregator = match word {
            "count" => Aggregator::Count,
            "sum" => Aggregator::Sum,
            "min" => Aggregator::Min,
            _ => Aggregator::Max,
        };
        let target = if aggregator == Aggregator::Count {
            None
        } else if self.is(Punct::Colon) {
            return Err(Fault::at(
                at as usize,
                format!("`{word}` needs a value to aggregate before `:`"),
            ));
        } else {
            Some(Box::new(self.value()?))
        };
        self.aggregate_body(aggregator, target, at)
    }

    /// `:` and the body of an aggregate, which admits no `;` at any depth.
    fn aggregate_body(
        &mut self,
        aggregator: Aggregator,
        target: Option<Box<Expr>>,
        at: u32,
    ) -> Result<Operand, Fault> {
        self.expect(Punct::Colon)?;
        let body = if self.eat(Punct::LBrace) {
            self.aggregate_bodies += 1;
            let pieces = self.pieces()?;
            self.aggregate_bodies -= 1;
            self.expect(Punct::RBrace)?;
            finish(pieces)?.0.pop().unwrap_or_default()
        } else {
            vec![Term::Atom(self.atom()?)]
        };
        Ok(Operand::Value(Expr::Aggregate {
            aggregator,
            target,
            body,
            at,
        }))
    }
}

fn expected_constraint(at: u32) -> Fault {
    Fault::at(
        at as usize,
        "expected an atom or a constraint, found a value alone",
    )
}

fn into_value(operand: Operand, at: u32) -> Result<Expr, Fault> {
    match operand {
        Operand::Value(expr) => Ok(expr),
        Operand::Atom(atom) => Err(Fault::at(
            at as usize,
            format!(
                "`{}` is a relation here, not a value: user-defined functors are not admitted",
                atom.name.text()
            ),
        )),
        Operand::Group(_) | Operand::Boolean | Operand::Contains(..) => Err(Fault::at(
            at as usize,
            "expected a value, found a constraint",
        )),
    }
}

/// A body whose every piece is a term.
fn finish(pieces: Vec<Vec<Piece>>) -> Result<Body, Fault> {
    let mut body = Vec::with_capacity(pieces.len());
    for conjunction in pieces {
        let mut terms = Vec::with_capacity(conjunction.len());
        for piece in conjunction {
            match piece {
                Piece::Term(term) => terms.push(term),
                Piece::Bare(_, at) => return Err(expected_constraint(at)),
            }
        }
        body.push(terms);
    }
    Ok(Body(body))
}
