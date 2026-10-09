//! Validate named SELECT declarations without inferring possible result types.
use super::*;
use crate::Schema;
use oxrdf::Variable;
use spargebra::{
    Query, SparqlParser,
    algebra::{Expression, GraphPattern, PropertyPathExpression},
    term::{GroundTerm, NamedNodePattern, TermPattern},
};

pub(super) struct Compiled {
    pub query: Query,
}

pub(super) fn compile(
    file: &str,
    definition: &Definition,
    schema: &Schema,
) -> Result<Compiled, Error> {
    compile_inner(definition, schema).map_err(|e| Error::definition(file, e))
}

fn compile_inner(definition: &Definition, schema: &Schema) -> Result<Compiled, String> {
    if definition.root_flavours.is_empty() || definition.columns.is_empty() {
        return Err("declare nonempty root flavour and tuple signatures".into());
    }
    if !definition
        .required_fact_sources
        .contains(&FactSource::Documents)
    {
        return Err("required_fact_sources must declare documents".into());
    }
    let mut sources = BTreeSet::new();
    if definition
        .required_fact_sources
        .iter()
        .any(|source| !sources.insert(*source))
    {
        return Err("required_fact_sources contains duplicates".into());
    }
    if definition
        .root_flavours
        .iter()
        .any(|name| !schema.flavours().contains_key(name))
    {
        return Err("unknown root flavour".into());
    }

    let mut names = BTreeSet::new();
    for (name, column) in &definition.columns {
        if !valid(name) || !names.insert(name.clone()) {
            return Err("invalid/repeated column name".into());
        }
        validate_column(name, column, schema)?;
    }
    for name in &definition.support_columns {
        if !valid(name) || !names.insert(name.clone()) {
            return Err("invalid/repeated support column".into());
        }
    }
    for name in &definition.source_columns {
        if !valid(name) || !names.insert(name.clone()) {
            return Err("invalid/repeated source column".into());
        }
    }
    for (name, parameter) in &definition.parameters {
        if !valid(name) || !names.insert(name.clone()) {
            return Err("invalid parameter name or collision with projection".into());
        }
        if !matches!(
            parameter.r#type,
            ValueType::Text
                | ValueType::Integer
                | ValueType::Number
                | ValueType::Boolean
                | ValueType::Item
                | ValueType::Iri
        ) {
            return Err("unsupported parameter signature".into());
        }
        if parameter
            .default
            .as_ref()
            .is_some_and(|value| value.kind() != parameter.r#type)
        {
            return Err("parameter default type mismatch".into());
        }
        if let Some(ParameterValue::Number(value)) = parameter.default
            && !value.is_finite()
        {
            return Err("nonfinite parameter default".into());
        }
        if let Some(ParameterValue::Iri(iri)) = &parameter.default {
            NamedNode::new(iri).map_err(|_| "invalid default IRI")?;
        }
    }

    let mut query = SparqlParser::new()
        .parse_query(&definition.select)
        .map_err(|e| e.to_string())?;
    let Query::Select {
        dataset: None,
        pattern,
        ..
    } = &mut query
    else {
        return Err("named queries require SELECT over the supplied default graph".into());
    };
    crate::query_backend::profile(pattern)?;

    let mut projected = BTreeSet::new();
    pattern.on_in_scope_variable(|variable| {
        projected.insert(variable.as_str().to_owned());
    });
    let expected = std::iter::once("root".to_owned())
        .chain(definition.columns.keys().cloned())
        .chain(definition.support_columns.iter().cloned())
        .chain(definition.source_columns.iter().cloned())
        .collect::<BTreeSet<_>>();
    if projected != expected {
        return Err(
            "SELECT projection must exactly match root, tuple, source and support columns".into(),
        );
    }

    let mut used = BTreeSet::new();
    normalize(
        pattern,
        schema,
        &mut used,
        &mut BTreeMap::new(),
        &BTreeMap::new(),
    )?;
    if used
        .iter()
        .chain(&projected)
        .any(|name| name.starts_with("__mara_"))
    {
        return Err("__mara_ variables are reserved for the query engine".into());
    }
    if nested_projection(pattern, true) {
        return Err("nested SELECT signatures belong to the later composition delivery".into());
    }
    if definition
        .parameters
        .keys()
        .any(|name| !used.contains(name))
    {
        return Err("declared parameter is unused by SELECT".into());
    }
    Ok(Compiled { query })
}

fn validate_column(name: &str, column: &Column, schema: &Schema) -> Result<(), String> {
    if column.r#type != ValueType::Item && !column.flavours.is_empty() {
        return Err(format!("flavours apply only to item column {name}"));
    }
    if column
        .flavours
        .iter()
        .any(|flavour| !schema.flavours().contains_key(flavour))
    {
        return Err(format!("unknown declared flavour for column {name}"));
    }
    Ok(())
}

