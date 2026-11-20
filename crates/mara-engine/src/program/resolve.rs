//! A program's files, components and types, and the checks that need them.
//!
//! Names resolve as Soufflé 2.5 resolves them, by the behaviour the reviews and this module's
//! tests observed: an include against the including file's directory; inside a component
//! instance, a relation or type name against the instance first, then the instances around it,
//! then the program; a type parameter only in its own component's body.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::syntax::{
    self, Aggregator, Clause, Component, Expr, Functor, Item, QualifiedName, Term,
};
use super::{
    Column, ColumnKind, DECLARATIONS, Fact, Files, Include, Place, Program, Refusal, Relation,
    line_starts, locate, parser,
};

/// How many declarations, clauses, instances and inherited bodies a program may expand to.
const MAX_EXPANSION: usize = 100_000;

/// How deep component expansion may recurse: instances in instances and inheritance chains
/// together.
const MAX_COMPONENT_DEPTH: usize = 100;

struct Source<'a> {
    path: String,
    text: &'a str,
    lines: Vec<usize>,
    file: syntax::File,
}

fn refusal(source: &Source, at: u32, message: impl Into<String>) -> Refusal {
    Refusal {
        message: message.into(),
        location: locate(&source.path, source.text, &source.lines, at as usize),
    }
}

fn parse_source<'a>(path: &str, bytes: &'a [u8]) -> Result<Source<'a>, Refusal> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        let valid = std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or_default();
        Refusal {
            message: "the file is not valid UTF-8".to_owned(),
            location: locate(path, valid, &line_starts(valid), valid.len()),
        }
    })?;
    let lines = line_starts(text);
    let file = parser::parse(text).map_err(|fault| Refusal {
        message: fault.message,
        location: locate(path, text, &lines, fault.at),
    })?;
    Ok(Source {
        path: path.to_owned(),
        text,
        lines,
        file,
    })
}

/// The declared program at `root` and every file it includes, each once, with every
/// `.include` that loads them.
fn load<'a>(root: &str, files: &Files<'a>) -> Result<(Vec<Source<'a>>, Vec<Include>), Refusal> {
    let refuse_root = |message: &str| Refusal {
        message: message.to_owned(),
        location: locate(root, "", &[0], 0),
    };
    if root == DECLARATIONS {
        return Err(refuse_root(
            "the project path `mara.dl` is reserved for Mara's declarations",
        ));
    }
    let Some(bytes) = files.programs.get(root) else {
        return Err(refuse_root("no program pattern declares this file"));
    };
    let mut sources = vec![parse_source(root, bytes)?];
    let mut sites = Vec::new();
    let mut loaded = HashMap::from([(root.to_owned(), 0)]);
    let mut next = 0;
    while next < sources.len() {
        let mut includes = Vec::new();
        for item in &sources[next].file.items {
            let Item::Include(include) = item else {
                continue;
            };
            let source = &sources[next];
            let dir = source.path.rsplit_once('/').map_or("", |(dir, _)| dir);
            let candidate = match dir {
                "" => include.path.clone(),
                dir => format!("{dir}/{}", include.path),
            };
            let target = if candidate == DECLARATIONS || files.programs.contains_key(&candidate) {
                candidate
            } else if include.path == DECLARATIONS {
                DECLARATIONS.to_owned()
            } else {
                return Err(refusal(
                    source,
                    include.at,
                    format!(
                        "`{}` is neither `mara.dl` nor a file that a program pattern declares",
                        include.path
                    ),
                ));
            };
            includes.push((target, include.at));
        }
        for (target, at) in includes {
            if let Some(&index) = loaded.get(&target) {
                sites.push(Include {
                    file: next,
                    at: at as usize,
                    target: index,
                });
                if !sources[index].file.once {
                    return Err(refusal(
                        &sources[next],
                        at,
                        format!(
                            "`{target}` is included more than once, so it must start with `.once`"
                        ),
                    ));
                }
                continue;
            }
            let bytes = if target == DECLARATIONS {
                files.declarations
            } else {
                &files.programs[&target]
            };
            loaded.insert(target.clone(), sources.len());
            sites.push(Include {
                file: next,
                at: at as usize,
                target: sources.len(),
            });
            sources.push(parse_source(&target, bytes)?);
        }
        next += 1;
    }
    Ok((sources, sites))
}

/// Items of one source with the frame that reads them, before they are given a namespace.
type Unit<'a> = (&'a [Item], usize, usize);

/// The names that one component instance, or the program itself, declares.
#[derive(Default)]
struct Namespace<'a> {
    /// `""` for the program, `c.` for instance `c`.
    prefix: String,
    parent: Option<usize>,
    types: HashSet<String>,
    relations: HashSet<String>,
    instances: HashSet<String>,
    /// Component definitions, with their source and the namespace they are defined in.
    components: HashMap<String, (&'a Component, usize, usize)>,
}

