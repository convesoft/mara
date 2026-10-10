use super::*;
use crate::{FieldFilter, ItemFilters};
use oxigraph::{
    model::{BlankNode, GraphName, NamedOrBlankNode, Quad, Variable},
    sparql::{QueryResults, SparqlEvaluator},
    store::Store,
};
use spargebra::{
    Query,
    algebra::{Expression, Function, GraphPattern, OrderExpression},
};

#[derive(Debug, Clone)]
pub(super) struct LoadedDefinition {
    pub(super) file: String,
    pub(super) definition: Definition,
}

struct QueryEvaluation {
    bindings: Vec<Binding>,
    complete: bool,
    unavailable_reason: Option<String>,
}

struct EvaluationContext<'a> {
    definitions: &'a BTreeMap<String, LoadedDefinition>,
    compiled: &'a BTreeMap<String, signature::Compiled>,
    corpus: &'a crate::Corpus,
    graph: &'a PreparedGraph,
    store: &'a Store,
    max_rows: u32,
    prepared: &'a BTreeMap<String, facts::Prepared<'a>>,
}

#[derive(Default)]
struct Derived {
    support: BTreeMap<String, Support>,
    facts: BTreeMap<String, FactSupport>,
}
struct EvaluationState<'a> {
    derived: &'a mut Derived,
    observed_rows: &'a mut u32,
}

