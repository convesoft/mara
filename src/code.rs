//! Disposable code projection: SCIP owns semantic identity; optional Tree-sitter
//! assets supply declaration ranges and comment ownership.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use ignore::WalkBuilder;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

mod scip_index;

use crate::{DiagnosticCode, Project, corpus::location};

pub use mara_engine::code::*;
struct Adapter {
    extensions: Vec<String>,
    parser: Parser,
    query: Query,
}

enum AttachmentError {
    Ambiguous,
    Unsupported,
}

fn matches_extension(path: &Path, extensions: &[String]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.iter().any(|candidate| candidate == extension))
}

impl Adapter {
    fn accepts(&self, path: &Path) -> bool {
        matches_extension(path, &self.extensions)
    }

    fn load(project: &Project) -> Result<(Vec<Self>, Vec<PathBuf>), CodeProblem> {
        let config_path = Path::new(crate::PROJECT_FILE);
        let fail = |message: String| CodeProblem {
            code: DiagnosticCode::CodeUnsupported,
            message,
            source: location(config_path, &[0], 0, 0),
            target: None,
        };
        let mut adapters = Vec::new();
        let mut assets = vec![config_path.to_path_buf()];
        let mut assigned = std::collections::BTreeSet::new();
        for language in project.code_languages() {
            if language.name.is_empty()
                || language.extensions.is_empty()
                || language.extensions.iter().any(|extension| {
                    extension.is_empty()
                        || !extension.chars().all(|ch| ch.is_ascii_alphanumeric())
                        || !assigned.insert(extension.clone())
                })
            {
                return Err(fail(format!(
                    "language {} needs nonempty, unique alphanumeric source extensions",
                    language.name
                )));
            }
            if language.grammar.is_none() && language.query.is_none() {
                continue;
            }
            let (Some(grammar_path), Some(query_path)) = (&language.grammar, &language.query)
            else {
                return Err(fail(format!(
                    "language {} must configure grammar and query together",
                    language.name
                )));
            };
            if !valid_asset_path(grammar_path) || !valid_asset_path(query_path) {
                return Err(fail(format!(
                    "invalid adapter asset path for {}",
                    language.name
                )));
            }
            let grammar = read_project_asset(project, grammar_path)
                .map_err(|e| fail(format!("{}: {e}", grammar_path.display())))?;
            let query_source = String::from_utf8(
                read_project_asset(project, query_path)
                    .map_err(|e| fail(format!("{}: {e}", query_path.display())))?,
            )
            .map_err(|e| fail(format!("{}: {e}", query_path.display())))?;
            let engine = tree_sitter::wasmtime::Engine::default();
            let mut store =
                tree_sitter::WasmStore::new(&engine).map_err(|e| fail(e.to_string()))?;
            let grammar = store
                .load_language(&language.name, &grammar)
                .map_err(|e| fail(format!("{}: {e}", grammar_path.display())))?;
            let query = Query::new(&grammar, &query_source)
                .map_err(|e| fail(format!("{}: {e}", query_path.display())))?;
            if query.capture_index_for_name("name").is_none()
                || query.capture_index_for_name("symbol").is_none()
                || query.capture_index_for_name("comment").is_none()
            {
                return Err(fail(format!(
                    "{} must capture @symbol, @name and @comment",
                    query_path.display()
                )));
            }
            let name_capture = query.capture_index_for_name("name").unwrap() as usize;
            let symbol_capture = query.capture_index_for_name("symbol").unwrap() as usize;
            let scope_capture = query
                .capture_index_for_name("scope")
                .map(|index| index as usize);
            if (0..query.pattern_count()).any(|pattern| {
                let captures = query.capture_quantifiers(pattern);
                (captures[symbol_capture] != tree_sitter::CaptureQuantifier::Zero
                    || scope_capture.is_some_and(|index| {
                        captures[index] != tree_sitter::CaptureQuantifier::Zero
                    }))
                    && captures[name_capture] == tree_sitter::CaptureQuantifier::Zero
            }) {
                return Err(fail(format!(
                    "{} must pair @name with @symbol or @scope in each declaration pattern",
                    query_path.display()
                )));
            }
            let mut parser = Parser::new();
            parser
                .set_wasm_store(store)
                .map_err(|e| fail(e.to_string()))?;
            parser
                .set_language(&grammar)
                .map_err(|e| fail(e.to_string()))?;
            adapters.push(Self {
                extensions: language.extensions.clone(),
                parser,
                query,
            });
            assets.extend([grammar_path.clone(), query_path.clone()]);
        }
        Ok((adapters, assets))
    }

