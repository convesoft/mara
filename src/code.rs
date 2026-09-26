//! Disposable, local code projection. Adapters own syntax and symbol selection;
//! the shared layer owns target grammar, marker meaning and endpoint identity.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use ignore::WalkBuilder;
use serde::Deserialize;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::{DiagnosticCode, Project, SourceLocation, corpus::location};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeIndex {
    root: PathBuf,
    files: BTreeMap<PathBuf, CodeFile>,
    assets: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeFile {
    pub path: PathBuf,
    pub source: String,
    pub symbols: Vec<CodeSymbol>,
    pub markers: Vec<CodeMarker>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeSymbol {
    pub selector: String,
    pub source: SourceLocation,
    pub content: SourceLocation,
    body_start: usize,
    body_end: usize,
    attach_starts: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeMarker {
    pub relation: String,
    pub target: String,
    pub endpoint: String,
    pub source: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeProblem {
    pub code: DiagnosticCode,
    pub message: String,
    pub source: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveError {
    MissingFile,
    MissingSymbol,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LanguageConfig {
    name: String,
    extensions: Vec<String>,
    grammar: PathBuf,
    query: PathBuf,
    separator: String,
}

struct Adapter {
    extensions: Vec<String>,
    separator: String,
    parser: Parser,
    query: Query,
}

impl Adapter {
    fn accepts(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                self.extensions
                    .iter()
                    .any(|candidate| candidate == extension)
            })
    }

    fn load(project: &Project) -> Result<(Vec<Self>, Vec<PathBuf>), CodeProblem> {
        let config_path = Path::new(crate::PROJECT_FILE);
        let fail = |message: String| CodeProblem {
            code: DiagnosticCode::CodeUnsupported,
            message,
            source: location(config_path, &[0], 0, 0),
        };
        let mut adapters = Vec::new();
        let mut assets = vec![config_path.to_path_buf()];
        let mut assigned = std::collections::BTreeSet::new();
        for language in project.code_languages() {
            if language.name.is_empty()
                || language.extensions.is_empty()
                || language.separator.is_empty()
                || !valid_asset_path(&language.grammar)
                || !valid_asset_path(&language.query)
                || language.extensions.iter().any(|extension| {
                    extension.is_empty()
                        || !extension.chars().all(|ch| ch.is_ascii_alphanumeric())
                        || !assigned.insert(extension.clone())
                })
            {
                return Err(fail(format!(
                    "invalid or duplicate adapter configuration for {}",
                    language.name
                )));
            }
            let grammar = read_project_asset(project, &language.grammar)
                .map_err(|e| fail(format!("{}: {e}", language.grammar.display())))?;
            let query_source = String::from_utf8(
                read_project_asset(project, &language.query)
                    .map_err(|e| fail(format!("{}: {e}", language.query.display())))?,
            )
            .map_err(|e| fail(format!("{}: {e}", language.query.display())))?;
            let engine = tree_sitter::wasmtime::Engine::default();
            let mut store =
                tree_sitter::WasmStore::new(&engine).map_err(|e| fail(e.to_string()))?;
            let grammar = store
                .load_language(&language.name, &grammar)
                .map_err(|e| fail(format!("{}: {e}", language.grammar.display())))?;
            let query = Query::new(&grammar, &query_source)
                .map_err(|e| fail(format!("{}: {e}", language.query.display())))?;
            if query.capture_index_for_name("name").is_none()
                || query.capture_index_for_name("symbol").is_none()
                || query.capture_index_for_name("comment").is_none()
            {
                return Err(fail(format!(
                    "{} must capture @symbol, @name and @comment",
                    language.query.display()
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
                separator: language.separator.clone(),
                parser,
                query,
            });
            assets.extend([language.grammar.clone(), language.query.clone()]);
        }
        Ok((adapters, assets))
    }

    fn attached_symbol<'a>(
        &self,
        comment: Node<'_>,
        source: &str,
        symbols: &'a [CodeSymbol],
    ) -> Result<Option<&'a CodeSymbol>, ()> {
        let mut attached = symbols
            .iter()
            .filter_map(|s| {
                s.attach_starts
                    .iter()
                    .copied()
                    .filter(|start| {
                        *start >= comment.end_byte()
                            && source[comment.end_byte()..*start].trim().is_empty()
                    })
                    .min()
                    .map(|start| (s, start))
            })
            .collect::<Vec<_>>();
        attached.sort_by_key(|(_, start)| *start);
        if let Some(first) = attached.first() {
            if attached.get(1).is_some_and(|next| next.1 == first.1) {
                return Err(());
            }
            return Ok(Some(first.0));
        }
        let mut enclosing = symbols
            .iter()
            .filter(|s| s.body_start <= comment.start_byte() && comment.end_byte() <= s.body_end)
            .collect::<Vec<_>>();
        enclosing.sort_by_key(|s| s.body_end - s.body_start);
        if let Some(first) = enclosing.first() {
            if enclosing.get(1).is_some_and(|next| {
                next.body_end - next.body_start == first.body_end - first.body_start
            }) {
                return Err(());
            }
            return Ok(Some(first));
        }
        Ok(None)
    }
}

pub(crate) fn split_reference(reference: &str) -> Result<(PathBuf, Option<&str>), ResolveError> {
    let rest = reference
        .strip_prefix("code:")
        .ok_or(ResolveError::Unsupported)?;
    let (path, selector) = rest
        .split_once("::")
        .map_or((rest, None), |(p, s)| (p, Some(s)));
    if path.is_empty()
        || selector.is_some_and(str::is_empty)
        || path.contains('\\')
        || path.chars().any(char::is_whitespace)
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || Path::new(path).is_absolute()
    {
        return Err(ResolveError::Unsupported);
    }
    Ok((PathBuf::from(path), selector))
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

impl CodeIndex {
    pub(crate) fn load(project: &Project) -> (Self, Vec<CodeProblem>) {
        let mut result = Self {
            root: project.root().to_owned(),
            files: BTreeMap::new(),
            assets: vec![PathBuf::from(crate::PROJECT_FILE)],
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
        if adapters.is_empty() {
            return (result, problems);
        }
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
                    });
                    continue;
                }
            };
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
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
                    });
                    continue;
                }
            };
            let (file, mut file_problems) = parse_file(path.clone(), source, adapter);
            problems.append(&mut file_problems);
            result.files.insert(path, file);
        }
        (result, problems)
    }

    pub(crate) fn empty(project: &Project) -> Self {
        Self {
            root: project.root().to_owned(),
            files: BTreeMap::new(),
            assets: vec![PathBuf::from(crate::PROJECT_FILE)],
        }
    }

    pub(crate) fn assets(&self) -> impl Iterator<Item = &PathBuf> {
        self.assets.iter()
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = &CodeFile> {
        self.files.values()
    }

    pub(crate) fn file_only_bytes(&self, path: &Path) -> Option<Vec<u8>> {
        let canonical = fs::canonicalize(self.root.join(path)).ok()?;
        if !canonical.starts_with(&self.root) || !canonical.is_file() {
            return None;
        }
        fs::read(canonical).ok()
    }

    pub(crate) fn resolve(&self, reference: &str) -> Result<CodeResolved, ResolveError> {
        let (path, selector) = split_reference(reference)?;
        let absolute = self.root.join(&path);
        let canonical = fs::canonicalize(&absolute).map_err(|_| ResolveError::MissingFile)?;
        if !canonical.starts_with(&self.root) || !canonical.is_file() {
            return Err(ResolveError::Unsupported);
        }
        let Some(selector) = selector else {
            let content = fs::read(&absolute)
                .map_err(|_| ResolveError::Unsupported)
                .map(|bytes| String::from_utf8(bytes).ok())?;
            let lines = content
                .as_deref()
                .map(line_starts)
                .unwrap_or_else(|| vec![0]);
            let length = content.as_ref().map_or(0, String::len);
            return Ok(CodeResolved {
                reference: reference.to_owned(),
                source: location(&path, &lines, 0, length),
                content,
                symbol: None,
            });
        };
        let file = self.files.get(&path).ok_or(ResolveError::Unsupported)?;
        let mut matches = file
            .symbols
            .iter()
            .filter(|symbol| symbol.selector == selector);
        let symbol = matches.next().ok_or(ResolveError::MissingSymbol)?;
        if matches.next().is_some() {
            return Err(ResolveError::Ambiguous);
        }
        Ok(CodeResolved {
            reference: reference.to_owned(),
            source: symbol.source.clone(),
            content: Some(
                file.source[symbol.content.span().start_byte()..symbol.content.span().end_byte()]
                    .to_owned(),
            ),
            symbol: Some(symbol.selector.clone()),
        })
    }
}

