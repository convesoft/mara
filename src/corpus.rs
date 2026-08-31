use std::{
    fs, io,
    path::{Path, PathBuf},
};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::{Error, PROJECT_FILE, Project, Schema, is_item_id, is_snake_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpus {
    documents: Vec<Document>,
}

impl Corpus {
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.documents.iter().flat_map(|document| document.items())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    path: PathBuf,
    source: String,
    items: Vec<Item>,
}

impl Document {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    flavour: String,
    id: String,
    title: String,
    metadata: Vec<MetadataEntry>,
    body: String,
    relations: Vec<Relation>,
    mentions: Vec<Mention>,
    source: SourceLocation,
    body_source: SourceLocation,
}

impl Item {
    pub fn flavour(&self) -> &str {
        &self.flavour
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn metadata(&self) -> &[MetadataEntry] {
        &self.metadata
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }

    pub fn mentions(&self) -> &[Mention] {
        &self.mentions
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }

    pub fn body_source(&self) -> &SourceLocation {
        &self.body_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataEntry {
    key: String,
    value: String,
    source: SourceLocation,
}

impl MetadataEntry {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    name: String,
    target: String,
    source: SourceLocation,
}

impl Relation {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    target: String,
    source: SourceLocation,
}

impl Mention {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    path: PathBuf,
    span: SourceSpan,
}

impl SourceLocation {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
}

impl SourceSpan {
    pub fn start_byte(self) -> usize {
        self.start_byte
    }

    pub fn end_byte(self) -> usize {
        self.end_byte
    }

    pub fn start_line(self) -> usize {
        self.start_line
    }

    pub fn end_line(self) -> usize {
        self.end_line
    }
}

pub fn load_corpus(project: &Project, schema: &Schema) -> Result<Corpus, Error> {
    let matcher = content_matcher(project)?;
    let mut paths = Vec::new();
    discover(project.root(), project.root(), &matcher, &mut paths)?;
    paths.sort();

    let mut documents = Vec::with_capacity(paths.len());
    for relative_path in paths {
        let absolute_path = project.root().join(&relative_path);
        let source = fs::read_to_string(&absolute_path).map_err(|source| Error::Io {
            action: "read Mara document",
            path: absolute_path,
            source,
        })?;
        documents.push(parse_document(relative_path, source, schema)?);
    }
    Ok(Corpus { documents })
}

fn content_matcher(project: &Project) -> Result<GlobSet, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in project.content_patterns() {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| Error::InvalidProject {
                path: project.root().join(PROJECT_FILE),
                message: format!("invalid content.include pattern '{pattern}': {error}"),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|error| Error::InvalidProject {
        path: project.root().join(PROJECT_FILE),
        message: format!("could not compile content.include patterns: {error}"),
    })
}

fn discover(
    root: &Path,
    directory: &Path,
    matcher: &GlobSet,
    paths: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let entries = fs::read_dir(directory).map_err(|source| Error::Io {
        action: "read content directory",
        path: directory.to_path_buf(),
        source,
    })?;
    let mut entries = entries
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|source| Error::Io {
            action: "read content directory entry",
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry.file_type().map_err(|source| Error::Io {
            action: "inspect content path",
            path: entry.path(),
            source,
        })?;
        if file_type.is_dir() {
            discover(root, &entry.path(), matcher, paths)?;
        } else if file_type.is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("discovered content remains below the project root")
                .to_path_buf();
            if is_mara_document(&relative) && matcher.is_match(&relative) {
                paths.push(relative);
            }
        }
    }
    Ok(())
}

fn is_mara_document(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".mara.md"))
}

fn parse_document(path: PathBuf, source: String, schema: &Schema) -> Result<Document, Error> {
    let lines = source_lines(&source);
    let line_starts = lines.iter().map(|line| line.start).collect::<Vec<_>>();
    let mut items = Vec::new();
    let mut line_index = 0;
    let mut fence = None;

    while line_index < lines.len() {
        let line = lines[line_index];
        if update_fence(line.text, &mut fence) || fence.is_some() {
            line_index += 1;
            continue;
        }
        if let Some((flavour, id)) = opener(line.text) {
            let (item, next_line) = parse_item(
                &path,
                &source,
                &lines,
                &line_starts,
                line_index,
                flavour,
                id,
                schema,
            )?;
            items.push(item);
            line_index = next_line;
        } else if looks_like_item_opener(line.text) {
            return Err(invalid(
                &path,
                line.number,
                "item opener must be ':::mara <flavour> <id>' with no other tokens",
            ));
        } else {
            line_index += 1;
        }
    }

    Ok(Document {
        path,
        source,
        items,
    })
}

#[allow(clippy::too_many_arguments)]
fn parse_item(
    path: &Path,
    source: &str,
    lines: &[SourceLine<'_>],
    line_starts: &[usize],
    opener_index: usize,
    flavour: &str,
    id: &str,
    schema: &Schema,
) -> Result<(Item, usize), Error> {
    if !is_snake_name(flavour) {
        return Err(invalid(
            path,
            lines[opener_index].number,
            format!("invalid flavour '{flavour}'"),
        ));
    }
    if !is_item_id(id) {
        return Err(invalid(
            path,
            lines[opener_index].number,
            format!("invalid item ID '{id}'"),
        ));
    }

    let mut metadata = Vec::new();
    let mut line_index = opener_index + 1;
    while line_index < lines.len() && !lines[line_index].text.is_empty() {
        let line = lines[line_index];
        let Some(rest) = line.text.strip_prefix(':') else {
            return Err(invalid(
                path,
                line.number,
                "expected metadata or a blank line before the item body",
            ));
        };
        let Some((key, value)) = rest.split_once(':') else {
            return Err(invalid(path, line.number, "invalid metadata entry"));
        };
        if !is_snake_name(key) {
            return Err(invalid(
                path,
                line.number,
                format!("invalid metadata key '{key}'"),
            ));
        }
        metadata.push(MetadataEntry {
            key: key.to_owned(),
            value: value.trim().to_owned(),
            source: location(path, line_starts, line.start, line.end),
        });
        line_index += 1;
    }
    if line_index == lines.len() {
        return Err(invalid(
            path,
            lines[opener_index].number,
            "item is missing its body boundary and closing delimiter",
        ));
    }

    let title_entries = metadata
        .iter()
        .filter(|entry| entry.key == "title")
        .collect::<Vec<_>>();
    if title_entries.len() != 1 || title_entries[0].value.is_empty() {
        return Err(invalid(
            path,
            lines[opener_index].number,
            "item must have exactly one non-empty title entry",
        ));
    }
    let title = title_entries[0].value.clone();
    let body_start = lines[line_index].full_end;
    line_index += 1;

    let mut fence = None;
    let closing_index = loop {
        let Some(line) = lines.get(line_index).copied() else {
            return Err(invalid(
                path,
                lines[opener_index].number,
                "item is missing its closing delimiter",
            ));
        };
        if update_fence(line.text, &mut fence) || fence.is_some() {
            line_index += 1;
            continue;
        }
        if line.text == ":::" {
            break line_index;
        }
        if looks_like_item_opener(line.text) {
            let message = if opener(line.text).is_some() {
                "items cannot nest"
            } else {
                "invalid nested item opener"
            };
            return Err(invalid(path, line.number, message));
        }
        line_index += 1;
    };

    let body_end = lines[closing_index].start;
    let relations = metadata
        .iter()
        .filter(|entry| schema.relations().contains_key(&entry.key))
        .map(|entry| Relation {
            name: entry.key.clone(),
            target: entry.value.clone(),
            source: entry.source.clone(),
        })
        .collect();
    let mentions = parse_mentions(path, source, line_starts, body_start, body_end);
    let item = Item {
        flavour: flavour.to_owned(),
        id: id.to_owned(),
        title,
        metadata,
        body: source[body_start..body_end].to_owned(),
        relations,
        mentions,
        source: location(
            path,
            line_starts,
            lines[opener_index].start,
            lines[closing_index].full_end,
        ),
        body_source: location(path, line_starts, body_start, body_end),
    };
    Ok((item, closing_index + 1))
}

fn parse_mentions(
    path: &Path,
    source: &str,
    line_starts: &[usize],
    start: usize,
    end: usize,
) -> Vec<Mention> {
    let mut mentions = Vec::new();
    let mut fence = None;
    let mut inline_code = None;
    for line in source_lines(&source[start..end]) {
        if update_fence(line.text, &mut fence) || fence.is_some() {
            continue;
        }
        let bytes = line.text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'`' {
                let run = byte_run(bytes, index, b'`');
                match inline_code {
                    Some(delimiter) if delimiter == run => inline_code = None,
                    None => inline_code = Some(run),
                    _ => {}
                }
                index += run;
                continue;
            }
            if inline_code.is_none()
                && bytes[index..].starts_with(b"[[")
                && !escaped(bytes, index)
                && let Some(relative_end) = line.text[index + 2..].find("]]")
            {
                let target_end = index + 2 + relative_end;
                let target = &line.text[index + 2..target_end];
                if is_item_id(target) {
                    let mention_start = start + line.start + index;
                    let mention_end = start + line.start + target_end + 2;
                    mentions.push(Mention {
                        target: target.to_owned(),
                        source: location(path, line_starts, mention_start, mention_end),
                    });
                    index = target_end + 2;
                    continue;
                }
            }
            index += 1;
        }
    }
    mentions
}

fn escaped(bytes: &[u8], index: usize) -> bool {
    let mut slashes = 0;
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        slashes += 1;
        cursor -= 1;
    }
    slashes % 2 == 1
}

