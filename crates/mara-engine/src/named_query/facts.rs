//! Supplied external fact snapshots, typed lookup mappings and precedence aggregation.
//! Inputs are validated data; nothing is fetched, executed or persisted here.
use super::*;
use crate::Corpus;

/// Validated request-level fact inputs. Labels are opaque; full content is hashed into identity.
pub(super) struct Inputs<'a> {
    pub snapshots: BTreeMap<String, &'a FactSnapshot>,
    pub mappings: BTreeMap<String, &'a TypedMapping>,
}

/// Validate the declared format-5 facts/lookups/aggregate and return derived output columns.
/// Derived columns must be declared but are not part of the SELECT tuple signature.
pub(super) fn validate_derived(definition: &Definition) -> Result<BTreeSet<String>, String> {
    let declared = definition.columns.keys().cloned().collect::<BTreeSet<_>>();
    let mut projected = declared.clone();
    let mut computed = BTreeSet::new();
    if !definition.external_facts.is_empty()
        && !definition
            .required_fact_sources
            .contains(&FactSource::External)
    {
        return Err("external fact inputs require the external fact source".into());
    }
    if definition
        .required_fact_sources
        .contains(&FactSource::External)
        && definition.external_facts.is_empty()
    {
        return Err("the external fact source requires at least one declared fact input".into());
    }

    let mut inputs = BTreeSet::new();
    for fact in &definition.external_facts {
        if !crate::rules::valid_parameter_name(&fact.input) || !inputs.insert(fact.input.clone()) {
            return Err("invalid or repeated external fact input name".into());
        }
        if fact.keys.is_empty() {
            return Err("external facts require declared key columns".into());
        }
        let mut keys = BTreeSet::new();
        for key in &fact.keys {
            if !keys.insert(key.clone()) {
                return Err("repeated external fact key column".into());
            }
            if !projected.contains(key) {
                return Err(format!(
                    "external fact key column {key} must be a projected tuple column"
                ));
            }
        }
        if fact.fields.is_empty() {
            return Err("external facts require declared fields".into());
        }
        let mut outputs = BTreeSet::new();
        for (field, column) in &fact.fields {
            if field.is_empty() || !projected.contains(column) || !outputs.insert(column.clone()) {
                return Err(format!(
                    "external fact field {field} must map one projected column to one derived output"
                ));
            }
        }
        let missing = fact.missing.keys().cloned().collect::<BTreeSet<_>>();
        if missing != outputs {
            return Err(
                "external fact missing values must declare every field output column".into(),
            );
        }
        for (column, value) in &fact.missing {
            if value.kind() != definition.columns[column].r#type {
                return Err(format!(
                    "external fact missing value for {column} has the wrong type"
                ));
            }
        }
        for column in outputs {
            projected.remove(&column);
            computed.insert(column);
        }
    }

    let mut mappings = BTreeSet::new();
    for lookup in &definition.lookups {
        if !crate::rules::valid_parameter_name(&lookup.mapping)
            || !mappings.insert(lookup.mapping.clone())
        {
            return Err("invalid or repeated lookup mapping name".into());
        }
        if lookup.keys.is_empty() {
            return Err("lookup mappings require declared key columns".into());
        }
        let mut keys = BTreeSet::new();
        for key in &lookup.keys {
            if !keys.insert(key.clone()) || !declared.contains(key) {
                return Err(format!("unknown or repeated lookup key column {key}"));
            }
        }
        if !projected.contains(&lookup.output) {
            return Err(format!(
                "lookup output {} must be one declared derived column",
                lookup.output
            ));
        }
        projected.remove(&lookup.output);
        computed.insert(lookup.output.clone());
    }

    if let Some(aggregate) = &definition.aggregate {
        let rank = definition
            .columns
            .get(&aggregate.rank)
            .ok_or_else(|| "aggregate rank column must be declared".to_string())?;
        let mut groups = BTreeSet::new();
        for column in &aggregate.group_by {
            if !groups.insert(column.clone()) || !declared.contains(column) {
                return Err(format!(
                    "unknown or repeated aggregate group column {column}"
                ));
            }
        }
        let mut retain = BTreeSet::new();
        for column in &aggregate.retain {
            if !retain.insert(column.clone()) || !declared.contains(column) {
                return Err(format!(
                    "unknown or repeated aggregate retain column {column}"
                ));
            }
        }
        if aggregate.precedence.is_empty() {
            return Err("aggregate precedence must declare at least one value".into());
        }
        let mut ranked = BTreeSet::new();
        for value in &aggregate.precedence {
            if value.kind() != rank.r#type || !ranked.insert(canonical_value_kind(value)) {
                return Err(
                    "aggregate precedence values must be unique and match the rank type".into(),
                );
            }
        }
        if aggregate.empty.kind() != rank.r#type {
            return Err("aggregate empty value must match the rank column type".into());
        }
        if aggregate.group_by.contains(&aggregate.count)
            || aggregate.retain.contains(&aggregate.count)
            || aggregate.count == aggregate.rank
        {
            return Err(
                "aggregate count column must be independent of groups, rank and retain".into(),
            );
        }
        if !projected.contains(&aggregate.count)
            || definition.columns[&aggregate.count].r#type != ValueType::Integer
        {
            return Err("aggregate count must be one declared derived integer column".into());
        }
        projected.remove(&aggregate.count);
        computed.insert(aggregate.count.clone());
    }
    // Fact keys are read before any lookup or aggregate runs, so they must remain in the
    // SELECT signature; a key that any declaration derives can never be populated.
    for fact in &definition.external_facts {
        for key in &fact.keys {
            if computed.contains(key) {
                return Err(format!(
                    "external fact key column {key} must be a projected tuple column"
                ));
            }
        }
    }
    for lookup in &definition.lookups {
        if lookup.keys.contains(&lookup.output) {
            return Err("a lookup key must not be its own output column".into());
        }
    }
    Ok(computed)
}
fn canonical_value_kind(value: &ParameterValue) -> String {
    serde_json::to_string(value).expect("typed value serializes")
}