// @mara implements REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara implements DES-QUERY-COMPOSITION-CONTRACT
pub(super) fn execute(engine: &SourceEngine, request: Request) -> Result<Page, Error> {
    let filtered = !request.selection.ids.is_empty()
        || !request.selection.flavours.is_empty()
        || !request.selection.fields.is_empty()
        || !request.selection.paths.is_empty();
    if request.selection.all == filtered {
        return Err(Error::argument(
            "select explicit all or at least one ID/filter; all is exclusive",
        ));
    }
    if request.max_rows == 0 || !(1..=100).contains(&request.limit) {
        return Err(Error::argument(
            "max_rows must be positive; limit must be 1..=100",
        ));
    }
    if request.definition_files.is_empty() {
        return Err(Error::argument("supply an explicit definition set"));
    }

    let mut identity_request = request.clone();
    identity_request.cursor = None;
    let identity = hash(&(
        "named-query-5",
        env!("CARGO_PKG_VERSION"),
        engine.source_fingerprint(),
        &identity_request,
    ));
    let mut page = Page {
        format_version: 3,
        definition: request.definition.clone(),
        definition_file: None,
        input_identity: identity,
        evaluation_complete: true,
        unavailable_reason: None,
        summary: Summary {
            selected_roots: 0,
            binding_count: Some(0),
            known_bindings: 0,
        },
        records: vec![],
        has_more: false,
        next_cursor: None,
    };
    let start = cursor_position(request.cursor.as_deref(), &page.input_identity)?;

    let mut definitions = BTreeMap::new();
    let mut files = BTreeSet::new();
    for file in &request.definition_files {
        let path = engine
            .normalize_path(file)
            .map_err(|e| Error::argument(e.to_string()))?;
        if !files.insert(path.clone()) {
            return Err(Error::argument("duplicate normalized definition file"));
        }
        let Some(text) = engine.files().get(&path) else {
            return unavailable(page, format!("missing definition source: {path}"), start);
        };
        let set: DefinitionSet =
            serde_json::from_str(text).map_err(|e| Error::definition(&path, e.to_string()))?;
        if set.format_version != 1 {
            return Err(Error::definition(
                &path,
                "expected named-definition format_version 1",
            ));
        }
        for definition in set.definitions {
            if definition.id.trim().is_empty() {
                return Err(Error::definition(&path, "empty definition identity"));
            }
            let id = definition.id.clone();
            if definitions
                .insert(
                    id,
                    LoadedDefinition {
                        file: path.clone(),
                        definition,
                    },
                )
                .is_some()
            {
                return Err(Error::definition(
                    &path,
                    "duplicate definition identity across loaded set",
                ));
            }
        }
    }
    let selected_definition = definitions
        .get(&request.definition)
        .ok_or_else(|| Error::definition("", "unknown exact definition identity"))?;
    page.definition_file = Some(selected_definition.file.clone());
    validate_loaded_dependency_graph(&definitions)?;

    let (_, schema) = match engine.strict_project() {
        Ok(v) => v,
        Err(e) => return unavailable(page, format!("schema input unavailable: {e}"), start),
    };
    let order = reachable_definitions(&request.definition, &definitions)?;
    let mut compiled = BTreeMap::new();
    // Preserve the strict loaded-set behavior: unused definitions are still validated.
    for (id, loaded) in &definitions {
        compiled.insert(
            id.clone(),
            signature::compile(&loaded.file, &loaded.definition, &schema)?,
        );
    }

    let mut declared_inputs = BTreeSet::new();
    let mut declared_mappings = BTreeSet::new();
    for loaded in definitions.values() {
        for fact in &loaded.definition.external_facts {
            declared_inputs.insert(fact.input.clone());
        }
        for lookup in &loaded.definition.lookups {
            declared_mappings.insert(lookup.mapping.clone());
        }
    }
    let inputs = facts::inputs(&request, &declared_inputs, &declared_mappings)?;

    let (project, _, corpus) = match engine.corpus() {
        Ok(v) => v,
        Err(e) => return unavailable(page, format!("source input unavailable: {e}"), start),
    };
    validate_parameter_tree(
        &request.definition,
        request.parameters.clone(),
        &definitions,
        &corpus,
    )?;
    let diagnostics = crate::validate_corpus(&corpus, &schema);
    if diagnostics.iter().any(|d| {
        !d.is_missing_mid()
            && !matches!(
                d.code(),
                crate::DiagnosticCode::CodeMissing
                    | crate::DiagnosticCode::CodeAmbiguous
                    | crate::DiagnosticCode::CodeUnsupported
                    | crate::DiagnosticCode::EvaluationUnavailable
            )
    }) {
        return unavailable(page, "graph source prerequisites are invalid".into(), start);
    }
    let graph = prepare(&project, &corpus, &schema, engine.source_fingerprint())
        .map_err(|e| Error::definition(&selected_definition.file, e))?;
    for (id, loaded) in &definitions {
        let query = &mut compiled
            .get_mut(id)
            .expect("reachable definition is compiled")
            .query;
        signature::bind_items(query, &schema, &graph)
            .map_err(|e| Error::definition(&loaded.file, e))?;
        signature::validate_item_constants(query, &graph)
            .map_err(|e| Error::definition(&loaded.file, e))?;
    }

    let filters = ItemFilters::new(
        request.selection.flavours.clone(),
        request
            .selection
            .fields
            .iter()
            .map(|f| FieldFilter::new(&f.key, &f.value))
            .collect(),
        vec![],
        request.selection.paths.iter().map(Into::into).collect(),
        None,
    )
    .with_ids(request.selection.ids.clone());
    let roots = crate::filtered_items(&corpus, &schema, &filters)
        .map_err(|e| Error::argument(e.to_string()))?;
    let mut selected = BTreeMap::new();
    for item in roots {
        for id in &order {
            let definition = &definitions[id].definition;
            if !definition
                .root_flavours
                .iter()
                .any(|flavour| flavour == item.flavour())
            {
                return Err(Error::argument(format!(
                    "root {} does not satisfy definition {} root signature",
                    item.id(),
                    definition.id
                )));
            }
        }
        let root = Endpoint::item(item);
        selected.insert(root.iri(), (root, ItemSource::from(item.source())));
    }
    page.summary.selected_roots = selected.len();

    if let Some(id) = order.iter().find(|id| {
        definitions[*id]
            .definition
            .required_fact_sources
            .contains(&FactSource::Code)
    }) {
        page.evaluation_complete = false;
        page.unavailable_reason = Some(format!(
            "required code facts for named definition {id} were not supplied to this snapshot"
        ));
    }

    let store =
        Store::new().map_err(|e| Error::definition(&selected_definition.file, e.to_string()))?;
    for triple in &graph.triples {
        store
            .insert(&Quad::new(
                triple.subject.clone(),
                triple.predicate.clone(),
                triple.object.clone(),
                GraphName::DefaultGraph,
            ))
            .map_err(|e| Error::definition(&selected_definition.file, e.to_string()))?;
    }
    add_normalized_text(&store, &corpus, &schema, &selected_definition.file)?;

    // Validate every supplied snapshot record and mapping entry against the declarations that
    // claim it, including loaded definitions this execution does not reach, so malformed inputs
    // never depend on root selection or reachability. Only reachable definitions make an
    // unbound required input unavailable.
    let reachable = order.iter().collect::<BTreeSet<_>>();
    let mut prepared = BTreeMap::new();
    let mut input_unavailable = None;
    for (id, loaded) in &definitions {
        let validation = facts::prepare(&loaded.definition, &loaded.file, &corpus, &inputs)?;
        if reachable.contains(id)
            && let Some(reason) = validation.unavailable()
        {
            input_unavailable.get_or_insert_with(|| reason.to_owned());
        }
        prepared.insert(id.clone(), validation);
    }

    let mut rows = 0u32;
    let context = EvaluationContext {
        definitions: &definitions,
        compiled: &compiled,
        corpus: &corpus,
        graph: &graph,
        store: &store,
        max_rows: request.max_rows,
        prepared: &prepared,
    };
    let mut records = vec![];
    for (_, (root, source)) in selected {
        // Evaluate every selected root: an unavailable child input or a reached budget may make
        // overall evaluation incomplete, but each root's known bindings are still retained.
        let mut path = vec![request.definition.clone()];
        let mut derived = Derived::default();
        let mut state = EvaluationState {
            derived: &mut derived,
            observed_rows: &mut rows,
        };
        let result = evaluate_definition(
            &request.definition,
            request.parameters.clone(),
            &root,
            &mut path,
            &context,
            &mut state,
        )?;
        if !result.complete {
            page.evaluation_complete = false;
            if page.unavailable_reason.is_none() {
                page.unavailable_reason = result.unavailable_reason;
            }
        }
        let count = result.bindings.len();
        page.summary.known_bindings += count;
        records.push(Record::Root {
            root: root.clone(),
            source,
            binding_count: Some(count),
        });
        records.extend(result.bindings.into_iter().map(|binding| Record::Binding {
            root: root.clone(),
            binding,
        }));
    }
    // A complete evaluation still reports an unbound required input for an empty selection,
    // where no root was evaluated to surface it.
    if page.evaluation_complete
        && let Some(reason) = input_unavailable
    {
        page.evaluation_complete = false;
        page.unavailable_reason = Some(reason);
    }
    if !page.evaluation_complete {
        page.summary.binding_count = None;
        for record in &mut records {
            if let Record::Root { binding_count, .. } = record {
                *binding_count = None;
            }
        }
    } else {
        page.summary.binding_count = Some(page.summary.known_bindings);
    }
    paginate(
        page,
        records,
        start,
        request.limit,
        request.cursor.is_some(),
    )
}