/// One component's type arguments: each parameter's argument, with the source, namespace and
/// frame it is written in.
#[derive(Default)]
struct Frame<'a> {
    params: HashMap<String, (&'a QualifiedName, usize, usize, usize)>,
}

/// Items, from one source, as one namespace and frame read them.
#[derive(Clone, Copy)]
struct Body<'a> {
    items: &'a [Item],
    source: usize,
    ns: usize,
    frame: usize,
}

/// A resolved type reference: the qualified type, or `symbol` or `number`, and where it is
/// written.
struct Ref {
    name: String,
    source: usize,
    at: u32,
}

enum Definition {
    Subtype(Ref),
    Union(Vec<Ref>),
    /// The types of its fields.
    Record(Vec<Ref>),
}

struct FlatRelation {
    columns: Vec<(String, Ref)>,
    inline: bool,
    source: usize,
    at: u32,
}

struct Resolver<'a> {
    sources: &'a [Source<'a>],
    namespaces: Vec<Namespace<'a>>,
    frames: Vec<Frame<'a>>,
    bodies: Vec<Body<'a>>,
    type_names: HashSet<String>,
    expansion: usize,
    /// How deep component expansion has recursed.
    depth: usize,
    types: Vec<(String, Definition, usize, u32)>,
    relations: BTreeMap<String, FlatRelation>,
    clauses: Vec<(&'a Clause, usize, usize)>,
}

pub(super) fn resolve(root: &str, files: &Files) -> Result<Program, Refusal> {
    let (sources, includes) = load(root, files)?;
    let mut resolver = Resolver {
        sources: &sources,
        namespaces: vec![Namespace::default()],
        frames: vec![Frame::default()],
        bodies: Vec::new(),
        type_names: HashSet::new(),
        expansion: 0,
        depth: 0,
        types: Vec::new(),
        relations: BTreeMap::new(),
        clauses: Vec::new(),
    };
    // Mara's declarations first, so that a program that declares their names again is refused
    // in its own file.
    let mut program: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(source, file)| (&file.file.items[..], source, 0))
        .collect();
    program.sort_by_key(|&(_, source, _)| sources[source].path != DECLARATIONS);
    resolver.expand(0, program, &mut Vec::new())?;
    for body in resolver.bodies.clone() {
        resolver.flatten(body)?;
    }
    let kinds = resolver.kinds()?;
    let relations = resolver.relations(&kinds);
    resolver.check_clauses(&relations, &kinds)?;
    let mut facts = Vec::new();
    for &(clause, source, ns) in &resolver.clauses {
        if clause.body.is_some() {
            continue;
        }
        for head in &clause.heads {
            if let Some(relation) = resolver.relation_name(&head.name, ns) {
                let place = Place {
                    file: source,
                    offset: head.name.at as usize,
                };
                facts.push(Fact { relation, place });
            }
        }
    }
    let mut atoms = BTreeSet::new();
    for source in sources.iter().filter(|source| source.path != DECLARATIONS) {
        atom_names(&source.file.items, &mut atoms);
    }
    Ok(Program {
        files: sources.iter().map(|source| source.path.clone()).collect(),
        relations,
        atoms,
        includes,
        facts,
        texts: sources
            .iter()
            .map(|source| (source.text.to_owned(), source.lines.clone()))
            .collect(),
    })
}

impl<'a> Resolver<'a> {
    fn refuse(&self, source: usize, at: u32, message: impl Into<String>) -> Refusal {
        refusal(&self.sources[source], at, message)
    }

    /// One level deeper into component expansion, before `expand` or `component_bodies`
    /// recurses: instances in instances and inheritance chains share the count.
    fn descend(&mut self, source: usize, at: u32) -> Result<(), Refusal> {
        self.depth += 1;
        if self.depth > MAX_COMPONENT_DEPTH {
            return Err(self.refuse(
                source,
                at,
                format!("components nest or inherit more than {MAX_COMPONENT_DEPTH} levels deep"),
            ));
        }
        Ok(())
    }

    fn count(&mut self, source: usize, at: u32) -> Result<(), Refusal> {
        self.expansion += 1;
        if self.expansion > MAX_EXPANSION {
            return Err(self.refuse(
                source,
                at,
                format!("the program expands to more than {MAX_EXPANSION} declarations, clauses, instances and inherited bodies"),
            ));
        }
        Ok(())
    }

    fn declare(
        &mut self,
        ns: usize,
        kind: &str,
        name: &syntax::Name,
        source: usize,
    ) -> Result<(), Refusal> {
        self.count(source, name.at)?;
        let namespace = &mut self.namespaces[ns];
        let set = match kind {
            "type" => &mut namespace.types,
            "relation" => &mut namespace.relations,
            _ => &mut namespace.instances,
        };
        if !set.insert(name.text.clone()) {
            return Err(self.refuse(
                source,
                name.at,
                format!("the {kind} `{}` is declared twice", name.text),
            ));
        }
        if kind == "type" {
            let qualified = format!("{}{}", self.namespaces[ns].prefix, name.text);
            self.type_names.insert(qualified);
        }
        Ok(())
    }

