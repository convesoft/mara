//! Invoke project-configured indexers and project their semantic identities locally.
use super::*;
use protobuf::Message;
use scip::{symbol, types};
use sha2::{Digest, Sha256};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Encoding {
    Utf8,
    Utf16,
    Utf32,
}

type Snapshot = BTreeMap<PathBuf, Vec<u8>>;
pub(super) struct Indexed {
    config: LanguageConfig,
    index: types::Index,
    inputs: Vec<PathBuf>,
}

fn problem(message: impl Into<String>) -> CodeProblem {
    CodeProblem {
        code: DiagnosticCode::SourceInvalid,
        message: message.into(),
        source: location(Path::new(crate::PROJECT_FILE), &[0], 0, 0),
        target: None,
    }
}

fn snapshot(project: &Project) -> Result<Snapshot, CodeProblem> {
    let mut result = BTreeMap::new();
    let mut walk = WalkBuilder::new(project.root());
    walk.hidden(false)
        .ignore(false)
        .git_ignore(true)
        .git_exclude(false)
        .git_global(false)
        .parents(true)
        .require_git(false)
        .follow_links(false);
    for entry in walk.build() {
        let entry = entry.map_err(|_| problem("could not snapshot indexer inputs"))?;
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
        if matches!(
            path.to_str(),
            Some(".mara/mutation.lock" | ".mara/transaction.json")
        ) || path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".mara-stage-"))
        {
            continue;
        }
        let bytes = fs::read(entry.path()).map_err(|_| problem("could not read indexer input"))?;
        result.insert(path, Sha256::digest(bytes).to_vec());
    }
    Ok(result)
}

// @mara implements DES-CODE-TRACEABILITY
pub(super) fn run(project: &Project) -> Result<Vec<Indexed>, CodeProblem> {
    let mut names = BTreeSet::new();
    let mut result = Vec::new();
    for config in project.code_languages() {
        if !crate::is_snake_name(&config.name)
            || !names.insert(config.name.clone())
            || config.command.is_empty()
            || config.command[0].is_empty()
            || config
                .command
                .iter()
                .filter(|arg| arg.as_str() == "{output}")
                .count()
                != 1
        {
            return Err(problem(
                "code languages need a unique snake_case name and command with one {output} argument",
            ));
        }
        let before = snapshot(project)?;
        if !before
            .keys()
            .any(|path| matches_extension(path, &config.extensions))
        {
            continue;
        }
        let output = tempfile::NamedTempFile::new()
            .map_err(|_| problem("could not create temporary SCIP output"))?;
        let status = Command::new(&config.command[0])
            .args(config.command[1..].iter().map(|arg| {
                if arg == "{output}" {
                    output.path().as_os_str().to_owned()
                } else {
                    arg.into()
                }
            }))
            .current_dir(project.root())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| {
                problem(format!(
                    "could not start SCIP indexer {}: {}",
                    config.name,
                    error.kind()
                ))
            })?;
        if !status.success() {
            return Err(problem(format!(
                "SCIP indexer {} failed ({status}); inspect its configured command",
                config.name
            )));
        }
        if before != snapshot(project)? {
            return Err(problem(format!(
                "project inputs changed while SCIP indexer {} ran; retry on stable inputs",
                config.name
            )));
        }
        let bytes = fs::read(output.path()).map_err(|_| problem("could not read SCIP output"))?;
        if bytes.is_empty() {
            return Err(problem(format!(
                "SCIP indexer {} produced no index",
                config.name
            )));
        }
        let index = types::Index::parse_from_bytes(&bytes)
            .map_err(|_| problem("invalid SCIP protobuf output"))?;
        let root_matches = index.metadata.as_ref().is_some_and(|metadata| {
            let uri_root = url::Url::parse(&metadata.project_root)
                .ok()
                .and_then(|url| url.to_file_path().ok())
                .and_then(|path| fs::canonicalize(path).ok());
            // Some emitters put an unescaped absolute path after file://.
            // Accept only an exact match to the known root, never another path.
            uri_root.as_deref() == Some(project.root())
                || metadata.project_root == format!("file://{}", project.root().display())
        });
        if !root_matches {
            return Err(problem(format!(
                "SCIP indexer {} returned a different project root",
                config.name
            )));
        }
        result.push(Indexed {
            config: config.clone(),
            index,
            inputs: before.into_keys().collect(),
        });
    }
    Ok(result)
}