fn valid(name: &str) -> bool {
    name != "root" && !name.starts_with("__mara_") && crate::rules::valid_parameter_name(name)
}

// Bind ID spellings and schema aliases to the canonical graph vocabulary.
pub(super) fn bind_items(
    query: &mut Query,
    schema: &Schema,
    graph: &PreparedGraph,
) -> Result<(), String> {
    let aliases = graph
        .items
        .iter()
        .filter_map(|(iri, (endpoint, _))| {
            let Endpoint::Item { id, .. } = endpoint else {
                return None;
            };
            Some((
                Endpoint::Item {
                    id: id.clone(),
                    mid: None,
                }
                .iri(),
                NamedNode::new(iri).unwrap(),
            ))
        })
        .collect();
    let Query::Select { pattern, .. } = query else {
        unreachable!()
    };
    normalize(
        pattern,
        schema,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
        &aliases,
    )
}

pub(super) fn add_parameter_projection<'a>(
    query: &mut Query,
    parameters: impl Iterator<Item = &'a String>,
) {
    let Query::Select { pattern, .. } = query else {
        unreachable!()
    };
    fn project(pattern: &mut GraphPattern, parameters: &[Variable]) {
        match pattern {
            GraphPattern::Distinct { inner } => project(inner, parameters),
            GraphPattern::Project { variables, .. } => {
                for parameter in parameters {
                    if !variables.contains(parameter) {
                        variables.push(parameter.clone());
                    }
                }
            }
            _ => unreachable!("SELECT projection"),
        }
    }
    let parameters = parameters
        .map(|name| Variable::new(name).expect("validated parameter name"))
        .collect::<Vec<_>>();
    project(pattern, &parameters);
}

pub(super) fn validate_item_constants(query: &Query, graph: &PreparedGraph) -> Result<(), String> {
    let Query::Select { pattern, .. } = query else {
        unreachable!()
    };
    validate_item_pattern(pattern, graph)
}

fn validate_item_node(node: &NamedNode, graph: &PreparedGraph) -> Result<(), String> {
    if (node.as_str().starts_with("urn:mara:id:") || node.as_str().starts_with("urn:mara:mid:"))
        && !graph.items.contains_key(node.as_str())
    {
        return Err("unknown exact item reference in SELECT".into());
    }
    Ok(())
}

fn validate_item_term(term: &TermPattern, graph: &PreparedGraph) -> Result<(), String> {
    if let TermPattern::NamedNode(node) = term {
        validate_item_node(node, graph)?;
    }
    Ok(())
}

