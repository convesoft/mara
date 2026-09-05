use std::collections::{BTreeMap, BTreeSet};

use super::{invalid, newline_style, transaction, validate_scalar};
use crate::{Corpus, Error, Item, ItemUpdateParams, Project, Schema, load_corpus, validate_corpus};

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ItemUpdate {
    pub id: String,
    pub mid: String,
    pub path: std::path::PathBuf,
    pub changed_fields: Vec<String>,
    pub warnings: Vec<crate::ValidationDiagnostic>,
}

fn replacements(
    schema: &Schema,
    item: &Item,
    request: &ItemUpdateParams,
) -> Result<BTreeMap<String, Vec<String>>, Error> {
    if request.title.is_none()
        && request.fields.is_empty()
        && request.clear_fields.is_empty()
        && request.body.is_none()
    {
        return invalid("item update requires at least one requested change");
    }
    let flavour = schema
        .flavours
        .get(item.flavour())
        .ok_or_else(|| Error::InvalidMutation {
            message: format!("unknown flavour '{}'", item.flavour()),
        })?;
    let mut values = BTreeMap::<String, Vec<String>>::new();
    for field in &request.fields {
        if !flavour.fields.contains_key(&field.key) {
            return invalid(format!(
                "'{}' is not a custom field of flavour '{}'; item update cannot change identity or relations",
                field.key,
                item.flavour()
            ));
        }
        validate_scalar(&format!("field '{}'", field.key), &field.value)?;
        values
            .entry(field.key.clone())
            .or_default()
            .push(field.value.trim().to_owned());
    }
    for name in &request.clear_fields {
        let Some(definition) = flavour.fields.get(name) else {
            return invalid(format!(
                "'{name}' is not a custom field of flavour '{}'; item update cannot change identity or relations",
                item.flavour()
            ));
        };
        if request.fields.iter().any(|field| field.key == *name) {
            return invalid(format!(
                "field '{name}' cannot be both replaced and cleared"
            ));
        }
        if definition.required {
            return invalid(format!("required field '{name}' cannot be cleared"));
        }
        values.insert(name.clone(), Vec::new());
    }
    if let Some(title) = &request.title {
        validate_scalar("title", title)?;
        if title.trim().is_empty() {
            return invalid("title must not be empty");
        }
        values.insert("title".into(), vec![title.trim().to_owned()]);
    }
    Ok(values)
}

fn render_update(
    source: &str,
    item: &Item,
    values: &BTreeMap<String, Vec<String>>,
    body: Option<&str>,
) -> (String, Vec<String>) {
    let newline = newline_style(source);
    let mut changed = BTreeSet::new();
    for (key, replacement) in values {
        let original = item
            .metadata()
            .iter()
            .filter(|entry| entry.key() == key)
            .map(|entry| entry.value())
            .collect::<Vec<_>>();
        if original != replacement.iter().map(String::as_str).collect::<Vec<_>>() {
            changed.insert(key.clone());
        }
    }
    // Reuse existing value slots and their whitespace. Extra values follow the last
    // slot; new keys follow all original metadata in deterministic key order.
    let mut candidate = String::new();
    let mut cursor = 0;
    let mut used = BTreeMap::<&str, usize>::new();
    for (index, entry) in item.metadata().iter().enumerate() {
        let span = entry.source().span();
        let start = span.start_byte();
        let end = super::full_line_end(source, span.end_byte());
        candidate.push_str(&source[cursor..start]);
        if changed.contains(entry.key()) {
            let replacement = &values[entry.key()];
            let used = used.entry(entry.key()).or_default();
            if let Some(value) = replacement.get(*used) {
                let raw = &source[start..span.end_byte()];
                let prefix = entry.key().len() + 2;
                let scalar = &raw[prefix..];
                let leading = scalar.len() - scalar.trim_start().len();
                let value_end = scalar.trim_end().len().max(leading);
                candidate.push_str(&raw[..prefix + leading]);
                candidate.push_str(value);
                candidate.push_str(&raw[prefix + value_end..]);
                candidate.push_str(&source[span.end_byte()..end]);
                *used += 1;
            }
            if !item.metadata()[index + 1..]
                .iter()
                .any(|later| later.key() == entry.key())
            {
                for value in &replacement[*used..] {
                    candidate.push_str(&format!(":{}: {value}{newline}", entry.key()));
                }
            }
        } else {
            candidate.push_str(&source[start..end]);
        }
        cursor = end;
    }
    for (key, replacement) in values {
        if !item.metadata().iter().any(|entry| entry.key() == key) {
            for value in replacement {
                candidate.push_str(&format!(":{key}: {value}{newline}"));
            }
        }
    }
    let span = item.body_source().span();
    candidate.push_str(&source[cursor..span.start_byte()]);
    let body = body.map(|body| normalize_body(body, newline));
    if let Some(body) = body {
        if body != item.body() {
            changed.insert("body".into());
        }
        candidate.push_str(&body);
    } else {
        candidate.push_str(item.body());
    }
    candidate.push_str(&source[span.end_byte()..]);
    (candidate, changed.into_iter().collect())
}