pub(super) fn validate_loaded_dependency_graph(
    definitions: &BTreeMap<String, LoadedDefinition>,
) -> Result<(), Error> {
    for loaded in definitions.values() {
        let mut names = BTreeSet::new();
        for dependency in &loaded.definition.dependencies {
            validate_dependency(&loaded.definition, dependency, definitions)?;
            if !names.insert(dependency.name.as_str()) {
                return Err(Error::definition(
                    &loaded.file,
                    format!("duplicate dependency name {}", dependency.name),
                ));
            }
        }
    }

    fn visit(
        id: &str,
        definitions: &BTreeMap<String, LoadedDefinition>,
        states: &mut BTreeMap<String, bool>,
    ) -> Result<(), Error> {
        if let Some(visiting) = states.get(id) {
            if *visiting {
                let file = definitions
                    .get(id)
                    .map_or("", |loaded| loaded.file.as_str());
                return Err(Error::definition(file, "recursive named-query dependency"));
            }
            return Ok(());
        }
        let loaded = definitions
            .get(id)
            .ok_or_else(|| Error::definition("", format!("unknown named dependency {id}")))?;
        states.insert(id.to_owned(), true);
        for dependency in &loaded.definition.dependencies {
            visit(&dependency.definition, definitions, states).map_err(|mut error| {
                if error.source.is_none() {
                    error.source = Some(loaded.file.clone());
                }
                error
            })?;
        }
        states.insert(id.to_owned(), false);
        Ok(())
    }

    let mut states = BTreeMap::new();
    for id in definitions.keys() {
        visit(id, definitions, &mut states)?;
    }
    Ok(())
}