    // @mara implements REQ-CODE-TRACEABILITY
    fn attached_symbol<'a>(
        &self,
        comment: Node<'_>,
        group_end: usize,
        source: &str,
        symbols: &'a [CodeSymbol],
    ) -> Result<Option<&'a CodeSymbol>, AttachmentError> {
        let mut enclosing = symbols
            .iter()
            .filter(|s| s.body_start <= comment.start_byte() && comment.end_byte() <= s.body_end)
            .collect::<Vec<_>>();
        enclosing.sort_by_key(|s| s.body_end - s.body_start);
        let scope_end = enclosing.first().map(|symbol| symbol.body_end);
        let mut attached = symbols
            .iter()
            .filter_map(|s| {
                s.attach_starts
                    .iter()
                    .copied()
                    .filter(|start| {
                        *start >= group_end
                            && scope_end.is_none_or(|end| *start < end)
                            && source[group_end..*start].trim().is_empty()
                    })
                    .min()
                    .map(|start| (s, start))
            })
            .collect::<Vec<_>>();
        attached.sort_by_key(|(_, start)| *start);
        if let Some(first) = attached.first() {
            if attached.get(1).is_some_and(|next| next.1 == first.1) {
                return Err(AttachmentError::Ambiguous);
            }
            return Ok(Some(first.0));
        }
        if let Some(first) = enclosing.first() {
            if enclosing.get(1).is_some_and(|next| {
                next.body_end - next.body_start == first.body_end - first.body_start
            }) {
                return Err(AttachmentError::Ambiguous);
            }
            return Ok(Some(first));
        }
        if comment
            .parent()
            .is_some_and(|parent| parent.parent().is_none())
        {
            Ok(None)
        } else {
            Err(AttachmentError::Unsupported)
        }
    }
}

pub fn load(project: &Project) -> (CodeIndex, Vec<CodeProblem>) {
    let mut result = CodeIndex {
        root: project.root().to_owned(),
        files: BTreeMap::new(),
        assets: vec![PathBuf::from(crate::PROJECT_FILE)],
        inputs: Some(std::sync::Arc::new(NativeCodeSource(
            project.root().to_owned(),
        ))),
    };
    let mut problems = Vec::new();
    let mut adapters = match Adapter::load(project) {
        Ok((adapters, assets)) => {
            result.assets = assets;
            adapters
        }
        Err(problem) => {
            problems.push(problem);
            return (result, problems);
        }
    };
    if project.code_languages().is_empty() {
        return (result, problems);
    }
    let indexes = match scip_index::run(project) {
        Ok(indexes) => indexes,
        Err(problem) => return (result, vec![problem]),
    };
    let mut walker = WalkBuilder::new(project.root());
    walker
        .hidden(false)
        .ignore(false)
        .git_ignore(true)
        .git_exclude(false)
        .git_global(false)
        .parents(true)
        .require_git(false)
        .follow_links(false);
    for entry in walker.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                let path = crate::corpus::walk_error_path(project.root(), &error);
                problems.push(CodeProblem {
                    code: DiagnosticCode::SourceInvalid,
                    message: format!("could not discover code files: {error}"),
                    source: location(&path, &[0], 0, 0),
                    target: None,
                });
                continue;
            }
        };
        let supported_file = entry.file_type().is_some_and(|kind| kind.is_file())
            || entry.file_type().is_some_and(|kind| kind.is_symlink())
                && fs::canonicalize(entry.path()).is_ok_and(|canonical| {
                    canonical.starts_with(project.root()) && canonical.is_file()
                });
        if !supported_file {
            continue;
        }
        let path = entry
            .path()
            .strip_prefix(project.root())
            .unwrap()
            .to_path_buf();
        let Some(adapter) = adapters.iter_mut().find(|adapter| adapter.accepts(&path)) else {
            continue;
        };
        let source = match fs::read_to_string(entry.path()) {
            Ok(source) => source,
            Err(error) => {
                problems.push(CodeProblem {
                    code: DiagnosticCode::SourceInvalid,
                    message: format!("could not read code file: {error}"),
                    source: location(&path, &[0], 0, 0),
                    target: None,
                });
                continue;
            }
        };
        let (file, mut file_problems) = parse_file(path.clone(), source, adapter);
        problems.append(&mut file_problems);
        result.files.insert(path, file);
    }
    scip_index::apply(project, indexes, &mut result, &mut problems);
    (result, problems)
}