/// Validate request-supplied snapshots/mappings against declared names and reject duplicates.
pub(super) fn inputs<'a>(
    request: &'a Request,
    declared_inputs: &BTreeSet<String>,
    declared_mappings: &BTreeSet<String>,
) -> Result<Inputs<'a>, Error> {
    let mut snapshots = BTreeMap::new();
    for snapshot in &request.fact_snapshots {
        if !crate::rules::valid_parameter_name(&snapshot.input)
            || snapshot.identity.trim().is_empty()
            || !declared_inputs.contains(&snapshot.input)
        {
            return Err(Error::argument(format!(
                "unknown, invalid or repeated fact input {}",
                snapshot.input
            )));
        }
        if snapshots.insert(snapshot.input.clone(), snapshot).is_some() {
            return Err(Error::argument(format!(
                "duplicate fact input {}",
                snapshot.input
            )));
        }
    }
    let mut mappings = BTreeMap::new();
    for mapping in &request.mappings {
        if !crate::rules::valid_parameter_name(&mapping.mapping)
            || !declared_mappings.contains(&mapping.mapping)
        {
            return Err(Error::argument(format!(
                "unknown or invalid mapping {}",
                mapping.mapping
            )));
        }
        if mappings.insert(mapping.mapping.clone(), mapping).is_some() {
            return Err(Error::argument(format!(
                "duplicate mapping {}",
                mapping.mapping
            )));
        }
    }
    Ok(Inputs {
        snapshots,
        mappings,
    })
}

/// Overlay facts, apply lookups and aggregate one definition's returned bindings.
/// Validated, selection-independent inputs for one definition's declared facts and mappings.
pub(super) struct Prepared<'a> {
    facts: Vec<Option<PreparedFact<'a>>>,
    lookups: Vec<Option<PreparedLookup>>,
    aggregate: Option<PreparedAggregate>,
    unavailable: Option<String>,
}
struct PreparedFact<'a> {
    snapshot: &'a FactSnapshot,
    records: BTreeMap<String, (usize, BTreeMap<String, Value>)>,
    coverage: BTreeSet<String>,
    missing: BTreeMap<String, Value>,
}
struct PreparedLookup {
    entries: BTreeMap<String, Value>,
    fallback: Option<Value>,
}
struct PreparedAggregate {
    precedence: Vec<String>,
    empty: Value,
}
impl Prepared<'_> {
    pub(super) fn unavailable(&self) -> Option<&str> {
        self.unavailable.as_deref()
    }
}