pub(crate) struct CodeResolved {
    pub reference: String,
    pub source: SourceLocation,
    pub content: Option<String>,
    pub symbol: Option<String>,
}

impl CodeResolved {
    pub(crate) fn summary(&self) -> crate::DiscoveryNodeSummary {
        crate::DiscoveryNodeSummary {
            reference: self.reference.clone(),
            kind: crate::DiscoveryKind::Code,
            source: (&self.source).into(),
            title: self
                .symbol
                .clone()
                .or_else(|| Some(self.source.path().display().to_string())),
            title_truncated: false,
            context: crate::DiscoveryContext {
                parent: None,
                section: None,
            },
            id: None,
            mid: None,
            flavour: None,
            block_kind: None,
            heading_level: None,
        }
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
            if matches!(
                name_node.kind(),
                "computed_property_name" | "string" | "number"
            ) {
                continue;
            }
            if let Some(name) = source.get(name_node.byte_range()) {
                declarations.insert(node.id(), (name.to_owned(), name_node, selectable));
            }
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
        separator: &adapter.separator,
        path: &path,
        lines: &lines,
    };
    collect(tree.root_node(), &context, &mut Vec::new(), &mut symbols);
    let mut markers = Vec::new();
    let mut problems = Vec::new();
    for comment in comments {
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
                    });
                } else {
                    match adapter.attached_symbol(comment, &source, &symbols) {
                        Ok(symbol) => {
                            let endpoint = symbol.map_or_else(
                                || format!("code:{}", path.display()),
                                |symbol| format!("code:{}::{}", path.display(), symbol.selector),
                            );
                            markers.push(CodeMarker {
                                relation: parts[0].into(),
                                target: parts[1].into(),
                                endpoint,
                                source: marker_source,
                            });
                        }
                        Err(()) => problems.push(CodeProblem {
                            code: DiagnosticCode::CodeUnsupported,
                            message: "ambiguous comment attachment".into(),
                            source: marker_source,
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
    declarations: &'data BTreeMap<usize, (String, Node<'tree>, bool)>,
    modifiers: &'data BTreeSet<usize>,
    wrappers: &'data BTreeSet<usize>,
    comments: &'data BTreeSet<usize>,
    separator: &'data str,
    path: &'data Path,
    lines: &'data [usize],
}

fn collect<'tree>(
    node: Node<'tree>,
    context: &CollectContext<'tree, '_>,
    prefix: &mut Vec<String>,
    symbols: &mut Vec<CodeSymbol>,
) {
    let declaration = context.declarations.get(&node.id());
    if let Some((name, name_node, selectable)) = &declaration {
        prefix.push(name.clone());
        if *selectable {
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
                selector: prefix.join(context.separator),
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
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect(child, context, prefix, symbols);
    }
    if declaration.is_some() {
        prefix.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_references_require_raw_ordinary_path_components() {
        for reference in [
            "code:src/./part.rs::run",
            "code:src//part.rs::run",
            "code:src/../part.rs::run",
            "code:src/part.rs/::run",
        ] {
            assert_eq!(split_reference(reference), Err(ResolveError::Unsupported));
        }
        assert_eq!(
            split_reference("code:src/.hidden.rs::run"),
            Ok((PathBuf::from("src/.hidden.rs"), Some("run")))
        );
    }

    fn adapters() -> Vec<Adapter> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let project = crate::resolve_project(Some(root), root).unwrap();
        Adapter::load(&project).unwrap().0
    }

    #[test]
    fn adapters_attach_markers_to_methods_and_nested_functions() {
        let cases = [
            (
                "sample.rs",
                "// @mara implements REQ-A\nmod outer { struct Worker; impl Worker { fn run() { // @mara verifies REQ-A\n fn check() {} } } }",
                "outer::Worker::run",
                "outer::Worker::run::check",
            ),
            (
                "sample.py",
                "class Outer:\n    # @mara implements REQ-A\n    def run(self):\n        # @mara verifies REQ-A\n        def check(): pass\n",
                "Outer.run",
                "Outer.run.check",
            ),
            (
                "sample.js",
                "class Outer { // @mara implements REQ-A\n run() { // @mara verifies REQ-A\n function check() {} } }",
                "Outer.run",
                "Outer.run.check",
            ),
            (
                "sample.ts",
                "class Outer { // @mara implements REQ-A\n run(): void { // @mara verifies REQ-A\n function check(): void {} } }",
                "Outer.run",
                "Outer.run.check",
            ),
        ];
        let mut adapters = adapters();
        for (path, source, method, nested) in cases {
            let adapter = adapters
                .iter_mut()
                .find(|adapter| adapter.accepts(Path::new(path)))
                .unwrap();
            let (file, problems) = parse_file(path.into(), source.into(), adapter);
            assert!(problems.is_empty(), "{path}: {problems:?}");
            assert!(
                file.symbols.iter().any(|s| s.selector == method),
                "{path}: {:?}",
                file.symbols.iter().map(|s| &s.selector).collect::<Vec<_>>()
            );
            assert!(
                file.symbols.iter().any(|s| s.selector == nested),
                "{path}: {:?}",
                file.symbols.iter().map(|s| &s.selector).collect::<Vec<_>>()
            );
            assert_eq!(file.markers.len(), 2, "{path}: {:?}", file.markers);
        }
    }

    #[test]
    fn shared_marker_parser_reads_block_comments_from_tree_sitter() {
        let mut adapters = adapters();
        let adapter = adapters
            .iter_mut()
            .find(|adapter| adapter.accepts(Path::new("sample.js")))
            .unwrap();
        let (file, problems) = parse_file(
            "sample.js".into(),
            "/* @mara code_implements REQ-A */\nfunction run() {}\n".into(),
            adapter,
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(file.markers[0].endpoint, "code:sample.js::run");
    }

    #[test]
    fn shared_marker_parser_requires_the_complete_introducer() {
        let mut adapters = adapters();
        let adapter = adapters
            .iter_mut()
            .find(|adapter| adapter.accepts(Path::new("sample.js")))
            .unwrap();
        let (file, problems) = parse_file(
            "sample.js".into(),
            "// @marathon is an ordinary comment\n// @mara code_implements REQ-A\nfunction run() {}\n"
                .into(),
            adapter,
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(file.markers.len(), 1);
        assert_eq!(file.markers[0].endpoint, "code:sample.js::run");
    }

    #[test]
    fn adapters_attach_markers_through_declaration_modifiers() {
        let cases = [
            (
                "sample.rs",
                "trait Api {\n    /// @mara code_implements REQ-A\n    fn run(&self);\n}\n",
                "code:sample.rs::Api::run",
            ),
            (
                "sample.rs",
                "// @mara code_implements REQ-A\n#[test]\nfn run() {}\n",
                "code:sample.rs::run",
            ),
            (
                "sample.rs",
                "#[test]\n// @mara code_implements REQ-A\nfn run() {}\n",
                "code:sample.rs::run",
            ),
            (
                "sample.py",
                "# @mara code_implements REQ-A\n@decorator\ndef run(): pass\n",
                "code:sample.py::run",
            ),
            (
                "sample.py",
                "@decorator\n# @mara code_implements REQ-A\ndef run(): pass\n",
                "code:sample.py::run",
            ),
            (
                "sample.js",
                "// @mara code_implements REQ-A\nexport function run() {}\n",
                "code:sample.js::run",
            ),
            (
                "sample.js",
                "// @mara code_implements REQ-A\n@sealed\nclass Service {}\n",
                "code:sample.js::Service",
            ),
            (
                "sample.ts",
                "// @mara code_implements REQ-A\nexport function run(): void {}\n",
                "code:sample.ts::run",
            ),
            (
                "sample.ts",
                "// @mara code_implements REQ-A\n@sealed\nclass Service {}\n",
                "code:sample.ts::Service",
            ),
            (
                "sample.ts",
                "@sealed\n// @mara code_implements REQ-A\nclass Service {}\n",
                "code:sample.ts::Service",
            ),
        ];
        let mut adapters = adapters();
        for (path, source, endpoint) in cases {
            let adapter = adapters
                .iter_mut()
                .find(|adapter| adapter.accepts(Path::new(path)))
                .unwrap();
            let (file, problems) = parse_file(path.into(), source.into(), adapter);
            assert!(problems.is_empty(), "{path}: {problems:?}");
            assert!(
                file.symbols
                    .iter()
                    .any(|s| format!("code:{path}::{}", s.selector) == endpoint)
            );
            assert_eq!(file.markers.len(), 1, "{path}: {:?}", file.markers);
            assert_eq!(file.markers[0].endpoint, endpoint, "{path}");
        }
    }

    #[test]
    fn symbol_content_includes_attached_modifiers() {
        let cases = [
            (
                "sample.rs",
                "#[test]\nfn run() {}\n",
                "run",
                "#[test]\nfn run() {}",
            ),
            (
                "sample.py",
                "@decorator\ndef run(): pass\n",
                "run",
                "@decorator\ndef run(): pass",
            ),
            (
                "sample.js",
                "export function run() {}\n",
                "run",
                "export function run() {}",
            ),
            (
                "sample.ts",
                "@sealed\nclass Service {}\n",
                "Service",
                "@sealed\nclass Service {}",
            ),
        ];
        let mut adapters = adapters();
        for (path, source, selector, expected) in cases {
            let adapter = adapters
                .iter_mut()
                .find(|adapter| adapter.accepts(Path::new(path)))
                .unwrap();
            let (file, problems) = parse_file(path.into(), source.into(), adapter);
            assert!(problems.is_empty(), "{path}: {problems:?}");
            let symbol = file
                .symbols
                .iter()
                .find(|symbol| symbol.selector == selector)
                .unwrap();
            let content =
                &file.source[symbol.content.span().start_byte()..symbol.content.span().end_byte()];
            assert_eq!(content, expected, "{path}");
        }
    }
}