#[derive(Debug)]
struct NativeCodeSource(PathBuf);
impl CodeSource for NativeCodeSource {
    fn file(&self, path: &Path) -> Result<Vec<u8>, ReferenceError> {
        let canonical =
            fs::canonicalize(self.0.join(path)).map_err(|_| ReferenceError::MissingFile)?;
        if !canonical.starts_with(&self.0) || !canonical.is_file() {
            return Err(ReferenceError::Unsupported);
        }
        fs::read(canonical).map_err(|_| ReferenceError::Unsupported)
    }
}
fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

fn parse_file(
    path: PathBuf,
    source: String,
    adapter: &mut Adapter,
) -> (CodeFile, Vec<CodeProblem>) {
    let tree = adapter
        .parser
        .parse(&source, None)
        .expect("parser accepts source");
    let lines = line_starts(&source);
    let mut symbols = Vec::new();
    let mut comments = Vec::new();
    let mut declarations = BTreeMap::new();
    let symbol_capture = adapter.query.capture_index_for_name("symbol").unwrap();
    let scope_capture = adapter.query.capture_index_for_name("scope");
    let name_capture = adapter.query.capture_index_for_name("name").unwrap();
    let comment_capture = adapter.query.capture_index_for_name("comment").unwrap();
    let modifier_capture = adapter.query.capture_index_for_name("modifier");
    let wrapper_capture = adapter.query.capture_index_for_name("wrapper");
    let mut modifiers = BTreeSet::new();
    let mut wrappers = BTreeSet::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&adapter.query, tree.root_node(), source.as_bytes());
    while let Some(found) = matches.next() {
        let mut declaration = None;
        let mut name = None;
        for capture in found.captures() {
            if capture.index == symbol_capture {
                declaration = Some((capture.node, true));
            } else if Some(capture.index) == scope_capture {
                declaration = Some((capture.node, false));
            } else if capture.index == name_capture {
                name = Some(capture.node);
            } else if capture.index == comment_capture {
                comments.push(capture.node);
            } else if Some(capture.index) == modifier_capture {
                modifiers.insert(capture.node.id());
            } else if Some(capture.index) == wrapper_capture {
                wrappers.insert(capture.node.id());
            }
        }
        if let (Some((node, selectable)), Some(name_node)) = (declaration, name) {
            declarations.insert(node.id(), (name_node, selectable));
        }
    }
    comments.sort_by_key(|node| (node.start_byte(), node.end_byte()));
    comments.dedup_by_key(|node| (node.start_byte(), node.end_byte()));
    let comment_ids = comments.iter().map(Node::id).collect::<BTreeSet<_>>();
    let context = CollectContext {
        declarations: &declarations,
        modifiers: &modifiers,
        wrappers: &wrappers,
        comments: &comment_ids,
        path: &path,
        lines: &lines,
    };
    collect(tree.root_node(), &context, &mut symbols);
    let mut markers = Vec::new();
    let mut problems = Vec::new();
    let reference_path = path
        .iter()
        .map(|component| component.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    for (index, &comment) in comments.iter().enumerate() {
        // Extend ownership across captured comments only. Keep the original
        // node for lexical scope and locations, and leave content spans alone.
        let mut group_end = comment.end_byte();
        for next in &comments[index + 1..] {
            if next.start_byte() < group_end
                || !source[group_end..next.start_byte()].trim().is_empty()
            {
                break;
            }
            group_end = next.end_byte();
        }
        let raw = &source[comment.byte_range()];
        let mut offset = comment.start_byte();
        for line in raw.split_inclusive('\n') {
            let cleaned = line
                .trim()
                .trim_start_matches(|ch: char| {
                    ch.is_whitespace() || ch.is_ascii_punctuation() && ch != '@'
                })
                .trim_end_matches(|ch: char| ch.is_whitespace() || ch.is_ascii_punctuation());
            if let Some(marker) = cleaned
                .strip_prefix("@mara")
                .filter(|rest| rest.chars().next().is_none_or(char::is_whitespace))
            {
                let marker_source = location(&path, &lines, offset, offset + line.len());
                let parts = marker.split_whitespace().collect::<Vec<_>>();
                if parts.len() != 2
                    || !crate::is_snake_name(parts[0])
                    || !crate::is_item_id(parts[1]) && !crate::is_mid(parts[1])
                {
                    problems.push(CodeProblem {
                        code: DiagnosticCode::CodeUnsupported,
                        message: "invalid code marker; expected @mara <relation> <item-ID-or-MID>"
                            .into(),
                        source: marker_source,
                        target: parts
                            .get(1)
                            .filter(|target| crate::is_item_id(target) || crate::is_mid(target))
                            .map(|target| (*target).to_owned()),
                    });
                } else {
                    match adapter.attached_symbol(comment, group_end, &source, &symbols) {
                        Ok(symbol) => {
                            let endpoint = symbol.map_or_else(
                                || format!("code:{reference_path}"),
                                |symbol| format!("code:{reference_path}::{}", symbol.selector),
                            );
                            markers.push(CodeMarker {
                                relation: parts[0].into(),
                                target: parts[1].into(),
                                endpoint,
                                source: marker_source,
                                owner_span: symbol.map(|symbol| {
                                    (
                                        symbol.source.span().start_byte(),
                                        symbol.source.span().end_byte(),
                                    )
                                }),
                            });
                        }
                        Err(error) => problems.push(CodeProblem {
                            code: DiagnosticCode::CodeUnsupported,
                            message: match error {
                                AttachmentError::Ambiguous => "ambiguous comment attachment",
                                AttachmentError::Unsupported => {
                                    "comment has no supported code owner"
                                }
                            }
                            .into(),
                            source: marker_source,
                            target: Some(parts[1].to_owned()),
                        }),
                    }
                }
            }
            offset += line.len();
        }
    }
    (
        CodeFile {
            path,
            source,
            symbols,
            markers,
        },
        problems,
    )
}