/// Validate every supplied record and mapping entry this definition declares, independently of
/// root selection, and resolve its definition-declared missing/empty/precedence values.
pub(super) fn prepare<'a>(
    definition: &Definition,
    file: &str,
    corpus: &Corpus,
    inputs: &Inputs<'a>,
) -> Result<Prepared<'a>, Error> {
    let mut unavailable = None;
    let mut facts = Vec::new();
    for fact in &definition.external_facts {
        let Some(snapshot) = inputs.snapshots.get(&fact.input) else {
            unavailable.get_or_insert(format!(
                "required external fact input {} was not supplied",
                fact.input
            ));
            facts.push(None);
            continue;
        };
        let mut records = BTreeMap::new();
        for (index, record) in snapshot.records.iter().enumerate() {
            let key = record_key(
                &record.keys,
                &fact.keys,
                &fact.input,
                corpus,
                &definition.columns,
            )?;
            // Every supplied record is validated, including records that no returned
            // tuple selects, so malformed snapshots never depend on root selection.
            if record.fields.len() != fact.fields.len()
                || fact
                    .fields
                    .keys()
                    .any(|field| !record.fields.contains_key(field))
            {
                return Err(Error::argument(format!(
                    "external fact input {} record must declare exactly the declared fields",
                    fact.input
                )));
            }
            let mut fields = BTreeMap::new();
            for (field, column) in &fact.fields {
                let value = record.fields[field].typed(corpus).map_err(|error| {
                    Error::argument(format!("external fact field {field}: {}", error.message))
                })?;
                check_column(definition, column, &value, corpus)?;
                fields.insert(field.clone(), value);
            }
            if records.insert(canonical(&key), (index, fields)).is_some() {
                return Err(Error::argument(format!(
                    "external fact input {} has conflicting records for one key",
                    fact.input
                )));
            }
        }
        let mut coverage = BTreeSet::new();
        for entry in &snapshot.coverage {
            coverage.insert(canonical(&record_key(
                entry,
                &fact.keys,
                &fact.input,
                corpus,
                &definition.columns,
            )?));
        }
        let mut missing = BTreeMap::new();
        for (column, raw) in &fact.missing {
            missing.insert(
                column.clone(),
                definition_value(definition, column, raw, corpus, file)?,
            );
        }
        facts.push(Some(PreparedFact {
            snapshot,
            records,
            coverage,
            missing,
        }));
    }

    let mut lookups = Vec::new();
    for lookup in &definition.lookups {
        let Some(mapping) = inputs.mappings.get(&lookup.mapping) else {
            unavailable.get_or_insert(format!(
                "required lookup mapping {} was not supplied",
                lookup.mapping
            ));
            lookups.push(None);
            continue;
        };
        let output = definition.columns[&lookup.output].r#type;
        let mut entries = BTreeMap::new();
        for entry in &mapping.entries {
            let key = record_key(
                &entry.keys,
                &lookup.keys,
                &lookup.mapping,
                corpus,
                &definition.columns,
            )?;
            if entry.value.kind() != output {
                return Err(Error::argument(format!(
                    "mapping {} entry value does not match {} type",
                    lookup.mapping, lookup.output
                )));
            }
            let value = entry.value.typed(corpus).map_err(|error| {
                Error::argument(format!(
                    "mapping {} value: {}",
                    lookup.mapping, error.message
                ))
            })?;
            check_column(definition, &lookup.output, &value, corpus)?;
            if entries.insert(canonical(&key), value).is_some() {
                return Err(Error::argument(format!(
                    "mapping {} has conflicting duplicate keys",
                    lookup.mapping
                )));
            }
        }
        let fallback = match &mapping.fallback {
            Some(raw) => {
                if raw.kind() != output {
                    return Err(Error::argument(format!(
                        "mapping {} fallback does not match {} type",
                        lookup.mapping, lookup.output
                    )));
                }
                let value = raw.typed(corpus).map_err(|error| {
                    Error::argument(format!(
                        "mapping {} fallback: {}",
                        lookup.mapping, error.message
                    ))
                })?;
                check_column(definition, &lookup.output, &value, corpus)?;
                Some(value)
            }
            None => None,
        };
        lookups.push(Some(PreparedLookup { entries, fallback }));
    }

    let aggregate = match &definition.aggregate {
        Some(aggregate) => {
            // Precedence must not give one resolved typed value two positions: item ID/MID
            // aliases and signed zero belong to one canonical identity.
            let mut precedence = Vec::new();
            let mut ranked = BTreeSet::new();
            for value in &aggregate.precedence {
                let resolved = definition_value(definition, &aggregate.rank, value, corpus, file)?;
                let canonical = serde_json::to_string(&resolved).expect("typed value serializes");
                if !ranked.insert(canonical.clone()) {
                    return Err(Error::definition(
                        file,
                        "aggregate precedence repeats one resolved value",
                    ));
                }
                precedence.push(canonical);
            }
            Some(PreparedAggregate {
                precedence,
                empty: definition_value(
                    definition,
                    &aggregate.rank,
                    &aggregate.empty,
                    corpus,
                    file,
                )?,
            })
        }
        None => None,
    };
    Ok(Prepared {
        facts,
        lookups,
        aggregate,
        unavailable,
    })
}