fn reachable_definitions(
    selected: &str,
    definitions: &BTreeMap<String, LoadedDefinition>,
) -> Result<Vec<String>, Error> {
    fn visit(
        id: &str,
        definitions: &BTreeMap<String, LoadedDefinition>,
        states: &mut BTreeMap<String, bool>,
        order: &mut Vec<String>,
    ) -> Result<(), Error> {
        if let Some(visiting) = states.get(id) {
            if *visiting {
                let file = definitions
                    .get(id)
                    .map_or("", |loaded| loaded.file.as_str());
                return Err(Error::definition(file, "recursive named-query dependency"));
            }
            return Ok(());
        }
        let loaded = definitions
            .get(id)
            .ok_or_else(|| Error::definition("", format!("unknown named dependency {id}")))?;
        states.insert(id.to_owned(), true);
        for dependency in &loaded.definition.dependencies {
            visit(&dependency.definition, definitions, states, order).map_err(|mut error| {
                if error.source.is_none() {
                    error.source = Some(loaded.file.clone());
                }
                error
            })?;
        }
        states.insert(id.to_owned(), false);
        order.push(id.to_owned());
        Ok(())
    }

    let mut states = BTreeMap::new();
    let mut order = Vec::new();
    visit(selected, definitions, &mut states, &mut order)?;
    Ok(order)
}

fn validate_dependency(
    parent: &Definition,
    dependency: &Dependency,
    definitions: &BTreeMap<String, LoadedDefinition>,
) -> Result<(), Error> {
    let parent_file = &definitions[&parent.id].file;
    if !crate::rules::valid_parameter_name(&dependency.name) {
        return Err(Error::definition(parent_file, "invalid dependency name"));
    }
    if dependency.root != "root" {
        return Err(Error::definition(
            parent_file,
            "dependency root must explicitly bind the caller-selected root",
        ));
    }
    let child = definitions.get(&dependency.definition).ok_or_else(|| {
        Error::definition(
            parent_file,
            format!("unknown named dependency {}", dependency.definition),
        )
    })?;
    let child_definition = &child.definition;
    let projections = &dependency.projections;
    let mut aliases = BTreeSet::new();
    for (column, alias) in &projections.columns {
        if !child_definition.columns.contains_key(column) {
            return Err(Error::definition(
                parent_file,
                format!("dependency {} has no column {column}", dependency.name),
            ));
        }
        validate_projection_alias(parent_file, alias, &mut aliases)?;
    }
    if let Some(alias) = &projections.contributors {
        if child_definition.source_columns.is_empty() {
            return Err(Error::definition(
                parent_file,
                format!(
                    "dependency {} has no projected contributors",
                    dependency.name
                ),
            ));
        }
        validate_projection_alias(parent_file, alias, &mut aliases)?;
    }
    if let Some(alias) = &projections.support {
        if child_definition.support_columns.is_empty() {
            return Err(Error::definition(
                parent_file,
                format!(
                    "dependency {} has no projected edge support",
                    dependency.name
                ),
            ));
        }
        validate_projection_alias(parent_file, alias, &mut aliases)?;
    }
    if let Some(alias) = &projections.facts {
        validate_projection_alias(parent_file, alias, &mut aliases)?;
    }
    for (child_parameter, parent_parameter) in &dependency.parameters {
        let Some(child_decl) = child_definition.parameters.get(child_parameter) else {
            return Err(Error::definition(
                parent_file,
                format!(
                    "dependency {} has no parameter {child_parameter}",
                    dependency.name
                ),
            ));
        };
        let Some(parent_decl) = parent.parameters.get(parent_parameter) else {
            return Err(Error::definition(
                parent_file,
                format!(
                    "dependency {} maps unknown parent parameter {parent_parameter}",
                    dependency.name
                ),
            ));
        };
        if child_decl.r#type != parent_decl.r#type {
            return Err(Error::definition(
                parent_file,
                format!(
                    "dependency {} parameter types do not match",
                    dependency.name
                ),
            ));
        }
    }
    for (name, parameter) in &child_definition.parameters {
        if !dependency.parameters.contains_key(name)
            && parameter.required
            && parameter.default.is_none()
        {
            return Err(Error::definition(
                parent_file,
                format!(
                    "dependency {} does not bind required parameter {name}",
                    dependency.name
                ),
            ));
        }
    }
    Ok(())
}

fn validate_projection_alias(
    file: &str,
    alias: &str,
    aliases: &mut BTreeSet<String>,
) -> Result<(), Error> {
    if !crate::rules::valid_parameter_name(alias) || !aliases.insert(alias.to_owned()) {
        return Err(Error::definition(
            file,
            "invalid or repeated dependency projection alias",
        ));
    }
    Ok(())
}