// Escape only characters that conflict with Mara inline tokens or percent escapes.
// Preserve the descriptor losslessly; identity comes from the indexer.
fn escape_descriptor(descriptor: &str) -> String {
    let mut result = String::new();
    for ch in descriptor.chars() {
        if ch.is_whitespace() || matches!(ch, '%' | '[' | ']' | '<' | '>' | '\\' | '|') {
            let mut bytes = [0; 4];
            for byte in ch.encode_utf8(&mut bytes).bytes() {
                result.push_str(&format!("%{byte:02X}"));
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn selector(indexer: &str, full: &str) -> Result<Option<String>, CodeProblem> {
    if symbol::is_local_symbol(full) {
        return Ok(None);
    }
    let parsed = symbol::parse_symbol(full)
        .map_err(|_| problem("indexer emitted an invalid SCIP symbol"))?;
    let descriptor = symbol::format_symbol_with(
        parsed,
        symbol::SymbolFormatOptions {
            include_scheme: false,
            include_package_manager: false,
            include_package_name: false,
            include_package_version: false,
            include_descriptor: true,
        },
    );
    if descriptor.is_empty() {
        return Err(problem("indexer emitted an empty SCIP descriptor"));
    }
    Ok(Some(format!(
        "{indexer}::{}",
        escape_descriptor(&descriptor)
    )))
}

fn offset(
    source: &str,
    lines: &[usize],
    line: i32,
    column: i32,
    encoding: Encoding,
) -> Option<usize> {
    let start = *lines.get(usize::try_from(line).ok()?)?;
    let text = source[start..].split('\n').next()?;
    let wanted = usize::try_from(column).ok()?;
    if encoding == Encoding::Utf8 {
        return (wanted <= text.len() && text.is_char_boundary(wanted)).then_some(start + wanted);
    }
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == wanted {
            return Some(start + byte);
        }
        units += if encoding == Encoding::Utf16 {
            ch.len_utf16()
        } else {
            1
        };
    }
    (units == wanted).then_some(start + text.len())
}

fn span(
    source: &str,
    lines: &[usize],
    range: &[i32],
    encoding: Encoding,
) -> Option<(usize, usize)> {
    let (sl, sc, el, ec) = match *range {
        [sl, sc, ec] => (sl, sc, sl, ec),
        [sl, sc, el, ec] => (sl, sc, el, ec),
        _ => return None,
    };
    let start = offset(source, lines, sl, sc, encoding)?;
    let end = offset(source, lines, el, ec, encoding)?;
    (start <= end).then_some((start, end))
}

fn enclosing_range(occurrence: &types::Occurrence) -> Vec<i32> {
    if occurrence.has_single_line_enclosing_range() {
        let r = occurrence.single_line_enclosing_range();
        vec![r.line, r.start_character, r.end_character]
    } else if occurrence.has_multi_line_enclosing_range() {
        let r = occurrence.multi_line_enclosing_range();
        vec![r.start_line, r.start_character, r.end_line, r.end_character]
    } else {
        occurrence.enclosing_range.clone()
    }
}

fn occurrence_range(occurrence: &types::Occurrence) -> Vec<i32> {
    if occurrence.has_single_line_range() {
        let r = occurrence.single_line_range();
        vec![r.line, r.start_character, r.end_character]
    } else if occurrence.has_multi_line_range() {
        let r = occurrence.multi_line_range();
        vec![r.start_line, r.start_character, r.end_line, r.end_character]
    } else {
        occurrence.range.clone()
    }
}

// @mara implements REQ-CODE-TRACEABILITY
pub(super) fn apply(
    project: &Project,
    indexes: Vec<Indexed>,
    result: &mut CodeIndex,
    problems: &mut Vec<CodeProblem>,
) {
    let syntax = result
        .files
        .iter_mut()
        .map(|(path, file)| (path.clone(), std::mem::take(&mut file.symbols)))
        .collect::<BTreeMap<_, _>>();
    let mut identities = BTreeMap::new();
    for indexed in indexes {
        result.assets.extend(indexed.inputs);
        for document in indexed.index.documents {
            let path = PathBuf::from(&document.relative_path);
            if !valid_asset_path(&path) {
                problems.push(problem("SCIP document path must stay within the project"));
                continue;
            }
            let source = match read_project_asset(project, &path)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
            {
                Some(source) => source,
                None => {
                    problems.push(problem("SCIP document source is missing or not UTF-8"));
                    continue;
                }
            };
            if !document.text.is_empty() && document.text != source {
                problems.push(problem("SCIP document text differs from current source"));
                continue;
            }
            let encoding = match document.position_encoding.enum_value() {
                Ok(types::PositionEncoding::UTF8CodeUnitOffsetFromLineStart) => {
                    Some(Encoding::Utf8)
                }
                Ok(types::PositionEncoding::UTF16CodeUnitOffsetFromLineStart) => {
                    Some(Encoding::Utf16)
                }
                Ok(types::PositionEncoding::UTF32CodeUnitOffsetFromLineStart) => {
                    Some(Encoding::Utf32)
                }
                _ => indexed
                    .config
                    .position_encoding
                    .or_else(|| source.is_ascii().then_some(Encoding::Utf8)),
            };
            let Some(encoding) = encoding else {
                problems.push(problem(format!("SCIP indexer {} omitted position encoding for non-ASCII source; configure its documented position_encoding", indexed.config.name)));
                continue;
            };
            let lines = line_starts(&source);
            let file = result
                .files
                .entry(path.clone())
                .or_insert_with(|| CodeFile {
                    path: path.clone(),
                    source: source.clone(),
                    symbols: vec![],
                    markers: vec![],
                });
            for occurrence in document.occurrences {
                if occurrence.symbol_roles & 1 == 0 || occurrence.symbol.is_empty() {
                    continue;
                }
                let selector = match selector(&indexed.config.name, &occurrence.symbol) {
                    Ok(Some(selector)) => selector,
                    Ok(None) => continue,
                    Err(error) => {
                        problems.push(error);
                        continue;
                    }
                };
                let identity = identities
                    .entry((path.clone(), selector.clone()))
                    .or_insert_with(|| occurrence.symbol.clone());
                if identity != &occurrence.symbol {
                    problems.push(problem("distinct SCIP symbols collide after local scoping"));
                    continue;
                }
                let Some((start, end)) =
                    span(&source, &lines, &occurrence_range(&occurrence), encoding)
                else {
                    problems.push(problem(
                        "SCIP definition range is invalid for current source",
                    ));
                    continue;
                };
                let enclosing = enclosing_range(&occurrence);
                let enclosing_span = if enclosing.is_empty() {
                    (start, end)
                } else if let Some(range) = span(&source, &lines, &enclosing, encoding) {
                    range
                } else {
                    problems.push(problem(
                        "SCIP enclosing range is invalid for current source",
                    ));
                    continue;
                };
                let syntax_symbol = syntax.get(&path).and_then(|symbols| {
                    symbols.iter().find(|symbol| {
                        symbol.source.span().start_byte() == start
                            && symbol.source.span().end_byte() == end
                    })
                });
                let content = syntax_symbol
                    .map(|symbol| symbol.content.clone())
                    .unwrap_or_else(|| location(&path, &lines, enclosing_span.0, enclosing_span.1));
                if file.symbols.iter().any(|symbol| {
                    symbol.selector == selector
                        && symbol.source.span().start_byte() == start
                        && symbol.source.span().end_byte() == end
                }) {
                    continue;
                }
                file.symbols.push(CodeSymbol {
                    selector,
                    identity: occurrence.symbol.clone(),
                    source: location(&path, &lines, start, end),
                    content,
                    body_start: start,
                    body_end: end,
                    attach_starts: vec![],
                });
            }
        }
    }
    for file in result.files.values_mut() {
        for marker in &mut file.markers {
            let Some(owner) = marker.owner_span else {
                continue;
            };
            let selectors = file
                .symbols
                .iter()
                .filter(|symbol| {
                    (
                        symbol.source.span().start_byte(),
                        symbol.source.span().end_byte(),
                    ) == owner
                })
                .map(|symbol| symbol.selector.as_str())
                .collect::<BTreeSet<_>>();
            if selectors.len() == 1 {
                marker.endpoint = format!(
                    "code:{}::{}",
                    file.path.to_string_lossy().replace('\\', "/"),
                    selectors.first().unwrap()
                );
            } else {
                problems.push(CodeProblem {
                    code: DiagnosticCode::CodeUnsupported,
                    message: "comment owner has no unique global SCIP symbol".into(),
                    source: marker.source.clone(),
                    target: Some(marker.target.clone()),
                });
            }
        }
    }
    result.assets.sort();
    result.assets.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_preserve_disambiguators_but_omit_package_versions() {
        let first = selector(
            "typescript",
            "scip-typescript npm demo 1.0.0 `service.ts`/parse().",
        )
        .unwrap();
        assert_eq!(first.as_deref(), Some("typescript::`service.ts`/parse()."));
        assert_eq!(
            first,
            selector(
                "typescript",
                "scip-typescript npm demo 2.0.0 `service.ts`/parse()."
            )
            .unwrap()
        );
        assert_ne!(
            selector("cpp", "cxx . . $ parse(abc).").unwrap(),
            selector("cpp", "cxx . . $ parse(def).").unwrap()
        );
        assert_eq!(selector("rust", "local 1").unwrap(), None);
    }

    #[test]
    fn positions_respect_unicode_units_and_character_boundaries() {
        let source = "a😀é\nnext";
        let lines = line_starts(source);
        assert_eq!(offset(source, &lines, 0, 3, Encoding::Utf16), Some(5));
        assert_eq!(offset(source, &lines, 0, 2, Encoding::Utf16), None);
        assert_eq!(offset(source, &lines, 0, 2, Encoding::Utf32), Some(5));
        assert_eq!(offset(source, &lines, 0, 5, Encoding::Utf8), Some(5));
        assert_eq!(offset(source, &lines, 0, 2, Encoding::Utf8), None);
        assert_eq!(offset(source, &lines, 1, 0, Encoding::Utf8), Some(8));
    }
}