/// Overlay prepared facts, apply lookups and aggregate one definition's returned bindings.
/// A returned reason means the evaluation is incomplete; known bindings are retained.
pub(super) fn apply(
    definition: &Definition,
    prepared: &Prepared<'_>,
    complete: bool,
    bindings: &mut Vec<Binding>,
) -> Result<Option<String>, Error> {
    let mut unavailable = prepared.unavailable.clone();
    for (fact, fact_data) in definition.external_facts.iter().zip(&prepared.facts) {
        let Some(fact_data) = fact_data else {
            continue;
        };
        for binding in bindings.iter_mut() {
            let Some(key) = row_key(binding, &fact.keys) else {
                unavailable.get_or_insert(format!(
                    "external fact input {} key columns are not bound to one tuple row",
                    fact.input
                ));
                continue;
            };
            let key_string = canonical(&key);
            // Declared coverage bounds what the snapshot asserts. A supplied record outside it
            // stays known evidence, but the required key is still unavailable for completeness.
            let covered = fact_data.coverage.contains(&key_string);
            if !covered {
                unavailable.get_or_insert(format!(
                    "external fact input {} does not cover a required key",
                    fact.input
                ));
            }
            if let Some((index, fields)) = fact_data.records.get(&key_string) {
                for (field, column) in &fact.fields {
                    binding
                        .values
                        .insert(column.clone(), Some(fields[field].clone()));
                }
                binding.facts.push(FactSupport {
                    input: fact.input.clone(),
                    identity: fact_data.snapshot.identity.clone(),
                    key,
                    record: Some(*index),
                });
            } else if covered {
                for (column, value) in &fact_data.missing {
                    binding.values.insert(column.clone(), Some(value.clone()));
                }
                binding.facts.push(FactSupport {
                    input: fact.input.clone(),
                    identity: fact_data.snapshot.identity.clone(),
                    key,
                    record: None,
                });
            }
        }
    }

    for (lookup, lookup_data) in definition.lookups.iter().zip(&prepared.lookups) {
        let Some(lookup_data) = lookup_data else {
            continue;
        };
        for binding in bindings.iter_mut() {
            let Some(key) = row_key(binding, &lookup.keys) else {
                unavailable.get_or_insert(format!(
                    "lookup mapping {} key columns are not bound to one tuple row",
                    lookup.mapping
                ));
                continue;
            };
            let value = lookup_data
                .entries
                .get(&canonical(&key))
                .cloned()
                .or_else(|| lookup_data.fallback.clone());
            match value {
                Some(value) => {
                    binding.values.insert(lookup.output.clone(), Some(value));
                }
                None => {
                    unavailable.get_or_insert(format!(
                        "mapping {} has no entry for a required key",
                        lookup.mapping
                    ));
                }
            }
        }
    }

    if let (Some(aggregate), Some(prepared)) = (&definition.aggregate, &prepared.aggregate) {
        let mut groups: BTreeMap<String, Vec<Binding>> = BTreeMap::new();
        for binding in bindings.drain(..) {
            match row_key(&binding, &aggregate.group_by) {
                Some(key) => groups.entry(canonical(&key)).or_default().push(binding),
                None => {
                    unavailable.get_or_insert(
                        "aggregate group columns are not bound to one tuple row".into(),
                    );
                    groups
                        .entry(format!("unbound:{}", canonical(&binding.values)))
                        .or_default()
                        .push(binding);
                }
            }
        }
        if groups.is_empty() {
            // The declared empty outcome holds only when the evaluation establishes emptiness.
            // A truncated or otherwise unavailable evaluation must not assert it.
            if complete && unavailable.is_none() {
                let mut values = BTreeMap::new();
                for name in definition.columns.keys() {
                    values.insert(
                        name.clone(),
                        if name == &aggregate.rank {
                            Some(prepared.empty.clone())
                        } else if name == &aggregate.count {
                            Some(Value::Integer { value: 0 })
                        } else {
                            None
                        },
                    );
                }
                bindings.push(Binding {
                    values,
                    contributors: vec![],
                    support: vec![],
                    facts: vec![],
                });
            }
            return Ok(unavailable);
        }
        // Rank every group before emitting counts, so a truncated or unavailable evaluation
        // never asserts an exact member count.
        let mut selected: Vec<(usize, Vec<Binding>)> = Vec::new();
        for group in groups.into_values() {
            let mut best: Option<(usize, usize)> = None;
            for (index, binding) in group.iter().enumerate() {
                let Some(rank) = binding.values.get(&aggregate.rank).and_then(Option::as_ref)
                else {
                    unavailable.get_or_insert(
                        "aggregate rank column is not bound to one tuple row".into(),
                    );
                    continue;
                };
                let ranked = serde_json::to_string(rank).expect("typed value serializes");
                let Some(position) = prepared
                    .precedence
                    .iter()
                    .position(|value| value == &ranked)
                else {
                    unavailable.get_or_insert(
                        "aggregate precedence does not rank a returned value".into(),
                    );
                    continue;
                };
                if best.is_none_or(|(current, _)| position < current) {
                    best = Some((position, index));
                }
            }
            selected.push((best.map_or(0, |(_, index)| index), group));
        }
        let counts_exact = complete && unavailable.is_none();
        for (member, group) in selected {
            let selected = &group[member];
            let mut values = BTreeMap::new();
            for name in definition.columns.keys() {
                let keep = aggregate.group_by.contains(name)
                    || aggregate.retain.contains(name)
                    || name == &aggregate.rank;
                values.insert(
                    name.clone(),
                    if name == &aggregate.count {
                        counts_exact.then_some(Value::Integer {
                            value: group.len() as i64,
                        })
                    } else if keep {
                        selected.values.get(name).cloned().flatten()
                    } else {
                        None
                    },
                );
            }
            let mut support = BTreeMap::new();
            let mut contributors = BTreeMap::new();
            let mut facts = Vec::new();
            for binding in &group {
                for supported in &binding.support {
                    support.insert(
                        format!("urn:mara:query:edge:{}", hash(&supported.edge)),
                        supported.clone(),
                    );
                }
                for contributor in &binding.contributors {
                    contributors.insert(hash(contributor), contributor.clone());
                }
                for provenance in &binding.facts {
                    if !facts.contains(provenance) {
                        facts.push(provenance.clone());
                    }
                }
            }
            bindings.push(Binding {
                values,
                contributors: contributors.into_values().collect(),
                support: support.into_values().collect(),
                facts,
            });
        }
    }
    Ok(unavailable)
}