fn validate_parameter_tree(
    id: &str,
    supplied: Vec<BindingParameter>,
    definitions: &BTreeMap<String, LoadedDefinition>,
    corpus: &crate::Corpus,
) -> Result<(), Error> {
    let loaded = &definitions[id];
    let values = parameter_values(&loaded.file, &loaded.definition, &supplied, corpus)?;
    for dependency in &loaded.definition.dependencies {
        let child_parameters = dependency
            .parameters
            .iter()
            .filter_map(|(child, parent)| {
                values.get(parent).map(|value| BindingParameter {
                    name: child.clone(),
                    value: value.clone(),
                })
            })
            .collect();
        validate_parameter_tree(
            &dependency.definition,
            child_parameters,
            definitions,
            corpus,
        )?;
    }
    Ok(())
}

fn parameter_values(
    file: &str,
    definition: &Definition,
    supplied: &[BindingParameter],
    corpus: &crate::Corpus,
) -> Result<BTreeMap<String, ParameterValue>, Error> {
    let mut values = BTreeMap::new();
    for parameter in supplied {
        let declaration = definition
            .parameters
            .get(&parameter.name)
            .ok_or_else(|| Error::argument(format!("undeclared parameter {}", parameter.name)))?;
        if declaration.r#type != parameter.value.kind() {
            return Err(Error::argument(format!(
                "wrong type for parameter {}",
                parameter.name
            )));
        }
        parameter.value.term(corpus)?;
        if values
            .insert(parameter.name.clone(), parameter.value.clone())
            .is_some()
        {
            return Err(Error::argument(format!(
                "duplicate parameter {}",
                parameter.name
            )));
        }
    }
    for (name, declaration) in &definition.parameters {
        if values.contains_key(name) {
            continue;
        }
        if let Some(value) = &declaration.default {
            value
                .term(corpus)
                .map_err(|error| Error::definition(file, error.message))?;
            values.insert(name.clone(), value.clone());
        } else if declaration.required {
            return Err(Error::argument(format!(
                "missing required parameter {name}"
            )));
        }
    }
    Ok(values)
}