fn opener(line: &str) -> Option<(&str, &str)> {
    let declaration = line.strip_prefix(":::mara ")?;
    let (flavour, id) = declaration.split_once(' ')?;
    (!flavour.is_empty() && !id.is_empty() && !id.bytes().any(|byte| byte.is_ascii_whitespace()))
        .then_some((flavour, id))
}

fn looks_like_item_opener(line: &str) -> bool {
    line == ":::mara" || line.starts_with(":::mara ")
}

#[derive(Clone, Copy)]
struct SourceLine<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    full_end: usize,
    number: usize,
}

fn source_lines(source: &str) -> Vec<SourceLine<'_>> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut number = 1;
    while start < bytes.len() {
        let newline = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset);
        let full_end = newline.map_or(bytes.len(), |offset| offset + 1);
        let mut end = newline.unwrap_or(bytes.len());
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        lines.push(SourceLine {
            text: &source[start..end],
            start,
            end,
            full_end,
            number,
        });
        start = full_end;
        number += 1;
    }
    lines
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    length: usize,
}

fn update_fence(line: &str, fence: &mut Option<Fence>) -> bool {
    let bytes = line.as_bytes();
    let indentation = bytes.iter().take_while(|byte| **byte == b' ').count();
    if indentation > 3 || indentation == bytes.len() {
        return false;
    }
    let marker = bytes[indentation];
    if marker != b'`' && marker != b'~' {
        return false;
    }
    let length = byte_run(bytes, indentation, marker);
    if length < 3 {
        return false;
    }

    match *fence {
        Some(open)
            if open.marker == marker
                && length >= open.length
                && line[indentation + length..].trim().is_empty() =>
        {
            *fence = None;
            true
        }
        None => {
            *fence = Some(Fence { marker, length });
            true
        }
        _ => false,
    }
}

fn byte_run(bytes: &[u8], start: usize, byte: u8) -> usize {
    bytes[start..]
        .iter()
        .take_while(|candidate| **candidate == byte)
        .count()
}

fn location(path: &Path, line_starts: &[usize], start: usize, end: usize) -> SourceLocation {
    SourceLocation {
        path: path.to_path_buf(),
        span: SourceSpan {
            start_byte: start,
            end_byte: end,
            start_line: line_number(line_starts, start),
            end_line: line_number(line_starts, end.saturating_sub(1).max(start)),
        },
    }
}

fn line_number(line_starts: &[usize], byte: usize) -> usize {
    if line_starts.is_empty() {
        return 1;
    }
    line_starts.partition_point(|start| *start <= byte).max(1)
}

fn invalid(path: &Path, line: usize, message: impl Into<String>) -> Error {
    Error::InvalidDocument {
        path: path.to_path_buf(),
        line,
        message: message.into(),
    }
}