pub fn update_item(
    project: &Project,
    schema: &Schema,
    request: ItemUpdateParams,
) -> Result<ItemUpdate, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let corpus = load_corpus(project, schema)?;
    // All identities must be usable before selecting a source span.
    super::ensure_unambiguous_item_identities(&corpus, "update items")?;
    let resolved =
        crate::get_item(&corpus, &request.reference).map_err(|error| Error::InvalidMutation {
            message: error.to_string(),
        })?;
    let item = corpus
        .items()
        .find(|item| item.id() == resolved.summary().id())
        .expect("resolved item belongs to corpus");
    let values = replacements(schema, item, &request)?;
    let document = corpus
        .documents()
        .iter()
        .find(|document| document.path() == item.source().path())
        .expect("item belongs to document");
    let (candidate, changed_fields) =
        render_update(document.source(), item, &values, request.body.as_deref());
    let path = document.path().to_path_buf();
    let projected =
        corpus.with_replacements(&BTreeMap::from([(path.clone(), candidate.clone())]), schema)?;
    let warnings = validate_update(&corpus, &projected, schema, item, &request)?;

    let updated = projected
        .items()
        .find(|candidate| candidate.mid() == item.mid())
        .ok_or_else(|| Error::InvalidMutation {
            message: "update would hide the selected item".into(),
        })?;
    let expected_body = request
        .body
        .as_ref()
        .map(|body| normalize_body(body, newline_style(document.source())))
        .unwrap_or_else(|| item.body().to_owned());
    if updated.body() != expected_body || projected.items().count() != corpus.items().count() {
        return invalid(
            "updated body must remain inside the selected item without changing item recognition",
        );
    }
    for original in corpus.items() {
        let Some(after) = projected
            .items()
            .find(|after| after.mid() == original.mid())
        else {
            return invalid("update would hide an existing item");
        };
        let mut expected = metadata_values(original);
        if original.mid() == item.mid() {
            for (key, replacement) in &values {
                if replacement.is_empty() {
                    expected.remove(key.as_str());
                } else {
                    expected.insert(key, replacement.iter().map(String::as_str).collect());
                }
            }
        }
        if original.id() != after.id()
            || original.flavour() != after.flavour()
            || original.source().path() != after.source().path()
            || metadata_values(after) != expected
            || (original.mid() != item.mid()
                && (original.body() != after.body()
                    || original
                        .mentions()
                        .iter()
                        .map(|mention| mention.target())
                        .collect::<Vec<_>>()
                        != after
                            .mentions()
                            .iter()
                            .map(|mention| mention.target())
                            .collect::<Vec<_>>()))
        {
            return invalid(
                "update would change identity, unrelated content, or metadata outside the request",
            );
        }
    }
    let result = ItemUpdate {
        id: item.id().to_owned(),
        mid: item.mid().expect("checked identity").to_owned(),
        path: path.clone(),
        changed_fields,
        warnings,
    };
    if candidate == document.source() {
        return Ok(result);
    }
    let change = transaction::Change::new(
        project,
        path.clone(),
        Some(document.source().to_owned()),
        candidate,
    )?;
    transaction::commit_single(project, change, || {
        if crate::resolve_project(Some(project.root()), project.root())? != *project
            || crate::load_schema(project)? != *schema
            || load_corpus(project, schema)? != corpus
            || !crate::corpus::document_is_discoverable(project, &path)?
        {
            return invalid("project changed since update preflight; retry the update");
        }
        Ok(())
    })?;
    Ok(result)
}

fn metadata_values(item: &Item) -> BTreeMap<&str, Vec<&str>> {
    let mut values = BTreeMap::<&str, Vec<&str>>::new();
    for entry in item.metadata() {
        values.entry(entry.key()).or_default().push(entry.value());
    }
    values
}

fn validate_update(
    original: &Corpus,
    candidate: &Corpus,
    schema: &Schema,
    selected: &Item,
    request: &ItemUpdateParams,
) -> Result<Vec<crate::ValidationDiagnostic>, Error> {
    let mut warnings = Vec::new();
    for diagnostic in validate_corpus(candidate, schema) {
        // Only an unchanged, already-missing body may remain incomplete. Use a
        // typed diagnostic, never a substring of user-controllable error text.
        let existing_scaffold = diagnostic.is_missing_body()
            && original.items().any(|item| {
                item.body().trim().is_empty()
                    && (item.mid() != selected.mid() || request.body.is_none())
                    && candidate.items().any(|after| {
                        after.mid() == item.mid()
                            && after.body() == item.body()
                            && diagnostic.source() == after.body_source()
                    })
            });
        if !existing_scaffold {
            return invalid(format!(
                "cannot update item while validation fails at {}:{}: {}",
                diagnostic.source().path().display(),
                diagnostic.source().span().start_line(),
                diagnostic.message()
            ));
        }
        warnings.push(crate::ValidationDiagnostic {
            scope: crate::ValidationScope::Item,
            path: Some(diagnostic.source().path().to_path_buf()),
            line: Some(diagnostic.source().span().start_line()),
            message: diagnostic.message().to_owned(),
        });
    }
    Ok(warnings)
}

fn normalize_body(body: &str, newline: &str) -> String {
    let mut body = body.replace("\r\n", "\n").replace('\n', newline);
    if !body.is_empty() && !body.ends_with('\n') {
        body.push_str(newline);
    }
    body
}