    /// Declares the names of `bodies` in `ns`, then expands each instance they create.
    fn expand(
        &mut self,
        ns: usize,
        bodies: Vec<Unit<'a>>,
        chain: &mut Vec<*const Component>,
    ) -> Result<(), Refusal> {
        for &(items, source, _) in &bodies {
            for item in items {
                match item {
                    Item::Type(decl) => self.declare(ns, "type", &decl.name, source)?,
                    Item::Relation(decl) => {
                        for name in &decl.names {
                            self.declare(ns, "relation", name, source)?;
                        }
                    }
                    Item::Init(init) => self.declare(ns, "instance", &init.name, source)?,
                    Item::Component(component) => {
                        let previous = self.namespaces[ns]
                            .components
                            .insert(component.name.text.clone(), (component, source, ns));
                        if previous.is_some() {
                            return Err(self.refuse(
                                source,
                                component.name.at,
                                format!(
                                    "the component `{}` is declared twice",
                                    component.name.text
                                ),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        for &(items, source, frame) in &bodies {
            for item in items {
                let Item::Init(init) = item else {
                    continue;
                };
                let (component, component_source, defined_in) =
                    self.component(ns, &init.component.name, source)?;
                if chain.contains(&std::ptr::from_ref(component)) {
                    return Err(self.refuse(
                        source,
                        init.name.at,
                        format!(
                            "the component `{}` instantiates itself",
                            component.name.text
                        ),
                    ));
                }
                let prefix = format!("{}{}.", self.namespaces[ns].prefix, init.name.text);
                let child = self.namespaces.len();
                self.namespaces.push(Namespace {
                    prefix,
                    parent: Some(ns),
                    ..Namespace::default()
                });
                let frame = self.bind(component, &init.component, source, ns, frame)?;
                let mut bases = Vec::new();
                self.descend(source, init.name.at)?;
                let bodies = self.component_bodies(
                    component,
                    component_source,
                    defined_in,
                    child,
                    frame,
                    &mut bases,
                )?;
                chain.push(std::ptr::from_ref(component));
                self.expand(child, bodies, chain)?;
                chain.pop();
                self.depth -= 1;
            }
        }
        for (items, source, frame) in bodies {
            self.bodies.push(Body {
                items,
                source,
                ns,
                frame,
            });
        }
        Ok(())
    }

    fn component(
        &self,
        ns: usize,
        name: &syntax::Name,
        source: usize,
    ) -> Result<(&'a Component, usize, usize), Refusal> {
        let mut scope = Some(ns);
        while let Some(index) = scope {
            if let Some(&found) = self.namespaces[index].components.get(&name.text) {
                return Ok(found);
            }
            scope = self.namespaces[index].parent;
        }
        Err(self.refuse(
            source,
            name.at,
            format!("no component `{}` is declared", name.text),
        ))
    }

    /// A frame binding `component`'s parameters to the arguments of `reference`, which is
    /// written in `source` and read in namespace `ns` and frame `frame`.
    fn bind(
        &mut self,
        component: &'a Component,
        reference: &'a syntax::ComponentRef,
        source: usize,
        ns: usize,
        frame: usize,
    ) -> Result<usize, Refusal> {
        if component.params.len() != reference.args.len() {
            return Err(self.refuse(
                source,
                reference.name.at,
                format!(
                    "the component `{}` takes {} type arguments",
                    component.name.text,
                    component.params.len()
                ),
            ));
        }
        let params = component
            .params
            .iter()
            .zip(&reference.args)
            .map(|(param, arg)| (param.text.clone(), (arg, source, ns, frame)))
            .collect();
        self.frames.push(Frame { params });
        Ok(self.frames.len() - 1)
    }

    /// The bodies an instance of `component` holds: its bases' first, then its own.
    fn component_bodies(
        &mut self,
        component: &'a Component,
        source: usize,
        defined_in: usize,
        ns: usize,
        frame: usize,
        bases: &mut Vec<*const Component>,
    ) -> Result<Vec<Unit<'a>>, Refusal> {
        // Every body counts, empty ones included, before anything is expanded.
        self.count(source, component.name.at)?;
        bases.push(std::ptr::from_ref(component));
        let mut bodies = Vec::new();
        for base in &component.bases {
            let (inherited, inherited_source, inherited_in) =
                self.component(defined_in, &base.name, source)?;
            if bases.contains(&std::ptr::from_ref(inherited)) {
                return Err(self.refuse(
                    source,
                    base.name.at,
                    format!(
                        "the component `{}` inherits from itself",
                        inherited.name.text
                    ),
                ));
            }
            let inherited_frame = self.bind(inherited, base, source, ns, frame)?;
            self.descend(source, base.name.at)?;
            bodies.extend(self.component_bodies(
                inherited,
                inherited_source,
                inherited_in,
                ns,
                inherited_frame,
                bases,
            )?);
            self.depth -= 1;
        }
        bases.pop();
        bodies.push((&component.body[..], source, frame));
        Ok(bodies)
    }

    /// The type that `name`, written in `source`, means in namespace `ns` and frame `frame`.
    fn resolve_type(
        &self,
        mut name: &'a QualifiedName,
        mut source: usize,
        mut ns: usize,
        mut frame: usize,
    ) -> Result<String, Refusal> {
        loop {
            if let [single] = &name.parts[..] {
                if let Some(&(arg, arg_source, arg_ns, arg_frame)) =
                    self.frames[frame].params.get(single)
                {
                    (name, source, ns, frame) = (arg, arg_source, arg_ns, arg_frame);
                    continue;
                }
                if single == "symbol" || single == "number" {
                    return Ok(single.clone());
                }
            }
            let mut scope = Some(ns);
            while let Some(index) = scope {
                let namespace = &self.namespaces[index];
                let found = match &name.parts[..] {
                    [single] => namespace.types.contains(single),
                    [first, ..] => namespace.instances.contains(first),
                    [] => false,
                };
                let qualified = format!("{}{}", namespace.prefix, name.text());
                if found && self.type_names.contains(&qualified) {
                    return Ok(qualified);
                }
                if found {
                    break;
                }
                scope = namespace.parent;
            }
            return Err(self.refuse(
                source,
                name.at,
                format!("no type `{}` is declared", name.text()),
            ));
        }
    }

    fn reference(&self, name: &'a QualifiedName, body: Body) -> Result<Ref, Refusal> {
        Ok(Ref {
            name: self.resolve_type(name, body.source, body.ns, body.frame)?,
            source: body.source,
            at: name.at,
        })
    }

    /// Records the types, relations and clauses of one body under their qualified names.
    fn flatten(&mut self, body: Body<'a>) -> Result<(), Refusal> {
        let prefix = self.namespaces[body.ns].prefix.clone();
        for item in body.items {
            match item {
                Item::Type(decl) => {
                    let definition = match &decl.definition {
                        syntax::TypeDefinition::Subtype(base) => {
                            Definition::Subtype(self.reference(base, body)?)
                        }
                        syntax::TypeDefinition::Union(members) => Definition::Union(
                            members
                                .iter()
                                .map(|member| self.reference(member, body))
                                .collect::<Result<_, _>>()?,
                        ),
                        syntax::TypeDefinition::Record(fields) => Definition::Record(
                            fields
                                .iter()
                                .map(|field| self.reference(&field.ty, body))
                                .collect::<Result<_, _>>()?,
                        ),
                    };
                    let name = format!("{prefix}{}", decl.name.text);
                    self.types
                        .push((name, definition, body.source, decl.name.at));
                }
                Item::Relation(decl) => {
                    for name in &decl.names {
                        let columns = decl
                            .attributes
                            .iter()
                            .map(|attribute| {
                                Ok((
                                    attribute.name.text.clone(),
                                    self.reference(&attribute.ty, body)?,
                                ))
                            })
                            .collect::<Result<_, Refusal>>()?;
                        let relation = FlatRelation {
                            columns,
                            inline: decl.inline,
                            source: body.source,
                            at: name.at,
                        };
                        self.relations
                            .insert(format!("{prefix}{}", name.text), relation);
                    }
                }
                Item::Clause(clause) => {
                    self.count(body.source, clause.heads[0].name.at)?;
                    self.clauses.push((clause, body.source, body.ns));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The kind of every declared type: what its subtypes and unions come down to.
    fn kinds(&self) -> Result<HashMap<&str, ColumnKind>, Refusal> {
        let index: HashMap<&str, usize> = self
            .types
            .iter()
            .enumerate()
            .map(|(at, (name, ..))| (name.as_str(), at))
            .collect();
        let builtin = |name: &str| match name {
            "symbol" => Some(ColumnKind::Symbol),
            "number" => Some(ColumnKind::Number),
            _ => None,
        };
        // `None` while a type is on the stack, so a type defined through itself is found.
        let mut kinds: HashMap<&str, Option<ColumnKind>> = HashMap::new();
        for (start, ..) in &self.types {
            if kinds.contains_key(start.as_str()) {
                continue;
            }
            kinds.insert(start, None);
            let mut stack = vec![index[start.as_str()]];
            while let Some(&top) = stack.last() {
                let (name, definition, source, at) = &self.types[top];
                let mut pushed = false;
                for dependency in dependencies(definition) {
                    if builtin(&dependency.name).is_some() {
                        continue;
                    }
                    match kinds.get(dependency.name.as_str()) {
                        Some(Some(_)) => {}
                        Some(None) => {
                            return Err(self.refuse(
                                *source,
                                *at,
                                format!("the type `{name}` is defined through itself"),
                            ));
                        }
                        None => {
                            kinds.insert(&dependency.name, None);
                            stack.push(index[dependency.name.as_str()]);
                            pushed = true;
                            break;
                        }
                    }
                }
                if pushed {
                    continue;
                }
                let kind_of = |reference: &Ref| {
                    builtin(&reference.name)
                        .or_else(|| kinds[reference.name.as_str()])
                        .expect("resolved before")
                };
                let kind = match definition {
                    Definition::Record(_) => ColumnKind::Record,
                    Definition::Subtype(base) => match kind_of(base) {
                        ColumnKind::Record => {
                            return Err(self.refuse(
                                base.source,
                                base.at,
                                "a subtype of a record type is not admitted",
                            ));
                        }
                        kind => kind,
                    },
                    Definition::Union(members) => {
                        let first = kind_of(&members[0]);
                        for member in &members[1..] {
                            let kind = kind_of(member);
                            if kind != first || kind == ColumnKind::Record {
                                return Err(self.refuse(
                                    member.source,
                                    member.at,
                                    "a union admits only subtypes of `symbol` or only subtypes of `number`",
                                ));
                            }
                        }
                        if members.len() > 1 && first == ColumnKind::Record {
                            return Err(self.refuse(
                                members[0].source,
                                members[0].at,
                                "a union admits only subtypes of `symbol` or only subtypes of `number`",
                            ));
                        }
                        first
                    }
                };
                kinds.insert(name, Some(kind));
                stack.pop();
            }
        }
        Ok(kinds
            .into_iter()
            .map(|(name, kind)| (name, kind.expect("resolved")))
            .chain([
                ("symbol", ColumnKind::Symbol),
                ("number", ColumnKind::Number),
            ])
            .collect())
    }

    fn relations(&self, kinds: &HashMap<&str, ColumnKind>) -> BTreeMap<String, Relation> {
        self.relations
            .iter()
            .map(|(name, relation)| {
                let columns = relation
                    .columns
                    .iter()
                    .map(|(column, reference)| Column {
                        name: column.clone(),
                        kind: kinds[reference.name.as_str()],
                        place: Place {
                            file: reference.source,
                            offset: reference.at as usize,
                        },
                    })
                    .collect();
                let relation = Relation {
                    columns,
                    inline: relation.inline,
                    place: Place {
                        file: relation.source,
                        offset: relation.at as usize,
                    },
                };
                (name.clone(), relation)
            })
            .collect()
    }

    /// The relation that `name` means in namespace `ns`, if one is declared.
    fn relation_name(&self, name: &QualifiedName, ns: usize) -> Option<String> {
        let mut scope = Some(ns);
        while let Some(index) = scope {
            let namespace = &self.namespaces[index];
            let found = match &name.parts[..] {
                [single] => namespace.relations.contains(single),
                [first, ..] => namespace.instances.contains(first),
                [] => false,
            };
            if found {
                let qualified = format!("{}{}", namespace.prefix, name.text());
                return self.relations.contains_key(&qualified).then_some(qualified);
            }
            scope = namespace.parent;
        }
        None
    }
}

impl Resolver<'_> {
    fn check_clauses(
        &self,
        relations: &BTreeMap<String, Relation>,
        kinds: &HashMap<&str, ColumnKind>,
    ) -> Result<(), Refusal> {
        let types: HashMap<&str, &Definition> = self
            .types
            .iter()
            .map(|(name, definition, ..)| (name.as_str(), definition))
            .collect();
        let mut recursion = None;
        for &(clause, source, ns) in &self.clauses {
            let mut checker = ClauseChecker {
                resolver: self,
                relations,
                kinds,
                types: &types,
                source,
                ns,
                records: HashSet::new(),
            };
            let mut atoms: Vec<_> = clause.heads.iter().collect();
            if let Some(body) = &clause.body {
                for conjunction in &body.0 {
                    atoms_of_terms(conjunction, &mut atoms);
                }
            }
            checker.record_vars(&atoms);
            for head in &clause.heads {
                for arg in &head.args {
                    checker.expr(arg)?;
                }
            }
            if let Some(body) = &clause.body {
                for conjunction in &body.0 {
                    checker.terms(conjunction)?;
                }
            }
            if let Some(plan) = &clause.plan {
                let sccs = recursion.get_or_insert_with(|| self.components_of_recursion());
                self.check_plan(clause, plan, source, ns, sccs)?;
            }
        }
        Ok(())
    }

    /// Each relation's strongly connected component in the graph from each head to the
    /// relations its body reads.
    fn components_of_recursion(&self) -> HashMap<String, usize> {
        let mut nodes: HashMap<String, usize> = HashMap::new();
        let mut edges: Vec<Vec<usize>> = Vec::new();
        let mut node = |name: String, edges: &mut Vec<Vec<usize>>| {
            let next = nodes.len();
            *nodes.entry(name).or_insert_with(|| {
                edges.push(Vec::new());
                next
            })
        };
        for &(clause, _, ns) in &self.clauses {
            let mut read = Vec::new();
            if let Some(body) = &clause.body {
                for conjunction in &body.0 {
                    atoms_of_terms(conjunction, &mut read);
                }
            }
            for head in &clause.heads {
                let Some(head) = self.relation_name(&head.name, ns) else {
                    continue;
                };
                let from = node(head, &mut edges);
                for atom in &read {
                    if let Some(to) = self.relation_name(&atom.name, ns) {
                        let to = node(to, &mut edges);
                        edges[from].push(to);
                    }
                }
            }
        }
        let components = tarjan(&edges);
        nodes
            .into_iter()
            .map(|(name, node)| (name, components[node]))
            .collect()
    }

    fn check_plan(
        &self,
        clause: &Clause,
        plan: &syntax::Plan,
        source: usize,
        ns: usize,
        sccs: &HashMap<String, usize>,
    ) -> Result<(), Refusal> {
        let refuse = |message: &str| Err(self.refuse(source, plan.at, message));
        let body = clause.body.as_ref().expect("a plan follows a rule");
        if clause.heads.len() != 1 || body.0.len() != 1 {
            return refuse("a plan is admitted only for a rule with one head and no `;`");
        }
        if body.0[0].iter().any(|term| matches!(term, Term::Group(_))) {
            return refuse("a plan is admitted only for a rule without parentheses around terms");
        }
        let component = |name: &QualifiedName| {
            self.relation_name(name, ns)
                .and_then(|name| sccs.get(&name).copied())
        };
        let head = component(&clause.heads[0].name);
        let atoms: Vec<_> = body.0[0]
            .iter()
            .filter_map(|term| match term {
                Term::Atom(atom) => Some(atom),
                _ => None,
            })
            .collect();
        let versions = atoms
            .iter()
            .filter(|atom| head.is_some() && component(&atom.name) == head)
            .count();
        if versions == 0 {
            return refuse("a plan is admitted only for a recursive rule");
        }
        let mut seen = HashSet::new();
        for (version, order) in &plan.entries {
            if *version as usize >= versions || !seen.insert(*version) {
                return refuse(&format!(
                    "this rule has plan versions 0 to {}, each planned once",
                    versions - 1
                ));
            }
            let mut sorted = order.clone();
            sorted.sort_unstable();
            if !sorted.iter().copied().eq(1..=atoms.len() as u32) {
                return refuse(&format!(
                    "a plan orders each of the rule's {} atoms once, numbered from 1",
                    atoms.len()
                ));
            }
        }
        Ok(())
    }
}

fn atoms_of_terms<'a>(terms: &'a [Term], out: &mut Vec<&'a syntax::Atom>) {
    for term in terms {
        match term {
            Term::Atom(atom) => {
                out.push(atom);
                atom.args.iter().for_each(|arg| atoms_of_expr(arg, out));
            }
            Term::Negation(term) => atoms_of_terms(std::slice::from_ref(&**term), out),
            Term::Group(body) => body.0.iter().for_each(|terms| atoms_of_terms(terms, out)),
            Term::Compare(a, b, _) | Term::Contains(a, b) => {
                atoms_of_expr(a, out);
                atoms_of_expr(b, out);
            }
            Term::Boolean => {}
        }
    }
}

fn atoms_of_expr<'a>(expr: &'a Expr, out: &mut Vec<&'a syntax::Atom>) {
    match expr {
        Expr::Record(items) | Expr::Arithmetic(items) | Expr::Call(_, items) => {
            items.iter().for_each(|item| atoms_of_expr(item, out));
        }
        Expr::Plus(a, b) => {
            atoms_of_expr(a, out);
            atoms_of_expr(b, out);
        }
        Expr::Aggregate { target, body, .. } => {
            if let Some(target) = target {
                atoms_of_expr(target, out);
            }
            atoms_of_terms(body, out);
        }
        Expr::Number(_) | Expr::Str | Expr::Var(_) | Expr::Wildcard | Expr::Nil => {}
    }
}

fn dependencies(definition: &Definition) -> &[Ref] {
    match definition {
        Definition::Subtype(base) => std::slice::from_ref(base),
        Definition::Union(members) => members,
        Definition::Record(_) => &[],
    }
}

/// The strongly connected component of each node, by Tarjan's algorithm without recursion.
fn tarjan(edges: &[Vec<usize>]) -> Vec<usize> {
    const UNSEEN: usize = usize::MAX;
    let count = edges.len();
    let (mut index, mut low, mut component) =
        (vec![UNSEEN; count], vec![0; count], vec![UNSEEN; count]);
    let (mut on_stack, mut stack, mut next, mut components) =
        (vec![false; count], Vec::new(), 0, 0);
    for start in 0..count {
        if index[start] != UNSEEN {
            continue;
        }
        let mut calls = vec![(start, 0)];
        while let Some(&(node, edge)) = calls.last() {
            if edge == 0 && index[node] == UNSEEN {
                index[node] = next;
                low[node] = next;
                next += 1;
                stack.push(node);
                on_stack[node] = true;
            }
            if let Some(&to) = edges[node].get(edge) {
                if let Some(call) = calls.last_mut() {
                    call.1 += 1;
                }
                if index[to] == UNSEEN {
                    calls.push((to, 0));
                } else if on_stack[to] {
                    low[node] = low[node].min(index[to]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(parent, _)) = calls.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component[member] = components;
                    if member == node {
                        break;
                    }
                }
                components += 1;
            }
        }
    }
    component
}

/// Checks the aggregates of one clause: `min` and `max` aggregate only numbers.
struct ClauseChecker<'a, 'b> {
    resolver: &'b Resolver<'a>,
    relations: &'b BTreeMap<String, Relation>,
    kinds: &'b HashMap<&'b str, ColumnKind>,
    types: &'b HashMap<&'b str, &'b Definition>,
    source: usize,
    ns: usize,
    /// The variables in record columns of the clause's atoms.
    records: HashSet<&'b str>,
}

impl<'b> ClauseChecker<'_, 'b> {
    /// The variables of `atoms` in record columns.
    fn record_vars(&mut self, atoms: &[&'b syntax::Atom]) {
        for atom in atoms {
            let Some(relation) = self
                .resolver
                .relation_name(&atom.name, self.ns)
                .and_then(|name| self.relations.get(&name))
            else {
                continue;
            };
            for (arg, column) in atom.args.iter().zip(&relation.columns) {
                if let Expr::Var(var) = arg
                    && column.kind == ColumnKind::Record
                {
                    self.records.insert(var.text.as_str());
                }
            }
        }
    }

    /// Whether `expr` is a record by its form or a variable in a record column.
    fn record(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Record(_) | Expr::Nil => true,
            Expr::Var(var) => self.records.contains(var.text.as_str()),
            _ => false,
        }
    }

    fn terms(&self, terms: &[Term]) -> Result<(), Refusal> {
        for term in terms {
            match term {
                Term::Atom(atom) => atom.args.iter().try_for_each(|arg| self.expr(arg))?,
                Term::Negation(term) => self.terms(std::slice::from_ref(&**term))?,
                Term::Group(body) => body.0.iter().try_for_each(|terms| self.terms(terms))?,
                Term::Compare(a, b, ordered) => {
                    if let Some(at) = ordered
                        && (self.record(a) || self.record(b))
                    {
                        return Err(self.resolver.refuse(
                            self.source,
                            *at,
                            "records have no order: `<`, `<=`, `>` and `>=` compare numbers or symbols",
                        ));
                    }
                    self.expr(a)?;
                    self.expr(b)?;
                }
                Term::Contains(a, b) => {
                    self.expr(a)?;
                    self.expr(b)?;
                }
                Term::Boolean => {}
            }
        }
        Ok(())
    }

    fn expr(&self, expr: &Expr) -> Result<(), Refusal> {
        match expr {
            Expr::Record(items) | Expr::Arithmetic(items) | Expr::Call(_, items) => {
                items.iter().try_for_each(|item| self.expr(item))
            }
            Expr::Plus(a, b) => {
                self.expr(a)?;
                self.expr(b)
            }
            Expr::Aggregate {
                aggregator,
                target,
                body,
                at,
            } => {
                if let Some(target) = target {
                    self.expr(target)?;
                    let refused = matches!(aggregator, Aggregator::Min | Aggregator::Max)
                        && !numeric(target, &self.numeric_vars(body));
                    if refused {
                        return Err(self.resolver.refuse(
                            self.source,
                            *at,
                            "`min` and `max` aggregates are admitted only over numbers that the aggregate's own atoms or comparisons show to be numbers",
                        ));
                    }
                }
                self.terms(body)
            }
            Expr::Number(_) | Expr::Str | Expr::Var(_) | Expr::Wildcard | Expr::Nil => Ok(()),
        }
    }

    /// The variables that `body` shows to be numbers: arguments of its atoms in `number`
    /// columns or in `number` fields of a record column, and variables it compares with such
    /// numbers or destructures from such records. Groups without `;` count as part of the body.
    fn numeric_vars<'t>(&self, body: &'t [Term]) -> HashSet<&'t str> {
        let mut terms = Vec::new();
        flatten_groups(body, &mut terms);
        let mut numbers = HashSet::new();
        let mut records = HashMap::new();
        for term in &terms {
            let Term::Atom(atom) = term else {
                continue;
            };
            let Some(relation) = self
                .resolver
                .relation_name(&atom.name, self.ns)
                .and_then(|name| self.resolver.relations.get(&name))
            else {
                continue;
            };
            for (arg, (_, column)) in atom.args.iter().zip(&relation.columns) {
                self.bind(arg, &column.name, &mut numbers, &mut records);
            }
        }
        loop {
            let before = numbers.len() + records.len();
            for term in &terms {
                let Term::Compare(a, b, _) = term else {
                    continue;
                };
                for (one, other) in [(a, b), (b, a)] {
                    let Expr::Var(var) = one else {
                        continue;
                    };
                    if numeric(other, &numbers) {
                        numbers.insert(var.text.as_str());
                    }
                    if let Some(&ty) = records.get(var.text.as_str()) {
                        self.bind(other, ty, &mut numbers, &mut records);
                    }
                }
            }
            if numbers.len() + records.len() == before {
                return numbers;
            }
        }
    }

    /// Records what a value of type `ty` makes of `expr`: a number variable, a record variable
    /// with its type, or the same for each field of a record.
    fn bind<'t>(
        &self,
        expr: &'t Expr,
        ty: &'b str,
        numbers: &mut HashSet<&'t str>,
        records: &mut HashMap<&'t str, &'b str>,
    ) {
        match (self.kinds.get(ty), expr) {
            (Some(ColumnKind::Number), Expr::Var(var)) => {
                numbers.insert(var.text.as_str());
            }
            (Some(ColumnKind::Record), Expr::Var(var)) => {
                records.insert(var.text.as_str(), ty);
            }
            (Some(ColumnKind::Record), Expr::Record(items)) => {
                if let Some(fields) = self.record_fields(ty) {
                    for (item, field) in items.iter().zip(fields) {
                        self.bind(item, &field.name, numbers, records);
                    }
                }
            }
            _ => {}
        }
    }

    /// The field types of record type `ty`, through aliases.
    fn record_fields(&self, mut ty: &'b str) -> Option<&'b [Ref]> {
        for _ in 0..=self.types.len() {
            match self.types.get(ty)? {
                Definition::Record(fields) => return Some(fields),
                Definition::Union(members) if members.len() == 1 => ty = &members[0].name,
                _ => return None,
            }
        }
        None
    }
}

/// The terms of `body`, with the terms of each group without `;` in place of the group.
fn flatten_groups<'t>(body: &'t [Term], out: &mut Vec<&'t Term>) {
    for term in body {
        match term {
            Term::Group(group) if group.0.len() == 1 => flatten_groups(&group.0[0], out),
            term => out.push(term),
        }
    }
}

/// Whether `expr` is a number by its form and the numeric variables `vars`.
fn numeric(expr: &Expr, vars: &HashSet<&str>) -> bool {
    match expr {
        Expr::Number(_) | Expr::Arithmetic(_) => true,
        Expr::Var(var) => vars.contains(var.text.as_str()),
        Expr::Plus(a, b) => numeric(a, vars) && numeric(b, vars),
        Expr::Call(Functor::Strlen | Functor::ToNumber | Functor::Range, _) => true,
        Expr::Call(Functor::Min | Functor::Max, args) => args.iter().all(|arg| numeric(arg, vars)),
        Expr::Call(Functor::Cat | Functor::Substr | Functor::ToString, _) => false,
        Expr::Aggregate { aggregator, .. } => {
            matches!(aggregator, Aggregator::Count | Aggregator::Sum)
        }
        Expr::Str | Expr::Wildcard | Expr::Nil | Expr::Record(_) => false,
    }
}

/// Adds the name of every atom of `items`, as written, to `names`: in the heads and bodies of
/// clauses, aggregates and component bodies included.
fn atom_names(items: &[Item], names: &mut BTreeSet<String>) {
    fn terms(list: &[Term], names: &mut BTreeSet<String>) {
        for term in list {
            match term {
                Term::Atom(atom) => atom_and_args(atom, names),
                Term::Negation(term) => terms(std::slice::from_ref(&**term), names),
                Term::Group(body) => body.0.iter().for_each(|list| terms(list, names)),
                Term::Compare(a, b, _) | Term::Contains(a, b) => {
                    expr(a, names);
                    expr(b, names);
                }
                Term::Boolean => {}
            }
        }
    }
    fn atom_and_args(atom: &syntax::Atom, names: &mut BTreeSet<String>) {
        names.insert(atom.name.text());
        atom.args.iter().for_each(|arg| expr(arg, names));
    }
    fn expr(expr_: &Expr, names: &mut BTreeSet<String>) {
        match expr_ {
            Expr::Record(items) | Expr::Arithmetic(items) | Expr::Call(_, items) => {
                items.iter().for_each(|item| expr(item, names))
            }
            Expr::Plus(a, b) => {
                expr(a, names);
                expr(b, names);
            }
            Expr::Aggregate { target, body, .. } => {
                if let Some(target) = target {
                    expr(target, names);
                }
                terms(body, names);
            }
            Expr::Number(_) | Expr::Str | Expr::Var(_) | Expr::Wildcard | Expr::Nil => {}
        }
    }
    for item in items {
        match item {
            Item::Clause(clause) => {
                clause
                    .heads
                    .iter()
                    .for_each(|atom| atom_and_args(atom, names));
                if let Some(body) = &clause.body {
                    body.0.iter().for_each(|list| terms(list, names));
                }
            }
            Item::Component(component) => atom_names(&component.body, names),
            _ => {}
        }
    }
}
