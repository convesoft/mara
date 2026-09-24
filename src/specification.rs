//! Read-only, source-linked specifications over explicitly selected corpus content.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    Corpus, DiagnosticCode, DiagnosticLocation, DiscoveryNodeKind, DiscoveryNodeSummary,
    FieldFilter, Item, ItemFilters, ItemSource, MetadataFragment, Project, ReferenceKind,
    RelationEdge, RelationEndpoint, Schema, Severity, TextRange, TraceSelection,
    ValidationDiagnostic, ValidationError, ValidationScope, query,
};

const PAGE_BYTES: usize = 65_536;
const FRAGMENT_BYTES: usize = 4_096;

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceSpecificationParams {
    #[serde(flatten)]
    pub selection: TraceSelection,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub render: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SpecificationRecord {
    Content {
        node: DiscoveryNodeSummary,
        source_range: ItemSource,
        content: String,
        content_range: TextRange,
        metadata: Vec<MetadataFragment>,
        breadcrumbs: Vec<String>,
    },
    Item {
        node: DiscoveryNodeSummary,
        breadcrumbs: Vec<String>,
    },
    Relationship {
        item: DiscoveryNodeSummary,
        edge: RelationEdge,
        label: String,
        direction: String,
        endpoint: Value,
        outside_selection: bool,
        occurrence_count: usize,
        inspection: Value,
    },
    Issue {
        diagnostic: Value,
    },
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TraceSpecificationResult {
    pub format_version: u8,
    pub kind: String,
    pub selection: TraceSelection,
    pub narrative_included: bool,
    pub evaluation_complete: bool,
    pub records: Vec<SpecificationRecord>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

pub(crate) fn generate(
    project: &Project,
    schema: &Schema,
    params: &TraceSpecificationParams,
) -> Result<TraceSpecificationResult, ValidationError> {
    let mut selection = params.selection.clone();
    selection.ids.sort();
    selection.ids.dedup();
    selection.flavours.sort();
    selection.flavours.dedup();
    selection
        .fields
        .sort_by(|a, b| (&a.key, &a.value).cmp(&(&b.key, &b.value)));
    selection
        .fields
        .dedup_by(|a, b| a.key == b.key && a.value == b.value);
    selection.paths = query::normalized_paths(&selection.paths)
        .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
    selection.paths.sort();
    selection.paths.dedup();
    let filtered = !selection.ids.is_empty()
        || !selection.flavours.is_empty()
        || !selection.fields.is_empty()
        || !selection.paths.is_empty();
    if selection.all == filtered {
        return Err(ValidationError::invalid_argument(
            "select at least one id, flavour, field or path, or use all:true alone",
        ));
    }
    let limit = params.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(ValidationError::invalid_argument(
            "limit must be 1 through 100",
        ));
    }
    if params.render.as_deref().is_some_and(|r| r != "markdown") {
        return Err(ValidationError::invalid_argument("render must be markdown"));
    }
    if params.cursor.as_deref() == Some("") {
        return Err(ValidationError::invalid_argument(
            "cursor must not be empty",
        ));
    }
    let narrative_included =
        selection.ids.is_empty() && selection.flavours.is_empty() && selection.fields.is_empty();
    let (corpus, mut diagnostics) = crate::load_corpus_for_validation(project, schema)
        .map_err(|e| ValidationError::new("io_error", e.to_string()))?;
    diagnostics.extend(crate::validate_corpus(&corpus, schema));
    let filters = ItemFilters::new(
        selection.flavours.clone(),
        selection
            .fields
            .iter()
            .map(|f| FieldFilter::new(&f.key, &f.value))
            .collect(),
        vec![],
        selection.paths.clone(),
        None,
    )
    .with_search_options(selection.ids.clone(), false);
    let selected = query::filtered_items(&corpus, schema, &filters, None)
        .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
    let relevant_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            if narrative_included {
                selection.all
                    || selection
                        .paths
                        .iter()
                        .any(|path| diagnostic.source().path().starts_with(path))
            } else {
                selected.iter().any(|item| {
                    if diagnostic.source().path() != item.source().path() {
                        return false;
                    }
                    let offset = diagnostic.source().span().start_byte();
                    let span = item.source().span();
                    (span.start_byte() <= offset && offset < span.end_byte())
                        || (diagnostic.coordinates_available()
                            && diagnostic.applies_to_item(item.id()))
                })
            }
        })
        .collect::<Vec<_>>();
    let selected_mids = selected
        .iter()
        .filter_map(|item| item.mid())
        .collect::<BTreeSet<_>>();
    let graph = corpus.discovery();
    let nodes = graph
        .nodes()
        .filter_map(|node| match node.kind() {
            DiscoveryNodeKind::Item(item) => Some((item.id(), node.summary())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let documents = graph
        .nodes()
        .filter_map(|node| match node.kind() {
            DiscoveryNodeKind::Document(document) => Some((document.path(), node.summary())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::new();
    let edges = collect_edges(&corpus, schema);
    for document in corpus.documents() {
        if narrative_included
            && !selection.all
            && !selection
                .paths
                .iter()
                .any(|path| document.path().starts_with(path))
        {
            continue;
        }
        let mut offset = 0;
        for item in document.items() {
            if narrative_included && offset < item.source().span().start_byte() {
                add_content(
                    &mut records,
                    documents[document.path()].clone(),
                    document,
                    offset,
                    item.source().span().start_byte(),
                    &[],
                );
            }
            offset = item.source().span().end_byte();
            if !selected
                .iter()
                .any(|candidate| std::ptr::eq(*candidate, item))
            {
                continue;
            }
            let node = nodes[item.id()].clone();
            let breadcrumbs = breadcrumbs(&graph, item);
            records.push(SpecificationRecord::Item {
                node: node.clone(),
                breadcrumbs: breadcrumbs.clone(),
            });
            for (index, entry) in item.metadata().iter().enumerate() {
                for (start, end) in fragments(entry.value()) {
                    let metadata = MetadataFragment {
                        index,
                        key: entry.key().to_owned(),
                        value: entry.value()[start..end].to_owned(),
                        range: text_range(start, end, entry.value().len()),
                    };
                    records.push(SpecificationRecord::Content {
                        node: node.clone(),
                        source_range: entry.source().into(),
                        content: String::new(),
                        content_range: text_range(0, 0, 0),
                        metadata: vec![metadata],
                        breadcrumbs: breadcrumbs.clone(),
                    });
                }
            }
            let body = item.body_source().span();
            add_content(
                &mut records,
                node.clone(),
                document,
                body.start_byte(),
                body.end_byte(),
                &breadcrumbs,
            );
            for (edge, count) in &edges {
                let (direction, endpoint) = if edge.symmetric {
                    if endpoint_mid(&edge.source) == item.mid() {
                        ("symmetric", &edge.target)
                    } else if endpoint_mid(&edge.target) == item.mid() {
                        ("symmetric", &edge.source)
                    } else {
                        continue;
                    }
                } else if endpoint_mid(&edge.source) == item.mid() {
                    ("outgoing", &edge.target)
                } else if endpoint_mid(&edge.target) == item.mid() {
                    ("incoming", &edge.source)
                } else {
                    continue;
                };
                let label = if direction == "incoming" {
                    schema
                        .relations()
                        .get(&edge.relation)
                        .and_then(|definition| serde_json::to_value(definition).ok())
                        .and_then(|value| value["inverse"].as_str().map(str::to_owned))
                        .unwrap_or_else(|| format!("incoming {}", edge.relation))
                } else {
                    edge.relation.clone()
                };
                let neighbour = match endpoint {
                    RelationEndpoint::Item { id, mid } => nodes
                        .get(id.as_str())
                        .map(|n| json!(n))
                        .unwrap_or_else(|| json!({"kind":"item","id":id,"mid":mid})),
                    RelationEndpoint::External { address } => {
                        json!({"kind":"external","address":address})
                    }
                };
                let outside = endpoint_mid(endpoint).is_none_or(|mid| !selected_mids.contains(mid));
                records.push(SpecificationRecord::Relationship {
                    item: node.clone(), edge: edge.clone(), label, direction: direction.into(),
                    endpoint: neighbour, outside_selection: outside, occurrence_count: *count,
                    inspection: json!({"source":edge.source.id(),"relation":edge.relation,"target":edge.target.id()}),
                });
            }
        }
        if narrative_included && offset < document.source().len() {
            add_content(
                &mut records,
                documents[document.path()].clone(),
                document,
                offset,
                document.source().len(),
                &[],
            );
        }
    }
    for diagnostic in &relevant_diagnostics {
        records.push(SpecificationRecord::Issue {
            diagnostic: json!(ValidationDiagnostic::from_source(diagnostic)),
        });
    }
    if !corpus.is_complete() {
        for item in &selected {
            for relation in item.relations() {
                if crate::external::address(relation.target()).is_some() {
                    continue;
                }
                let matches = corpus
                    .items()
                    .filter(|target| {
                        target.id() == relation.target() || target.mid() == Some(relation.target())
                    })
                    .count();
                if matches != 1 {
                    records.push(SpecificationRecord::Issue {
                        diagnostic: json!(ValidationDiagnostic::new(
                            DiagnosticCode::ReferenceUnresolved,
                            Severity::Error,
                            ValidationScope::Item,
                            DiagnosticLocation::source(relation.source()),
                            format!(
                                "relation target '{}' is unavailable in the loaded corpus",
                                relation.target()
                            ),
                        )),
                    });
                }
            }
        }
    }
    let evaluation_complete = !records
        .iter()
        .any(|record| matches!(record, SpecificationRecord::Issue { .. }));
    let mut result = TraceSpecificationResult {
        format_version: 1,
        kind: "specification".into(),
        selection,
        narrative_included,
        evaluation_complete,
        records,
        has_more: false,
        next_cursor: None,
        markdown: None,
    };
    paginate(
        project,
        schema,
        &corpus,
        &diagnostics,
        params,
        limit,
        &mut result,
    )?;
    Ok(result)
}

fn text_range(start: usize, end: usize, total: usize) -> TextRange {
    TextRange {
        start_byte: start,
        end_byte: end,
        total_bytes: total,
        partial: start != 0 || end != total,
    }
}

fn fragments(text: &str) -> Vec<(usize, usize)> {
    if text.is_empty() {
        return vec![(0, 0)];
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + FRAGMENT_BYTES).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        result.push((start, end));
        start = end;
    }
    result
}

fn add_content(
    records: &mut Vec<SpecificationRecord>,
    node: DiscoveryNodeSummary,
    document: &crate::Document,
    start: usize,
    end: usize,
    breadcrumbs: &[String],
) {
    let original = &document.source()[start..end];
    let mut definitions = Vec::new();
    collect_definitions(document.blocks(), &mut definitions);
    for item in document.items() {
        collect_definitions(item.body_blocks(), &mut definitions);
    }
    let reference_spans = document
        .references()
        .iter()
        .map(|reference| reference.source().span())
        .chain(definitions.iter().map(|block| block.source().span()))
        .collect::<Vec<_>>();
    let mut parts = Vec::new();
    let mut part_start = 0;
    while part_start < original.len() {
        let mut part_end = (part_start + FRAGMENT_BYTES).min(original.len());
        while !original.is_char_boundary(part_end) {
            part_end -= 1;
        }
        if part_end < original.len()
            && let Some(newline) = original[part_start..part_end].rfind('\n')
        {
            part_end = part_start + newline + 1;
        }
        for span in &reference_spans {
            if span.start_byte() < start || span.end_byte() > end {
                continue;
            }
            let ref_start = span.start_byte() - start;
            let ref_end = span.end_byte() - start;
            if ref_start < part_end && ref_end > part_end && ref_start >= part_start {
                part_end = if ref_start == part_start {
                    ref_end
                } else {
                    ref_start
                };
                break;
            }
        }
        parts.push((part_start, part_end));
        part_start = part_end;
    }
    let line_starts = std::iter::once(0)
        .chain(document.source().match_indices('\n').map(|(i, _)| i + 1))
        .collect::<Vec<_>>();
    for (part_start, part_end) in parts {
        let absolute_start = start + part_start;
        let absolute_end = start + part_end;
        records.push(SpecificationRecord::Content {
            node: node.clone(),
            source_range: ItemSource::from(&crate::corpus::location(
                document.path(),
                &line_starts,
                absolute_start,
                absolute_end,
            )),
            content: original[part_start..part_end].to_owned(),
            content_range: text_range(part_start, part_end, original.len()),
            metadata: Vec::new(),
            breadcrumbs: breadcrumbs.to_vec(),
        });
    }
}

fn breadcrumbs(graph: &crate::DiscoveryGraph<'_>, item: &Item) -> Vec<String> {
    let Some(mut parent) = graph
        .nodes()
        .find(|node| {
            matches!(node.kind(), DiscoveryNodeKind::Item(candidate)
        if std::ptr::eq(candidate, item))
        })
        .and_then(|node| node.parent())
    else {
        return Vec::new();
    };
    let mut labels = Vec::new();
    loop {
        if let DiscoveryNodeKind::Section { heading } = parent.kind()
            && let Some(text) = heading.heading_text()
        {
            labels.push(text.to_owned());
        }
        let Some(next) = parent.parent() else { break };
        parent = next;
    }
    labels.reverse();
    labels
}

fn endpoint_mid(endpoint: &RelationEndpoint) -> Option<&str> {
    match endpoint {
        RelationEndpoint::Item { mid, .. } => Some(mid),
        _ => None,
    }
}

fn collect_edges(corpus: &Corpus, schema: &Schema) -> Vec<(RelationEdge, usize)> {
    let mut edges = BTreeMap::<String, (RelationEdge, usize)>::new();
    for item in corpus.items() {
        for relation in item.relations() {
            let edge = crate::relations::resolve_edge(
                corpus,
                schema,
                item.id(),
                relation.name(),
                relation.target(),
            );
            if let Ok(edge) = edge {
                let key = serde_json::to_string(&edge).expect("edge serializes");
                edges
                    .entry(key)
                    .and_modify(|(_, count)| *count += 1)
                    .or_insert((edge, 1));
            }
        }
    }
    edges.into_values().collect()
}

fn paginate(
    project: &Project,
    schema: &Schema,
    corpus: &Corpus,
    diagnostics: &[crate::Diagnostic],
    params: &TraceSpecificationParams,
    limit: usize,
    result: &mut TraceSpecificationResult,
) -> Result<(), ValidationError> {
    let mut hash = Sha256::new();
    hash.update(b"trace-specification-1");
    hash.update(serde_json::to_vec(schema).expect("schema serializes"));
    for path in [
        project.root().join(crate::PROJECT_FILE),
        project.schema_path().to_owned(),
    ]
    .into_iter()
    .chain(
        project
            .rule_files()
            .iter()
            .map(|path| project.root().join(path)),
    ) {
        hash.update(path.as_os_str().as_encoded_bytes());
        if let Ok(bytes) = fs::read(path) {
            hash.update(bytes);
        }
    }
    for document in corpus.documents() {
        hash.update(document.path().as_os_str().as_encoded_bytes());
        hash.update(document.source().as_bytes());
    }
    let retained = corpus
        .documents()
        .iter()
        .map(|document| document.path())
        .collect::<BTreeSet<_>>();
    let mut excluded = BTreeSet::new();
    for diagnostic in diagnostics {
        hash.update(
            serde_json::to_vec(&ValidationDiagnostic::from_source(diagnostic))
                .expect("diagnostic serializes"),
        );
        if !retained.contains(diagnostic.source().path()) {
            excluded.insert(diagnostic.source().path());
        }
    }
    for path in excluded {
        hash.update(path.as_os_str().as_encoded_bytes());
        if let Ok(bytes) = fs::read(project.root().join(path)) {
            hash.update(bytes);
        }
    }
    hash.update(
        serde_json::to_vec(&(&result.selection, limit, &params.render))
            .expect("selection serializes"),
    );
    let fingerprint = format!("{:x}", hash.finalize());
    let start = match &params.cursor {
        None => 0,
        Some(cursor) => {
            let (prefix, position) = cursor.rsplit_once(':').ok_or_else(stale_cursor)?;
            if prefix != format!("1:{fingerprint}") {
                return Err(stale_cursor());
            }
            position.parse::<usize>().map_err(|_| stale_cursor())?
        }
    };
    let all = std::mem::take(&mut result.records);
    if params.cursor.is_some() && (start == 0 || start >= all.len()) {
        return Err(stale_cursor());
    }
    for record in all.iter().skip(start).take(limit) {
        result.records.push(record.clone());
        result.has_more = start + result.records.len() < all.len();
        result.next_cursor = result
            .has_more
            .then(|| format!("1:{fingerprint}:{}", start + result.records.len()));
        if params.render.as_deref() == Some("markdown") {
            result.markdown = Some(markdown(result, corpus));
        }
        if serde_json::to_vec(result).expect("result serializes").len() > PAGE_BYTES {
            result.records.pop();
            if result.records.is_empty() {
                return Err(ValidationError::new(
                    "output_limit",
                    "a specification record exceeds the 65536-byte page budget",
                ));
            }
            result.has_more = true;
            result.next_cursor = Some(format!("1:{fingerprint}:{}", start + result.records.len()));
            if params.render.as_deref() == Some("markdown") {
                result.markdown = Some(markdown(result, corpus));
            }
            break;
        }
    }
    if params.render.as_deref() == Some("markdown") {
        result.markdown = Some(markdown(result, corpus));
    }
    if serde_json::to_vec(result).expect("result serializes").len() > PAGE_BYTES {
        return Err(ValidationError::new(
            "output_limit",
            "specification envelope exceeds the 65536-byte page budget",
        ));
    }
    Ok(())
}

fn stale_cursor() -> ValidationError {
    ValidationError::new(
        "stale_cursor",
        "invalid or stale specification cursor; restart without a cursor",
    )
}

fn markdown(result: &TraceSpecificationResult, corpus: &Corpus) -> String {
    let anchors = heading_anchors(corpus);
    let mut out = String::from(
        "# Specification\n\nCanonical links are relative to the project root. Save this page there to follow them.\n\n",
    );
    out.push_str(&format!("Selection: `{}`. Narrative included: **{}**. Evaluation complete: **{}**. More records: **{}**.\n\n",
        serde_json::to_string(&result.selection).unwrap(), result.narrative_included,
        result.evaluation_complete, result.has_more));
    if let Some(cursor) = &result.next_cursor {
        out.push_str(&format!(
            "Continue with `--cursor {cursor}` and the same selection and limit.\n\n"
        ));
    }
    if !result.narrative_included {
        out.push_str("Item selection omits document narrative and unselected ancestor bodies; headings appear as breadcrumbs.\n\n");
    }
    for record in &result.records {
        match record {
            SpecificationRecord::Item { node, breadcrumbs } => {
                out.push_str(&format!(
                    "\n## [{}](<{}>) — {} (line {})\n\n",
                    node.id.as_deref().unwrap_or("item"),
                    source_target(node.source.path(), node.source.start_byte(), &anchors),
                    node.title.as_deref().unwrap_or(""),
                    node.source.start_line()
                ));
                if !breadcrumbs.is_empty() {
                    out.push_str(&format!("Context: {}\n\n", breadcrumbs.join(" › ")));
                }
            }
            SpecificationRecord::Content {
                source_range,
                content,
                content_range,
                metadata,
                ..
            } => {
                if !metadata.is_empty() {
                    for entry in metadata {
                        out.push_str(&format!(
                            "- `{}`: {}{} ([source line {}](<{}>))\n",
                            entry.key,
                            entry.value,
                            if entry.range.partial {
                                " (continued)"
                            } else {
                                ""
                            },
                            source_range.start_line(),
                            source_target(source_range.path(), source_range.start_byte(), &anchors)
                        ));
                    }
                    out.push('\n');
                } else if !content.is_empty() {
                    out.push_str(&format!(
                        "[Source: {} (line {})](<{}>){}\n\n",
                        source_range.path().display(),
                        source_range.start_line(),
                        source_target(source_range.path(), source_range.start_byte(), &anchors),
                        if content_range.partial {
                            " — continued"
                        } else {
                            ""
                        }
                    ));
                    let authored = render_references(content, source_range, corpus, &anchors);
                    out.push_str(&render_fragment(&authored, source_range, corpus));
                    out.push_str("\n\n");
                }
            }
            SpecificationRecord::Relationship {
                label,
                endpoint,
                outside_selection,
                occurrence_count,
                inspection,
                ..
            } => {
                let name = endpoint["id"]
                    .as_str()
                    .or_else(|| endpoint["address"].as_str())
                    .unwrap_or("?");
                let path = endpoint["source"]["path"].as_str();
                let byte = endpoint["source"]["start_byte"].as_u64().unwrap_or(0) as usize;
                out.push_str(&format!("- Relationship `{}` → {}{} ({} occurrence{}; inspect with `relation get {} {} {}`)\n",
                    label, path.map_or_else(|| name.to_owned(), |p| format!("[{name}](<{}>)", source_target(Path::new(p), byte, &anchors))),
                    if *outside_selection { " [outside selection]" } else { "" }, occurrence_count,
                    if *occurrence_count == 1 { "" } else { "s" }, inspection["source"].as_str().unwrap_or(""),
                    inspection["relation"].as_str().unwrap_or(""), inspection["target"].as_str().unwrap_or("")));
            }
            SpecificationRecord::Issue { diagnostic } => {
                out.push_str(&format!("\nIssue: {}\n", diagnostic))
            }
        }
    }
    out
}

fn md_target(path: &Path) -> String {
    path.to_string_lossy()
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('#', "%23")
        .replace('?', "%3F")
}

fn render_fragment(content: &str, source: &ItemSource, corpus: &Corpus) -> String {
    let Some(document) = corpus
        .documents()
        .iter()
        .find(|doc| doc.path() == source.path())
    else {
        return content.to_owned();
    };
    let mut code_blocks = Vec::new();
    collect_code_blocks(document.blocks(), &mut code_blocks);
    for item in document.items() {
        collect_code_blocks(item.body_blocks(), &mut code_blocks);
    }
    let mut opening = None;
    let mut closing = None;
    for block in code_blocks {
        let span = block.source().span();
        let Some((open, close)) = fence_markers(document.source(), span) else {
            continue;
        };
        if span.start_byte() < source.start_byte() && source.start_byte() < span.end_byte() {
            opening = Some(open);
        }
        if span.start_byte() < source.end_byte() && source.end_byte() < span.end_byte() {
            closing = Some(close);
        }
    }
    let mut rendered = String::new();
    if let Some(open) = opening {
        rendered.push_str(&open);
        rendered.push('\n');
    }
    rendered.push_str(content);
    if let Some(close) = closing {
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str(&close);
        rendered.push('\n');
    }
    rendered
}

fn collect_code_blocks<'a>(
    blocks: &'a [crate::MarkdownBlock],
    out: &mut Vec<&'a crate::MarkdownBlock>,
) {
    for block in blocks {
        if block.kind() == crate::MarkdownBlockKind::CodeBlock {
            out.push(block);
        }
        collect_code_blocks(block.children(), out);
    }
}

fn fence_markers(source: &str, span: crate::SourceSpan) -> Option<(String, String)> {
    let first = source[span.start_byte()..span.end_byte()].lines().next()?;
    let tick = first.find("```");
    let tilde = first.find("~~~");
    let start = match (tick, tilde) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return None,
    };
    let prefix = &first[..start];
    if prefix
        .rsplit('>')
        .next()
        .unwrap_or(prefix)
        .chars()
        .filter(|ch| *ch == ' ')
        .count()
        > 3
    {
        return None;
    }
    let marker = first[start..].chars().next()?;
    let count = first[start..]
        .chars()
        .take_while(|ch| *ch == marker)
        .count();
    Some((
        first.to_owned(),
        format!("{prefix}{}", marker.to_string().repeat(count)),
    ))
}

type HeadingAnchors = BTreeMap<PathBuf, Vec<(usize, usize, String)>>;

fn heading_anchors(corpus: &Corpus) -> HeadingAnchors {
    let graph = corpus.discovery();
    let mut result = HeadingAnchors::new();
    let mut used = BTreeMap::<PathBuf, BTreeSet<String>>::new();
    for node in graph.nodes() {
        if let DiscoveryNodeKind::Section { heading } = node.kind() {
            let base = crate::discovery::heading_anchor(heading.heading_text().unwrap_or_default());
            let path = node.source().path().to_owned();
            let names = used.entry(path.clone()).or_default();
            let mut anchor = base.clone();
            let mut number = 0;
            while !names.insert(anchor.clone()) {
                number += 1;
                anchor = format!("{base}-{number}");
            }
            result.entry(path).or_default().push((
                node.source().span().start_byte(),
                node.source().span().end_byte(),
                anchor,
            ));
        }
    }
    result
}

fn source_target(path: &Path, byte: usize, anchors: &HeadingAnchors) -> String {
    let mut target = md_target(path);
    if let Some((_, _, anchor)) = anchors
        .get(path)
        .into_iter()
        .flatten()
        .filter(|(start, end, _)| *start <= byte && byte < *end)
        .max_by_key(|(start, _, _)| start)
    {
        target.push('#');
        target.push_str(anchor);
    }
    target
}

fn render_references(
    content: &str,
    source: &ItemSource,
    corpus: &Corpus,
    anchors: &HeadingAnchors,
) -> String {
    // Parser reference spans exclude code and escaped literal examples.
    let Some(document) = corpus
        .documents()
        .iter()
        .find(|doc| doc.path() == source.path())
    else {
        return content.to_owned();
    };
    let mut edits = Vec::<(usize, usize, String)>::new();
    for reference in document.references() {
        let span = reference.source().span();
        if span.start_byte() < source.start_byte() || span.end_byte() > source.end_byte() {
            continue;
        }
        let start = span.start_byte() - source.start_byte();
        let end = span.end_byte() - source.start_byte();
        let raw = &content[start..end];
        let replacement = match reference.kind() {
            ReferenceKind::Item => {
                let mut targets = corpus.items().filter(|item| {
                    item.id() == reference.target() || item.mid() == Some(reference.target())
                });
                match (targets.next(), targets.next()) {
                    (Some(item), None) => Some(format!(
                        "[{raw}](<{}>)",
                        source_target(
                            item.source().path(),
                            item.source().span().start_byte(),
                            anchors
                        )
                    )),
                    _ => None,
                }
            }
            ReferenceKind::MarkdownLink => {
                let target = reference.target();
                rebase_target(target, source.path()).and_then(|rebased| {
                    raw.rfind(target)
                        .map(|at| format!("{}{}{}", &raw[..at], rebased, &raw[at + target.len()..]))
                })
            }
            ReferenceKind::Anchor => None,
        };
        if let Some(replacement) = replacement {
            edits.push((start, end, replacement));
        }
    }
    let mut definitions = Vec::new();
    collect_definitions(document.blocks(), &mut definitions);
    for item in document.items() {
        collect_definitions(item.body_blocks(), &mut definitions);
    }
    for block in definitions {
        let span = block.source().span();
        if span.start_byte() < source.start_byte() || span.end_byte() > source.end_byte() {
            continue;
        }
        let start = span.start_byte() - source.start_byte();
        let end = span.end_byte() - source.start_byte();
        let raw = &content[start..end];
        let Some((prefix, rest)) = raw.split_once("]:") else {
            continue;
        };
        let leading = rest.len() - rest.trim_start().len();
        let target_start = prefix.len() + 2 + leading;
        let destination = &raw[target_start..];
        let target_end = if destination.starts_with('<') {
            destination
                .find('>')
                .map(|i| i + 1)
                .unwrap_or(destination.len())
        } else {
            destination
                .find(char::is_whitespace)
                .unwrap_or(destination.len())
        };
        let token = &destination[..target_end];
        let target = token
            .strip_prefix('<')
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(token);
        if let Some(rebased) = rebase_target(target, source.path()) {
            edits.push((
                start + target_start + usize::from(token.starts_with('<')),
                start + target_start + target_end - usize::from(token.ends_with('>')),
                rebased,
            ));
        }
    }
    edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut output = content.to_owned();
    let mut next_start = usize::MAX;
    for (start, end, replacement) in edits {
        if end <= next_start {
            output.replace_range(start..end, &replacement);
            next_start = start;
        }
    }
    output
}

fn collect_definitions<'a>(
    blocks: &'a [crate::MarkdownBlock],
    out: &mut Vec<&'a crate::MarkdownBlock>,
) {
    for block in blocks {
        if block.kind() == crate::MarkdownBlockKind::LinkReferenceDefinition {
            out.push(block);
        }
        collect_definitions(block.children(), out);
    }
}

fn rebase_target(target: &str, path: &Path) -> Option<String> {
    if target.is_empty() || target.starts_with("//") {
        return None;
    }
    let (file, fragment) = target.split_once('#').unwrap_or((target, ""));
    if file.contains(':') {
        return None;
    }
    let file = crate::discovery::percent_decode(file);
    let destination = if file.is_empty() {
        path.to_path_buf()
    } else {
        let base = if file.starts_with('/') {
            Path::new("")
        } else {
            path.parent().unwrap_or(Path::new(""))
        };
        base.join(file.trim_start_matches('/'))
    };
    let mut normalized = PathBuf::new();
    for component in destination.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            Component::Normal(name) => normalized.push(name),
            _ => {}
        }
    }
    let mut rebased = md_target(&normalized);
    if !fragment.is_empty() {
        rebased.push('#');
        rebased.push_str(fragment);
    }
    Some(rebased)
}