fn canonical(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("typed value serializes")
}

/// Reject records/coverage keys that do not declare exactly the declared typed key columns.
fn record_key(
    supplied: &BTreeMap<String, ParameterValue>,
    keys: &[String],
    label: &str,
    corpus: &Corpus,
    columns: &BTreeMap<String, Column>,
) -> Result<BTreeMap<String, Value>, Error> {
    if supplied.len() != keys.len() || keys.iter().any(|key| !supplied.contains_key(key)) {
        return Err(Error::argument(format!(
            "{label} keys must declare exactly the declared key columns"
        )));
    }
    let mut key = BTreeMap::new();
    for name in keys {
        let value = supplied[name]
            .typed(corpus)
            .map_err(|error| Error::argument(format!("{label} key {name}: {}", error.message)))?;
        column_problem(&columns[name], name, &value, corpus)
            .map_err(|message| Error::argument(format!("{label} key {name}: {message}")))?;
        key.insert(name.clone(), value);
    }
    Ok(key)
}

fn row_key(binding: &Binding, keys: &[String]) -> Option<BTreeMap<String, Value>> {
    let mut key = BTreeMap::new();
    for name in keys {
        let value = binding.values.get(name)?.clone()?;
        key.insert(name.clone(), value);
    }
    Some(key)
}

fn check_column(
    definition: &Definition,
    column: &str,
    value: &Value,
    corpus: &Corpus,
) -> Result<(), Error> {
    column_problem(&definition.columns[column], column, value, corpus)
        .map_err(|message| Error::argument(format!("derived {message}")))
}

/// Type and flavour checks shared by supplied and definition-declared derived values.
fn column_problem(
    declaration: &Column,
    column: &str,
    value: &Value,
    corpus: &Corpus,
) -> Result<(), String> {
    if value.kind() != declaration.r#type {
        return Err(format!("value for {column} has the wrong type"));
    }
    if let Value::Item {
        endpoint: Endpoint::Item { id, .. },
        ..
    } = value
        && !declaration.flavours.is_empty()
    {
        let item = crate::resolve_item(corpus, id).map_err(|error| error.to_string())?;
        if !declaration
            .flavours
            .iter()
            .any(|flavour| flavour == item.flavour())
        {
            return Err(format!("value for {column} has the wrong flavour"));
        }
    }
    Ok(())
}

fn definition_value(
    definition: &Definition,
    column: &str,
    value: &ParameterValue,
    corpus: &Corpus,
    file: &str,
) -> Result<Value, Error> {
    let typed = value
        .typed(corpus)
        .map_err(|error| Error::definition(file, error.message))?;
    column_problem(&definition.columns[column], column, &typed, corpus)
        .map_err(|message| Error::definition(file, format!("declared {message}")))?;
    Ok(typed)
}