struct CollectContext<'tree, 'data> {
    declarations: &'data BTreeMap<usize, (Node<'tree>, bool)>,
    modifiers: &'data BTreeSet<usize>,
    wrappers: &'data BTreeSet<usize>,
    comments: &'data BTreeSet<usize>,
    path: &'data Path,
    lines: &'data [usize],
}

fn collect<'tree>(
    node: Node<'tree>,
    context: &CollectContext<'tree, '_>,
    symbols: &mut Vec<CodeSymbol>,
) {
    let declaration = context.declarations.get(&node.id());
    if let Some((name_node, selectable)) = &declaration
        && *selectable
    {
        let body = node.child_by_field_name("body");
        let mut attach_starts = vec![node.start_byte()];
        let mut outer = node;
        while let Some(wrapper) = outer
            .parent()
            .filter(|p| context.wrappers.contains(&p.id()))
        {
            attach_starts.push(wrapper.start_byte());
            outer = wrapper;
        }
        let mut previous = node.prev_named_sibling();
        while let Some(sibling) = previous {
            if context.modifiers.contains(&sibling.id()) {
                attach_starts.push(sibling.start_byte());
            } else if !context.comments.contains(&sibling.id()) {
                break;
            }
            previous = sibling.prev_named_sibling();
        }
        let mut cursor = node.walk();
        let mut has_leading_modifier = false;
        for child in node.children(&mut cursor) {
            if context.modifiers.contains(&child.id()) {
                attach_starts.push(child.start_byte());
                has_leading_modifier = true;
            } else if !context.comments.contains(&child.id()) {
                if has_leading_modifier {
                    attach_starts.push(child.start_byte());
                }
                break;
            }
        }
        let content_start = attach_starts
            .iter()
            .copied()
            .min()
            .unwrap_or(node.start_byte());
        symbols.push(CodeSymbol {
            selector: String::new(),
            identity: String::new(),
            source: location(
                context.path,
                context.lines,
                name_node.start_byte(),
                name_node.end_byte(),
            ),
            content: location(context.path, context.lines, content_start, outer.end_byte()),
            body_start: body.map_or(node.start_byte(), |b| b.start_byte()),
            body_end: body.map_or(node.end_byte(), |b| b.end_byte()),
            attach_starts,
        });
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect(child, context, symbols);
    }
}

fn valid_asset_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn read_project_asset(project: &Project, path: &Path) -> Result<Vec<u8>, String> {
    let resolved = fs::canonicalize(project.root().join(path)).map_err(|e| e.to_string())?;
    if !resolved.starts_with(project.root()) || !resolved.is_file() {
        return Err("adapter asset must be a regular file within the project".into());
    }
    fs::read(resolved).map_err(|e| e.to_string())
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    #[test]
    fn code_references_require_raw_ordinary_path_components() {
        for reference in [
            "code:src/./part.rs::rust::run().",
            "code:src//part.rs::rust::run().",
            "code:src/../part.rs::rust::run().",
            "code:src/part.rs/::rust::run().",
        ] {
            assert_eq!(split_reference(reference), Err(ReferenceError::Unsupported));
        }
        assert_eq!(
            split_reference("code:src/.hidden.rs::rust::run()."),
            Ok((PathBuf::from("src/.hidden.rs"), Some("rust::run().")))
        );
    }
}