fn validate_item_pattern(pattern: &GraphPattern, graph: &PreparedGraph) -> Result<(), String> {
    match pattern {
        GraphPattern::Bgp { patterns } => {
            for triple in patterns {
                validate_item_term(&triple.subject, graph)?;
                validate_item_term(&triple.object, graph)?;
            }
        }
        GraphPattern::Path {
            subject, object, ..
        } => {
            validate_item_term(subject, graph)?;
            validate_item_term(object, graph)?;
        }
        GraphPattern::Values { bindings, .. } => {
            for term in bindings.iter().flatten().flatten() {
                if let GroundTerm::NamedNode(node) = term {
                    validate_item_node(node, graph)?;
                }
            }
        }
        GraphPattern::Extend {
            inner, expression, ..
        } => {
            validate_item_expression(expression, graph)?;
            validate_item_pattern(inner, graph)?;
        }
        GraphPattern::Join { left, right }
        | GraphPattern::Union { left, right }
        | GraphPattern::Minus { left, right } => {
            validate_item_pattern(left, graph)?;
            validate_item_pattern(right, graph)?;
        }
        GraphPattern::LeftJoin {
            left,
            right,
            expression,
        } => {
            validate_item_pattern(left, graph)?;
            validate_item_pattern(right, graph)?;
            if let Some(expression) = expression {
                validate_item_expression(expression, graph)?;
            }
        }
        GraphPattern::Filter { inner, expr } => {
            validate_item_pattern(inner, graph)?;
            validate_item_expression(expr, graph)?;
        }
        GraphPattern::Project { inner, .. } | GraphPattern::Distinct { inner } => {
            validate_item_pattern(inner, graph)?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_item_expression(expression: &Expression, graph: &PreparedGraph) -> Result<(), String> {
    match expression {
        Expression::NamedNode(node) => validate_item_node(node, graph)?,
        Expression::Exists(pattern) => validate_item_pattern(pattern, graph)?,
        Expression::SameTerm(left, right)
        | Expression::And(left, right)
        | Expression::Or(left, right) => {
            validate_item_expression(left, graph)?;
            validate_item_expression(right, graph)?;
        }
        Expression::Not(inner) => validate_item_expression(inner, graph)?,
        _ => {}
    }
    Ok(())
}

fn nested_projection(pattern: &GraphPattern, outer: bool) -> bool {
    match pattern {
        GraphPattern::Distinct { inner } => nested_projection(inner, outer),
        GraphPattern::Project { inner, .. } => !outer || nested_projection(inner, false),
        GraphPattern::Join { left, right }
        | GraphPattern::Union { left, right }
        | GraphPattern::Minus { left, right } => {
            nested_projection(left, false) || nested_projection(right, false)
        }
        GraphPattern::LeftJoin {
            left,
            right,
            expression,
        } => {
            nested_projection(left, false)
                || nested_projection(right, false)
                || expression.as_ref().is_some_and(expression_projection)
        }
        GraphPattern::Filter { inner, expr } => {
            nested_projection(inner, false) || expression_projection(expr)
        }
        GraphPattern::Extend {
            inner, expression, ..
        } => nested_projection(inner, false) || expression_projection(expression),
        _ => false,
    }
}

fn expression_projection(expression: &Expression) -> bool {
    match expression {
        Expression::Exists(pattern) => nested_projection(pattern, false),
        Expression::SameTerm(left, right)
        | Expression::And(left, right)
        | Expression::Or(left, right) => {
            expression_projection(left) || expression_projection(right)
        }
        Expression::Not(inner) => expression_projection(inner),
        _ => false,
    }
}

fn collect(term: &TermPattern, used: &mut BTreeSet<String>) {
    if let TermPattern::Variable(variable) = term {
        used.insert(variable.as_str().into());
    }
}

// spargebra lowers sequence-path blank nodes to existential intermediate terms.
fn existential(term: &mut TermPattern, names: &mut BTreeMap<String, Variable>) {
    if let TermPattern::BlankNode(node) = term {
        let index = names.len();
        *term = names
            .entry(node.as_str().into())
            .or_insert_with(|| Variable::new_unchecked(format!("__mara_existential_{index}")))
            .clone()
            .into();
    }
}

fn relation_path(path: &mut PropertyPathExpression, schema: &Schema) -> Result<(), String> {
    match path {
        PropertyPathExpression::NamedNode(node) => {
            let raw = node
                .as_str()
                .strip_prefix("urn:mara:relation:")
                .ok_or("property paths must bind schema relations")?;
            let (canonical, _, inverse) = schema
                .resolve_relation(raw)
                .ok_or_else(|| format!("unknown schema relation {raw}"))?;
            let canonical = PropertyPathExpression::NamedNode(
                NamedNode::new(format!("urn:mara:relation:{canonical}")).unwrap(),
            );
            *path = if inverse {
                PropertyPathExpression::Reverse(Box::new(canonical))
            } else {
                canonical
            };
        }
        PropertyPathExpression::Reverse(inner)
        | PropertyPathExpression::ZeroOrMore(inner)
        | PropertyPathExpression::OneOrMore(inner)
        | PropertyPathExpression::ZeroOrOne(inner) => relation_path(inner, schema)?,
        PropertyPathExpression::Sequence(left, right)
        | PropertyPathExpression::Alternative(left, right) => {
            relation_path(left, schema)?;
            relation_path(right, schema)?;
        }
        PropertyPathExpression::NegatedPropertySet(_) => {
            return Err("negated open-vocabulary paths are not schema-bound".into());
        }
    }
    Ok(())
}

fn normalize(
    pattern: &mut GraphPattern,
    schema: &Schema,
    used: &mut BTreeSet<String>,
    existentials: &mut BTreeMap<String, Variable>,
    item_aliases: &BTreeMap<String, NamedNode>,
) -> Result<(), String> {
    match pattern {
        GraphPattern::Bgp { patterns } => {
            for triple in patterns {
                collect(&triple.subject, used);
                collect(&triple.object, used);
                vocabulary_term(&mut triple.subject, schema, item_aliases)?;
                vocabulary_term(&mut triple.object, schema, item_aliases)?;
                existential(&mut triple.subject, existentials);
                existential(&mut triple.object, existentials);
                let NamedNodePattern::NamedNode(predicate) = &triple.predicate else {
                    return Err("predicate variables are not schema-bound".into());
                };
                let mut predicate = predicate.clone();
                let inverse = normalize_predicate(&mut predicate, schema)?;
                triple.predicate = predicate.into();
                if inverse {
                    std::mem::swap(&mut triple.subject, &mut triple.object);
                }
            }
        }
        GraphPattern::Path {
            subject,
            path,
            object,
        } => {
            collect(subject, used);
            collect(object, used);
            vocabulary_term(subject, schema, item_aliases)?;
            vocabulary_term(object, schema, item_aliases)?;
            existential(subject, existentials);
            existential(object, existentials);
            relation_path(path, schema)?;
        }
        GraphPattern::Join { left, right }
        | GraphPattern::Union { left, right }
        | GraphPattern::Minus { left, right } => {
            normalize(left, schema, used, existentials, item_aliases)?;
            normalize(right, schema, used, existentials, item_aliases)?;
        }
        GraphPattern::LeftJoin {
            left,
            right,
            expression,
        } => {
            normalize(left, schema, used, existentials, item_aliases)?;
            normalize(right, schema, used, existentials, item_aliases)?;
            if let Some(expression) = expression {
                normalize_expression(expression, schema, used, existentials, item_aliases)?;
            }
        }
        GraphPattern::Filter { expr, inner } => {
            normalize(inner, schema, used, existentials, item_aliases)?;
            normalize_expression(expr, schema, used, existentials, item_aliases)?;
        }
        GraphPattern::Extend {
            variable,
            expression,
            inner,
        } => {
            used.insert(variable.as_str().into());
            normalize(inner, schema, used, existentials, item_aliases)?;
            normalize_expression(expression, schema, used, existentials, item_aliases)?;
        }
        GraphPattern::Values {
            variables,
            bindings,
        } => {
            used.extend(
                variables
                    .iter()
                    .map(|variable| variable.as_str().to_owned()),
            );
            for term in bindings.iter_mut().flatten().flatten() {
                if let GroundTerm::NamedNode(node) = term {
                    vocabulary_node(node, schema, item_aliases)?;
                }
            }
        }
        GraphPattern::Project { inner, .. } | GraphPattern::Distinct { inner } => {
            normalize(inner, schema, used, existentials, item_aliases)?;
        }
        _ => return Err("unsupported named SELECT construct".into()),
    }
    Ok(())
}

fn normalize_predicate(predicate: &mut NamedNode, schema: &Schema) -> Result<bool, String> {
    let iri = predicate.as_str();
    if let Some(raw) = iri.strip_prefix("urn:mara:relation:") {
        let (canonical, _, inverse) = schema
            .resolve_relation(raw)
            .ok_or_else(|| format!("unknown schema relation {raw}"))?;
        *predicate = NamedNode::new(format!("urn:mara:relation:{canonical}")).unwrap();
        return Ok(inverse);
    }
    if let Some(field) = iri.strip_prefix("urn:mara:field:") {
        if schema_has_field(schema, field) {
            return Ok(false);
        }
        return Err(format!("unknown schema field {field}"));
    }
    if iri == oxrdf::vocab::rdf::TYPE.as_str()
        || matches!(iri, "urn:mara:item:id" | "urn:mara:item:title")
        || matches!(
            iri,
            "urn:mara:query:source" | "urn:mara:query:target" | "urn:mara:query:relation"
        )
    {
        return Ok(false);
    }
    Err(format!("unknown schema-bound predicate {iri}"))
}

fn schema_has_field(schema: &Schema, field: &str) -> bool {
    schema
        .flavours()
        .values()
        .any(|flavour| flavour.fields.contains_key(field))
}

fn vocabulary_node(
    node: &mut NamedNode,
    schema: &Schema,
    item_aliases: &BTreeMap<String, NamedNode>,
) -> Result<(), String> {
    if let Some(canonical) = item_aliases.get(node.as_str()) {
        *node = canonical.clone();
    }
    if let Some(name) = node.as_str().strip_prefix("urn:mara:flavour:") {
        if !schema.flavours().contains_key(name) {
            return Err(format!("unknown schema flavour {name}"));
        }
    } else if let Some(name) = node.as_str().strip_prefix("urn:mara:field:") {
        if !schema_has_field(schema, name) {
            return Err(format!("unknown schema field {name}"));
        }
    } else if let Some(name) = node.as_str().strip_prefix("urn:mara:relation:") {
        let (canonical, _, _) = schema
            .resolve_relation(name)
            .ok_or_else(|| format!("unknown schema relation {name}"))?;
        *node = NamedNode::new(format!("urn:mara:relation:{canonical}")).unwrap();
    }
    Ok(())
}

fn vocabulary_term(
    term: &mut TermPattern,
    schema: &Schema,
    item_aliases: &BTreeMap<String, NamedNode>,
) -> Result<(), String> {
    if let TermPattern::NamedNode(node) = term {
        vocabulary_node(node, schema, item_aliases)?;
    }
    Ok(())
}

fn normalize_expression(
    expression: &mut Expression,
    schema: &Schema,
    used: &mut BTreeSet<String>,
    existentials: &mut BTreeMap<String, Variable>,
    item_aliases: &BTreeMap<String, NamedNode>,
) -> Result<(), String> {
    match expression {
        Expression::NamedNode(node) => vocabulary_node(node, schema, item_aliases)?,
        Expression::Variable(variable) | Expression::Bound(variable) => {
            used.insert(variable.as_str().into());
        }
        Expression::SameTerm(left, right)
        | Expression::And(left, right)
        | Expression::Or(left, right) => {
            normalize_expression(left, schema, used, existentials, item_aliases)?;
            normalize_expression(right, schema, used, existentials, item_aliases)?;
        }
        Expression::Not(inner) => {
            normalize_expression(inner, schema, used, existentials, item_aliases)?;
        }
        Expression::Exists(pattern) => {
            normalize(pattern, schema, used, existentials, item_aliases)?;
        }
        _ => {}
    }
    Ok(())
}
