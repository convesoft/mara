use crate::{DiagnosticCode, Project, SourceLocation, corpus::location};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
#[derive(Debug, Clone)]
pub struct CodeIndex {
    pub root: PathBuf,
    pub files: BTreeMap<PathBuf, CodeFile>,
    pub assets: Vec<PathBuf>,
    pub inputs: Option<Arc<dyn CodeSource>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeFile {
    pub path: PathBuf,
    pub source: String,
    pub symbols: Vec<CodeSymbol>,
    pub markers: Vec<CodeMarker>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSymbol {
    pub selector: String,
    pub identity: String,
    pub source: SourceLocation,
    pub content: SourceLocation,
    pub body_start: usize,
    pub body_end: usize,
    pub attach_starts: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeMarker {
    pub relation: String,
    pub target: String,
    pub endpoint: String,
    pub source: SourceLocation,
    pub owner_span: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeProblem {
    pub code: DiagnosticCode,
    pub message: String,
    pub source: SourceLocation,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceError {
    MissingFile,
    MissingSymbol,
    Ambiguous,
    Unsupported,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    pub name: String,
    pub command: Vec<String>,
    /// Compatibility for indexers predating SCIP's per-document position encoding.
    #[serde(default)]
    pub position_encoding: Option<Encoding>,
    #[serde(default)]
    pub extensions: Vec<String>,
    pub grammar: Option<PathBuf>,
    pub query: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    Utf8,
    Utf16,
    Utf32,
}
/// Native host code-file/asset inputs. Document-only snapshots never install this capability.
pub trait CodeSource: std::fmt::Debug + Send + Sync {
    fn file(&self, path: &Path) -> Result<Vec<u8>, ReferenceError>;
}
impl PartialEq for CodeIndex {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.files == other.files
            && self.assets == other.assets
            && self.inputs.is_some() == other.inputs.is_some()
    }
}
impl Eq for CodeIndex {}
pub fn split_reference(reference: &str) -> Result<(PathBuf, Option<&str>), ReferenceError> {
    let rest = reference
        .strip_prefix("code:")
        .ok_or(ReferenceError::Unsupported)?;
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
        return Err(ReferenceError::Unsupported);
    }
    Ok((PathBuf::from(path), selector))
}

impl CodeIndex {
    pub fn empty(project: &Project) -> Self {
        Self {
            root: project.root().to_owned(),
            files: BTreeMap::new(),
            assets: vec![PathBuf::from(crate::PROJECT_FILE)],
            inputs: None,
        }
    }

    pub fn assets(&self) -> impl Iterator<Item = &PathBuf> {
        self.assets.iter()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn files(&self) -> impl Iterator<Item = &CodeFile> {
        self.files.values()
    }

    pub fn file_only_bytes(&self, path: &Path) -> Option<Vec<u8>> {
        self.inputs.as_ref()?.file(path).ok()
    }

    // @mara implements DES-CODE-READ
    pub fn resolve(&self, reference: &str) -> Result<CodeResolved, ReferenceError> {
        let (path, selector) = split_reference(reference)?;
        let bytes = self
            .inputs
            .as_ref()
            .ok_or(ReferenceError::Unavailable)?
            .file(&path)?;
        let Some(selector) = selector else {
            let content = String::from_utf8(bytes).ok();
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
        let file = self.files.get(&path).ok_or(ReferenceError::Unsupported)?;
        let mut matches = file
            .symbols
            .iter()
            .filter(|symbol| symbol.selector == selector);
        let symbol = matches.next().ok_or(ReferenceError::MissingSymbol)?;
        let mut start = symbol.content.span().start_byte();
        let mut end = symbol.content.span().end_byte();
        for declaration in matches {
            if declaration.identity != symbol.identity {
                return Err(ReferenceError::Ambiguous);
            }
            start = start.min(declaration.content.span().start_byte());
            end = end.max(declaration.content.span().end_byte());
        }
        Ok(CodeResolved {
            reference: reference.to_owned(),
            source: symbol.source.clone(),
            content: Some(file.source[start..end].to_owned()),
            symbol: Some(symbol.selector.clone()),
        })
    }
}

pub struct CodeResolved {
    pub reference: String,
    pub source: SourceLocation,
    pub content: Option<String>,
    pub symbol: Option<String>,
}

impl CodeResolved {
    pub fn summary(&self) -> crate::DiscoveryNodeSummary {
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
