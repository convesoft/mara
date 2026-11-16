//! The syntax tree of one program file. Positions are byte offsets into the file's text.

/// One file: whether it starts with `.once`, and its items.
#[derive(Clone, Debug)]
pub struct File {
    pub once: bool,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Include(Include),
    Type(TypeDecl),
    Relation(RelationDecl),
    Component(Component),
    Init(Init),
    /// `.override name`: Soufflé checks that the base declares it `overridable`.
    Override,
    Clause(Clause),
}

#[derive(Clone, Debug)]
pub struct Include {
    pub path: String,
    pub at: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub at: u32,
}

/// A name with `.` between its parts, such as `c.r` or `c.Type`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualifiedName {
    pub parts: Vec<String>,
    pub at: u32,
}

impl QualifiedName {
    pub fn text(&self) -> String {
        self.parts.join(".")
    }
}

#[derive(Clone, Debug)]
pub struct TypeDecl {
    pub name: Name,
    pub definition: TypeDefinition,
}

#[derive(Clone, Debug)]
pub enum TypeDefinition {
    /// `<: base`
    Subtype(QualifiedName),
    /// `= a | b`, one member for an alias.
    Union(Vec<QualifiedName>),
    /// `= [field: type, …]`
    Record(Vec<Attribute>),
}

#[derive(Clone, Debug)]
pub struct Attribute {
    pub name: Name,
    pub ty: QualifiedName,
}

#[derive(Clone, Debug)]
pub struct RelationDecl {
    pub names: Vec<Name>,
    pub attributes: Vec<Attribute>,
    pub inline: bool,
    pub overridable: bool,
}

#[derive(Clone, Debug)]
pub struct Component {
    pub name: Name,
    pub params: Vec<Name>,
    pub bases: Vec<ComponentRef>,
    pub body: Vec<Item>,
}

/// A component with its type arguments, as `.init` and a base list name it.
#[derive(Clone, Debug)]
pub struct ComponentRef {
    pub name: Name,
    pub args: Vec<QualifiedName>,
}

#[derive(Clone, Debug)]
pub struct Init {
    pub name: Name,
    pub component: ComponentRef,
}

/// A fact or a rule, with its `.plan`.
#[derive(Clone, Debug)]
pub struct Clause {
    pub heads: Vec<Atom>,
    pub body: Option<Body>,
    pub plan: Option<Plan>,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub at: u32,
    /// Each version with its order of atoms, both as written.
    pub entries: Vec<(u32, Vec<u32>)>,
}

/// Disjuncts of conjuncts.
#[derive(Clone, Debug)]
pub struct Body(pub Vec<Vec<Term>>);

#[derive(Clone, Debug)]
pub enum Term {
    Atom(Atom),
    Negation(Box<Term>),
    Group(Body),
    /// Two values compared; the position of `<`, `<=`, `>` or `>=`, which order them.
    Compare(Expr, Expr, Option<u32>),
    Contains(Expr, Expr),
    Boolean,
}

#[derive(Clone, Debug)]
pub struct Atom {
    pub name: QualifiedName,
    pub args: Vec<Expr>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Functor {
    Cat,
    Strlen,
    Substr,
    ToString,
    ToNumber,
    Range,
    Min,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aggregator {
    Count,
    Sum,
    Min,
    Max,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Number(i64),
    Str,
    Var(Name),
    Wildcard,
    Nil,
    Record(Vec<Expr>),
    /// `+`, which also joins symbols.
    Plus(Box<Expr>, Box<Expr>),
    /// Every other operator, unary or binary: they all take and give numbers.
    Arithmetic(Vec<Expr>),
    Call(Functor, Vec<Expr>),
    Aggregate {
        aggregator: Aggregator,
        target: Option<Box<Expr>>,
        body: Vec<Term>,
        at: u32,
    },
}