fn evaluate_definition(
    id: &str,
    supplied: Vec<BindingParameter>,
    root: &Endpoint,
    path: &mut Vec<String>,
    context: &EvaluationContext<'_>,
    state: &mut EvaluationState<'_>,
) -> Result<QueryEvaluation, Error> {
    let loaded = &context.definitions[id];
    let definition = &loaded.definition;
    let file = loaded.file.as_str();
    let item_id = match root {
        Endpoint::Item { id, .. } => id,
        _ => return Err(Error::argument("named-query child root must be an item")),
    };
    let item = crate::resolve_item(context.corpus, item_id)
        .map_err(|error| Error::argument(error.to_string()))?;
    if !definition
        .root_flavours
        .iter()
        .any(|flavour| flavour == item.flavour())
    {
        return Err(Error::argument(format!(
            "root {} does not satisfy definition {} root signature",
            item.id(),
            definition.id
        )));
    }
    let parameter_values = parameter_values(file, definition, &supplied, context.corpus)?;
    let scope = hash(&(
        "query-input-scope-1",
        id,
        root.iri(),
        &*path,
        &parameter_values,
    ));
    // A required source this host cannot supply makes the definition incomplete from the start;
    // known graph bindings and derived values are still retained without exact counts.
    let code_missing = definition.required_fact_sources.contains(&FactSource::Code);
    let mut result = QueryEvaluation {
        bindings: vec![],
        complete: !code_missing,
        unavailable_reason: code_missing.then(|| {
            format!(
                "required code facts for named definition {id} were not supplied to this snapshot"
            )
        }),
    };

    for dependency in &definition.dependencies {
        let child_parameters = dependency
            .parameters
            .iter()
            .filter_map(|(child, parent)| {
                parameter_values.get(parent).map(|value| BindingParameter {
                    name: child.clone(),
                    value: value.clone(),
                })
            })
            .collect();
        path.push(dependency.name.clone());
        path.push(dependency.definition.clone());
        let child = evaluate_definition(
            &dependency.definition,
            child_parameters,
            root,
            &mut *path,
            context,
            &mut *state,
        )?;
        path.pop();
        path.pop();
        if !child.complete {
            result.complete = false;
            if result.unavailable_reason.is_none() {
                result.unavailable_reason = child.unavailable_reason;
            }
        }
        materialize_dependency(
            context.store,
            file,
            root,
            &scope,
            dependency,
            &child.bindings,
            state.derived,
        )?;
    }

    let mut query = context.compiled[id].query.clone();
    signature::scope_inputs(&mut query, &definition.dependencies, &scope);
    signature::add_parameter_projection(&mut query, definition.parameters.keys());
    let mut invocation = parameter_values
        .iter()
        .map(|(name, value)| {
            value
                .term(context.corpus)
                .map(|term| (Variable::new(name).expect("validated parameter"), term))
                .map_err(|error| Error::definition(file, error.message))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    invocation.insert(
        Variable::new("root").unwrap(),
        NamedNode::new(root.iri())
            .expect("canonical root IRI")
            .into(),
    );

    // Unordered backend iteration differs between native and WASM. Order the visible
    // solution terms in the backend before taking an observed-row prefix. RDF numeric
    // comparisons can tie distinct terms, so lexical/language/datatype break those ties.
    let Query::Select { pattern, .. } = &mut query else {
        unreachable!("compiled SELECT")
    };
    let mut variables = BTreeSet::new();
    pattern.on_in_scope_variable(|v| {
        variables.insert(v.clone());
    });
    let expression = variables
        .into_iter()
        .flat_map(|v| {
            let value = Expression::Variable(v);
            [
                OrderExpression::Asc(value.clone()),
                OrderExpression::Asc(Expression::FunctionCall(Function::Str, vec![value.clone()])),
                OrderExpression::Asc(Expression::FunctionCall(
                    Function::Lang,
                    vec![value.clone()],
                )),
                OrderExpression::Asc(Expression::FunctionCall(Function::Datatype, vec![value])),
            ]
        })
        .collect();
    let inner = std::mem::replace(pattern, GraphPattern::Bgp { patterns: vec![] });
    *pattern = GraphPattern::OrderBy {
        inner: Box::new(inner),
        expression,
    };
    let evaluator = SparqlEvaluator::new().without_optimizations();
    #[cfg(not(target_family = "wasm"))]
    let evaluator = evaluator.without_default_http_service_handler();
    let mut prepared = evaluator.for_query(query);
    for (name, value) in &invocation {
        prepared = prepared.substitute_variable(name.clone(), value.clone());
    }
    let solutions = match prepared.on_store(context.store).execute() {
        Ok(QueryResults::Solutions(solutions)) => solutions,
        Ok(_) => unreachable!("compiled SELECT"),
        Err(error) => {
            result.complete = false;
            result
                .unavailable_reason
                .get_or_insert_with(|| format!("backend evaluation unavailable: {error}"));
            return Ok(result);
        }
    };

    let mut tuples: BTreeMap<String, Binding> = BTreeMap::new();
    for solution in solutions {
        let solution = match solution {
            Ok(solution) => solution,
            Err(error) => {
                result.complete = false;
                result
                    .unavailable_reason
                    .get_or_insert_with(|| format!("backend evaluation unavailable: {error}"));
                break;
            }
        };
        if *state.observed_rows == context.max_rows {
            result.complete = false;
            result.unavailable_reason.get_or_insert_with(|| {
                "observed solution-row budget reached; no exact count or negative conclusion".into()
            });
            break;
        }
        *state.observed_rows += 1;
        let root_iri: Term = NamedNode::new(root.iri()).unwrap().into();
        if solution.get("root") != Some(&root_iri) {
            return Err(Error::definition(
                file,
                "SELECT changed its explicitly bound root",
            ));
        }

        let mut values = BTreeMap::new();
        for (name, column) in &definition.columns {
            if context.compiled[id].computed.contains(name) {
                // Derived columns are supplied by declared facts, mappings or aggregation.
                values.insert(name.clone(), None);
                continue;
            }
            let value = solution
                .get(name.as_str())
                .map(|term| typed(term, context.graph, &state.derived.support))
                .transpose()
                .map_err(|error| Error::definition(file, error))?;
            if let Some(value) = &value {
                if value.kind() != column.r#type {
                    return Err(Error::definition(
                        file,
                        format!("column {name} returned incompatible type"),
                    ));
                }
                if let Value::Item {
                    endpoint: Endpoint::Item { id, .. },
                    ..
                } = value
                {
                    let item = crate::resolve_item(context.corpus, id).expect("graph item");
                    if !column.flavours.is_empty()
                        && !column
                            .flavours
                            .iter()
                            .any(|flavour| flavour == item.flavour())
                    {
                        return Err(Error::definition(
                            file,
                            format!("column {name} returned incompatible flavour"),
                        ));
                    }
                }
            } else if !column.optional {
                return Err(Error::definition(
                    file,
                    format!("required column {name} is absent; declare optional bindings"),
                ));
            }
            values.insert(name.clone(), value);
        }
        let mut contributors = BTreeMap::new();
        let mut support = BTreeMap::new();
        for name in &definition.support_columns {
            if let Some(term) = solution.get(name.as_str()) {
                let Term::NamedNode(id) = term else {
                    return Err(Error::definition(
                        file,
                        "support column must bind a canonical edge",
                    ));
                };
                let edge = context
                    .graph
                    .support
                    .get(id.as_str())
                    .or_else(|| state.derived.support.get(id.as_str()))
                    .ok_or_else(|| Error::definition(file, "unknown canonical edge support"))?;
                support.insert(id.as_str().to_owned(), edge.clone());
            }
        }
        for name in &definition.source_columns {
            if let Some(term) = solution.get(name.as_str()) {
                let value = typed(term, context.graph, &state.derived.support)
                    .map_err(|error| Error::definition(file, error))?;
                if !matches!(value, Value::Item { .. }) {
                    return Err(Error::definition(
                        file,
                        "source column must bind a canonical item",
                    ));
                }
                contributors.insert(hash(&value), value);
            }
        }
        let mut facts = BTreeMap::new();
        for name in &definition.facts_columns {
            if let Some(term) = solution.get(name.as_str()) {
                let Term::NamedNode(node) = term else {
                    return Err(Error::definition(
                        file,
                        "fact provenance column must bind a canonical node",
                    ));
                };
                let provenance = state
                    .derived
                    .facts
                    .get(node.as_str())
                    .ok_or_else(|| Error::definition(file, "unknown fact provenance"))?;
                // Key merged provenance by content so an equivalent fact projected twice collapses.
                facts.insert(hash(provenance), provenance.clone());
            }
        }
        let key = serde_json::to_string(&values).expect("typed tuple");
        let entry = tuples.entry(key).or_insert_with(|| Binding {
            values,
            contributors: vec![],
            support: vec![],
            facts: vec![],
        });
        for contributor in &entry.contributors {
            contributors.insert(hash(contributor), contributor.clone());
        }
        for supported in &entry.support {
            support.insert(
                format!("urn:mara:query:edge:{}", hash(&supported.edge)),
                supported.clone(),
            );
        }
        for provenance in &entry.facts {
            facts.insert(hash(provenance), provenance.clone());
        }
        entry.contributors = contributors.into_values().collect();
        entry.support = support.into_values().collect();
        entry.facts = facts.into_values().collect();
    }
    result.bindings = tuples.into_values().collect();
    if let Some(reason) = facts::apply(
        definition,
        &context.prepared[id],
        result.complete,
        &mut result.bindings,
    )? {
        result.complete = false;
        result.unavailable_reason.get_or_insert(reason);
    }
    Ok(result)
}

fn materialize_dependency(
    store: &Store,
    file: &str,
    root: &Endpoint,
    scope: &str,
    dependency: &Dependency,
    bindings: &[Binding],
    derived: &mut Derived,
) -> Result<(), Error> {
    let root_iri = NamedNode::new(root.iri()).expect("canonical root IRI");
    let relation = NamedNode::new(format!("urn:mara:query:input:{scope}:{}", dependency.name))
        .expect("validated dependency IRI");
    for (index, binding) in bindings.iter().enumerate() {
        let row = BlankNode::new(format!(
            "mara_query_{scope}_{}_{}",
            hash(&dependency.name),
            index
        ))
        .expect("safe deterministic query row identity");
        insert(
            store,
            file,
            root_iri.clone(),
            relation.clone(),
            row.clone().into(),
        )?;
        for (child_column, alias) in &dependency.projections.columns {
            if let Some(Some(value)) = binding.values.get(child_column) {
                let predicate = column_predicate(scope, &dependency.name, alias);
                insert(
                    store,
                    file,
                    row.clone(),
                    predicate,
                    value_term(file, value)?,
                )?;
            }
        }
        if let Some(alias) = &dependency.projections.contributors {
            for contributor in &binding.contributors {
                let predicate = column_predicate(scope, &dependency.name, alias);
                insert(
                    store,
                    file,
                    row.clone(),
                    predicate,
                    value_term(file, contributor)?,
                )?;
            }
        }
        if let Some(alias) = &dependency.projections.support {
            for supported in &binding.support {
                let predicate = column_predicate(scope, &dependency.name, alias);
                let id = format!("urn:mara:query:edge:{}", hash(&supported.edge));
                derived.support.insert(id.clone(), supported.clone());
                insert(
                    store,
                    file,
                    row.clone(),
                    predicate,
                    NamedNode::new(id).expect("canonical support IRI").into(),
                )?;
            }
        }
        if let Some(alias) = &dependency.projections.facts {
            for (fact_index, provenance) in binding.facts.iter().enumerate() {
                let predicate = column_predicate(scope, &dependency.name, alias);
                let id = format!(
                    "urn:mara:query:fact:{scope}:{}:{index}:{fact_index}",
                    hash(&dependency.name)
                );
                derived.facts.insert(id.clone(), provenance.clone());
                insert(
                    store,
                    file,
                    row.clone(),
                    predicate,
                    NamedNode::new(id)
                        .expect("canonical fact provenance IRI")
                        .into(),
                )?;
            }
        }
    }
    Ok(())
}

fn column_predicate(scope: &str, dependency: &str, column: &str) -> NamedNode {
    NamedNode::new(format!(
        "urn:mara:query:column:{scope}:{dependency}:{column}"
    ))
    .expect("validated dependency column IRI")
}

fn value_term(file: &str, value: &Value) -> Result<Term, Error> {
    use oxrdf::vocab::xsd;
    Ok(match value {
        Value::Item { endpoint, .. } => NamedNode::new(endpoint.iri())
            .expect("canonical item IRI")
            .into(),
        Value::Code { reference } => NamedNode::new(
            Endpoint::Code {
                reference: reference.clone(),
            }
            .iri(),
        )
        .expect("canonical code IRI")
        .into(),
        Value::External { address } => NamedNode::new(
            Endpoint::External {
                address: address.clone(),
            }
            .iri(),
        )
        .expect("canonical external IRI")
        .into(),
        Value::Iri { value } => NamedNode::new(value)
            .map_err(|error| Error::definition(file, error.to_string()))?
            .into(),
        Value::Text { value, language } => match language {
            Some(language) => Literal::new_language_tagged_literal(value, language)
                .map_err(|error| Error::definition(file, error.to_string()))?
                .into(),
            None => Literal::new_simple_literal(value).into(),
        },
        Value::Integer { value } => {
            Literal::new_typed_literal(value.to_string(), xsd::INTEGER).into()
        }
        Value::Number { value } => Literal::new_typed_literal(value.clone(), xsd::DOUBLE).into(),
        Value::Boolean { value } => {
            Literal::new_typed_literal(value.to_string(), xsd::BOOLEAN).into()
        }
        Value::Edge { edge } => NamedNode::new(format!("urn:mara:query:edge:{}", hash(edge)))
            .expect("canonical support IRI")
            .into(),
    })
}

fn insert(
    store: &Store,
    file: &str,
    subject: impl Into<NamedOrBlankNode>,
    predicate: NamedNode,
    object: Term,
) -> Result<(), Error> {
    store
        .insert(&Quad::new(
            subject.into(),
            predicate,
            object,
            GraphName::DefaultGraph,
        ))
        .map_err(|error| Error::definition(file, error.to_string()))?;
    Ok(())
}

fn add_normalized_text(
    store: &Store,
    corpus: &crate::Corpus,
    schema: &crate::Schema,
    file: &str,
) -> Result<(), Error> {
    for item in corpus.items() {
        let iri = NamedNode::new(Endpoint::item(item).iri()).expect("canonical item IRI");
        insert(
            store,
            file,
            iri.clone(),
            NamedNode::new("urn:mara:query:nfc-casefold:title").unwrap(),
            Literal::new_simple_literal(crate::query::canonical_text(item.title())).into(),
        )?;
        let fields = &schema.flavours()[item.flavour()].fields;
        for metadata in item.metadata() {
            let Some(definition) = fields.get(metadata.key()) else {
                continue;
            };
            if !matches!(
                definition.field_type,
                crate::FieldType::String | crate::FieldType::Enum
            ) {
                continue;
            }
            insert(
                store,
                file,
                iri.clone(),
                NamedNode::new(format!(
                    "urn:mara:query:nfc-casefold:field:{}",
                    metadata.key()
                ))
                .expect("schema field name is an IRI component"),
                Literal::new_simple_literal(crate::query::canonical_text(metadata.value())).into(),
            )?;
        }
    }
    Ok(())
}
