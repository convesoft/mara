//! Seeded corpus generator (DES-PERFORMANCE-HARNESS in `docs/performance.mara.md`).
//!
//! A corpus of size `n` holds `8 * n` clusters. Each cluster is 25 items in three documents and
//! 12 Rust files carrying markers, built from one template with seeded choices, so chains of
//! `Decomposes` and `Derives from` have the same depth and fan-out at every size. Cluster `c`
//! depends only on the seed and `c`, and links only to itself and to cluster `c - 1`.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use tempfile::TempDir;

/// Clusters per unit of size: 1× is 200 items.
const CLUSTERS_PER_SIZE: usize = 8;
/// Items per cluster.
const CLUSTER_ITEMS: usize = 25;
/// Clusters per directory under `docs/`, `src/` and `tests/`.
const CLUSTERS_PER_AREA: usize = 8;
/// Rust files per cluster under `src/` and under `tests/`.
const SOURCE_FILES: usize = 9;
const TEST_FILES: usize = 3;
/// Timestamp part of the first MID, in milliseconds.
const MID_TIME: u64 = 1_794_000_000_000;

/// The configuration example of DES-PROJECT-CONFIGURATION. It declares no code language and no
/// program, so code files are not inputs yet.
const CONFIG: &str = r#"format_version = 1
schema = ".mara/schema.toml"

[documents]
format_version = 1
include = ["docs/**/*.mara.md"]
"#;
/// The engineering vocabulary (DES-ENGINEERING-VOCABULARY): the `engineering` template's schema.
const SCHEMA: &str = include_str!("../../templates/engineering/schema.toml");
const GITIGNORE: &str = "/target/\n/docs/generated/\n";

static WORDS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    "access account action address agent alert anchor archive audit backup balance batch billing
    binary block branch buffer build bundle cache catalog channel check client cluster column
    command commit config contract counter credential cursor daemon dashboard data deadline
    delivery deploy device digest document domain draft edge editor engine entry error event
    export feature field filter folder format gateway grammar graph handler hash header history
    host identity import index input invoice issue item journal key label latency layer ledger
    limit link listener lock log marker message metric migration mirror module monitor network
    node notice offset order outcome owner package page parser partition patch path payload
    payment peer permission pipeline policy pool port process profile program project proxy
    query queue quota range reader record region release replica report request resource
    response result retry review revision role route rule runtime sample schema scope search
    secret segment server service session shard signal snapshot socket source stage state
    storage stream subscriber summary table target task template tenant thread ticket timeout
    token topic trace transaction upload usage user value vault version view volume watcher
    window worker workspace writer"
        .split_whitespace()
        .collect()
});
static VERBS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    "accepts blocks caches checks collects compares computes creates deletes derives emits
    exports filters groups hashes indexes keeps limits links lists loads locks marks merges moves
    orders parses records reads rejects renames reports resolves returns scans sends signs sorts
    splits stores streams tracks updates validates writes"
        .split_whitespace()
        .collect()
});
const LIKELIHOODS: &[&str] = &["Rare", "Unlikely", "Possible", "Likely", "Almost-certain"];
const CONSEQUENCES: &[&str] = &["Minimal", "Minor", "Moderate", "Major", "Severe"];
const METHODS: &[&str] = &["Test", "Analysis", "Inspection", "Demonstration"];

/// A generated project in its own temporary directory, removed on drop.
pub struct Corpus {
    dir: TempDir,
    /// Items in declared documents.
    pub items: usize,
    /// Declared documents.
    pub documents: usize,
    /// Relation links and mentions in declared documents.
    pub edges: usize,
    /// The ID of the first item, the same at every size.
    pub first_id: String,
    /// Edges on the longest chain of `Decomposes` and `Derives from`.
    pub max_chain_depth: usize,
    /// Most `Decomposes` and `Derives from` edges into one item.
    pub max_fan_out: usize,
    /// The ID and the document path of every item, cluster by cluster, each in template order.
    pub ids: Vec<(String, String)>,
    /// The index in `ids` of the first verification of the first cluster that no relation or
    /// mention targets, the same at every size.
    pub untargeted: usize,
}

impl Corpus {
    /// Generates the corpus of `size` × 200 items for `seed` under `$TMPDIR`.
    pub fn generate(size: usize, seed: u64) -> Corpus {
        let dir = tempfile::Builder::new()
            .prefix("mara-corpus-")
            .tempdir()
            .expect("create a temporary directory for the corpus");
        let root = dir.path();
        let clusters = CLUSTERS_PER_SIZE * size;
        let mut items = Vec::with_capacity(clusters * CLUSTER_ITEMS);
        let mut documents = Vec::with_capacity(clusters * 3);
        for cluster in 0..clusters {
            add_cluster(cluster, seed, &mut items, &mut documents);
        }
        let mut written = Vec::new();
        for (index, document) in documents.iter().enumerate() {
            write(
                root,
                &document.path,
                render(index, &documents, &items).as_bytes(),
            );
            written.push(document.path.clone());
        }
        for cluster in 0..clusters {
            for (path, contents) in code_files(cluster, seed, &items) {
                write(root, &path, contents.as_bytes());
                written.push(path);
            }
        }
        let mut rng = Rng::new(seed, u64::MAX);
        write_project_files(root, size, seed, &mut rng, items.len());
        write_git_directory(root, &mut rng, &written);
        let (max_chain_depth, max_fan_out) = chain_shape(&items);
        let edges = items
            .iter()
            .map(|item| item.relations.len() + item.mentions.len())
            .sum();
        let targeted: std::collections::HashSet<usize> = items
            .iter()
            .flat_map(|item| {
                item.relations
                    .iter()
                    .map(|&(_, target)| target)
                    .chain(item.mentions.iter().copied())
            })
            .collect();
        let untargeted = (20..24)
            .find(|index| !targeted.contains(index))
            .expect("a verification of the first cluster that nothing targets");
        Corpus {
            dir,
            items: items.len(),
            documents: documents.len(),
            edges,
            first_id: items[0].id.clone(),
            max_chain_depth,
            max_fan_out,
            ids: items
                .iter()
                .map(|item| (item.id.clone(), documents[item.document].path.clone()))
                .collect(),
            untargeted,
        }
    }

    /// The project root.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Goal,
    Need,
    Scenario,
    Term,
    Requirement,
    Risk,
    Design,
    Decision,
    Evidence,
    Verification,
    Validation,
}

impl Kind {
    /// The `Type` label and the ID prefix.
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Kind::Goal => ("Goal", "GOAL"),
            Kind::Need => ("Need", "NEED"),
            Kind::Scenario => ("Scenario", "SCN"),
            Kind::Term => ("Term", "TERM"),
            Kind::Requirement => ("Requirement", "REQ"),
            Kind::Risk => ("Risk", "RISK"),
            Kind::Design => ("Design", "DES"),
            Kind::Decision => ("Decision", "ADR"),
            Kind::Evidence => ("Evidence", "EVD"),
            Kind::Verification => ("Verification", "VER"),
            Kind::Validation => ("Validation", "VAL"),
        }
    }
}

/// A relation label and its target item.
type Relation = (&'static str, usize);
/// Kind, document within the cluster, status and relations of one templated item.
type Spec = (Kind, usize, &'static str, Vec<Relation>);

struct Item {
    kind: Kind,
    mid: String,
    id: String,
    title: String,
    status: &'static str,
    /// Index into the corpus documents.
    document: usize,
    fields: Vec<(&'static str, String)>,
    relations: Vec<Relation>,
    body: String,
    /// Items the body mentions.
    mentions: Vec<usize>,
}

struct Document {
    path: String,
    title: String,
    intro: String,
    /// Section heading and the items under it.
    sections: Vec<(&'static str, Vec<usize>)>,
    open_questions: Vec<String>,
}

/// Adds the 25 items and three documents of one cluster.
fn add_cluster(cluster: usize, seed: u64, items: &mut Vec<Item>, documents: &mut Vec<Document>) {
    use Kind::*;
    const ACCEPTED: &str = "Accepted";

    let mut rng = Rng::new(seed, cluster as u64);
    let base = items.len();
    let previous = cluster.checked_sub(1).map(|c| c * CLUSTER_ITEMS);
    let first_document = documents.len();
    let at = |local: usize| base + local;

    let topic = [rng.word(), rng.word()];
    let directory = format!("docs/a{:03}", cluster / CLUSTERS_PER_AREA);
    let slug = format!("c{cluster:04}-{}-{}", topic[0], topic[1]);
    let title = capitalize(&format!("{} {}", topic[0], topic[1]));
    for suffix in ["", "-design", "-verification"] {
        documents.push(Document {
            path: format!("{directory}/{slug}{suffix}.mara.md"),
            title: format!("{title}{}", suffix.replace('-', " ")),
            intro: paragraph(&mut rng, 1, 2),
            sections: Vec::new(),
            open_questions: Vec::new(),
        });
    }

    // Seeded choices, then the template by local index.
    let mut status = |percent| {
        if rng.percent(percent) {
            "Draft"
        } else {
            ACCEPTED
        }
    };
    let (term, decision, evidence) = (status(30), status(30), status(20));
    let need_parent = rng.maybe(("Decomposes", at(1)));
    let second_satisfies = rng.maybe(("Satisfies", at(9)));
    let accepts_risk = rng.maybe(("Accepts", at(12)));
    let requirement_parent = rng.maybe(at(7)).unwrap_or(at(5));
    let requirement_origin = rng.maybe(at(9)).unwrap_or(at(6));
    let threatened = rng.maybe(at(1)).unwrap_or(at(5));
    let goal = previous
        .and_then(|previous| rng.maybe(previous))
        .unwrap_or(at(0));
    let supports_previous = previous.map(|previous| ("Supports", previous + 13));
    let with = |first: Relation, second: Option<Relation>| -> Vec<Relation> {
        [Some(first), second].into_iter().flatten().collect()
    };
    #[rustfmt::skip]
    let specs: Vec<Spec> = vec![
        (Goal, 0, ACCEPTED, vec![]),
        (Need, 0, ACCEPTED, vec![("Contributes to", at(0))]),
        (Need, 0, ACCEPTED, with(("Contributes to", at(0)), need_parent)),
        (Scenario, 0, ACCEPTED, vec![]),
        (Term, 0, term, vec![]),
        (Requirement, 0, ACCEPTED, vec![("Satisfies", at(1))]),
        (Requirement, 0, ACCEPTED, vec![("Satisfies", at(2)), ("Supersedes", at(11))]),
        (Requirement, 0, ACCEPTED, vec![("Decomposes", at(5))]),
        (Requirement, 0, ACCEPTED, vec![("Decomposes", requirement_parent)]),
        (Requirement, 0, ACCEPTED, vec![("Derives from", at(6))]),
        (Requirement, 0, ACCEPTED, vec![("Derives from", requirement_origin)]),
        (Requirement, 0, "Retired", vec![("Satisfies", at(2))]),
        (Risk, 0, ACCEPTED, vec![("Threatens", threatened)]),
        (Design, 1, ACCEPTED, vec![("Satisfies", at(5))]),
        (Design, 1, ACCEPTED, with(("Satisfies", at(6)), second_satisfies)),
        (Design, 1, ACCEPTED, vec![("Decomposes", at(13))]),
        (Design, 1, ACCEPTED, vec![("Contributes to", goal), ("Mitigates", at(12))]),
        (Decision, 1, ACCEPTED, vec![("Justifies", at(13))]),
        (Decision, 1, decision, with(("Justifies", at(14)), accepts_risk)),
        (Evidence, 1, evidence, with(("Supports", at(17)), supports_previous)),
        (Verification, 2, ACCEPTED, vec![("Verifies", at(5)), ("Verifies", at(7))]),
        (Verification, 2, ACCEPTED, vec![("Verifies", at(8))]),
        (Verification, 2, ACCEPTED, vec![("Verifies", at(6)), ("Verifies", at(9))]),
        (Verification, 2, ACCEPTED, vec![("Verifies", at(10))]),
        (Validation, 2, ACCEPTED, vec![("Validates", at(1)), ("Exercises", at(3))]),
    ];
    assert_eq!(specs.len(), CLUSTER_ITEMS);

    for (local, (kind, document, status, relations)) in specs.into_iter().enumerate() {
        let index = base + local;
        // Term titles and aliases differ in word count and carry the index, so no two are equal.
        let words = if kind == Term { 2 } else { rng.between(2, 5) };
        let words: Vec<&str> = (0..words).map(|_| rng.word()).collect();
        let prefix = kind.names().1;
        let id = format!("{prefix}-{}-{}-{index}", words[0], words[1]).to_uppercase();
        let mut title = capitalize(&words.join(" "));
        let fields = match kind {
            Term => {
                let _ = write!(title, " {index}");
                let alias = format!("{} {} {} {index}", rng.word(), rng.word(), rng.word());
                vec![("Aliases", alias)]
            }
            Risk => vec![
                ("Likelihood", rng.pick(LIKELIHOODS).to_string()),
                ("Consequence", rng.pick(CONSEQUENCES).to_string()),
            ],
            Verification | Validation => vec![("Methods", rng.pick(METHODS).to_string())],
            _ => vec![],
        };
        let mut mentions = Vec::new();
        if rng.percent(30) {
            let low = previous.unwrap_or(base);
            let target = low + rng.below(base + CLUSTER_ITEMS - low);
            if target != index {
                mentions.push(target);
            }
        }
        let body = body(kind, &mut rng);
        items.push(Item {
            kind,
            mid: mid(MID_TIME + index as u64, &mut rng),
            id,
            title,
            status,
            document: first_document + document,
            fields,
            relations,
            body,
            mentions,
        });
    }

    let sections: [(usize, &'static str, &[usize]); 8] = [
        (0, "Goals and needs", &[0, 1, 2, 3, 4]),
        (0, "Requirements", &[5, 6, 7, 8, 9, 10, 11]),
        (0, "Risks", &[12]),
        (1, "Design", &[13, 14, 15, 16]),
        (1, "Decisions", &[17, 18]),
        (1, "Evidence", &[19]),
        (2, "Verification", &[20, 21, 22, 23]),
        (2, "Validation", &[24]),
    ];
    for (document, heading, members) in sections {
        let members = members.iter().map(|&local| at(local)).collect();
        documents[first_document + document]
            .sections
            .push((heading, members));
    }
    if rng.percent(30) {
        let questions = rng.between(1, 3);
        documents[first_document].open_questions = (0..questions)
            .map(|_| sentence(&mut rng).replace('.', "?"))
            .collect();
    }
}

/// The body of an item of `kind`: Markdown of a few hundred bytes.
fn body(kind: Kind, rng: &mut Rng) -> String {
    let mut body = String::new();
    match kind {
        Kind::Requirement | Kind::Term | Kind::Risk | Kind::Validation => {
            body.push_str(&paragraph(rng, 1, 3));
        }
        Kind::Goal | Kind::Need => body.push_str(&paragraph(rng, 2, 4)),
        Kind::Scenario => {
            body.push_str(&paragraph(rng, 1, 2));
            body.push('\n');
            for step in 1..=rng.between(3, 5) {
                let _ = write!(body, "\n{step}. {}", sentence(rng));
            }
        }
        Kind::Design => {
            body.push_str(&paragraph(rng, 1, 3));
            body.push('\n');
            for _ in 0..rng.between(2, 6) {
                let _ = write!(body, "\n- {}", sentence(rng));
            }
            if rng.percent(25) {
                body.push_str("\n\n");
                body.push_str(&table(rng, ["Part", "Holds"]));
            }
            if rng.percent(10) {
                let (section, key, value) = (rng.word(), rng.word(), rng.below(1000));
                let _ = write!(body, "\n\n```toml\n[{section}]\n{key}_limit = {value}\n```");
            }
        }
        Kind::Decision => {
            let _ = write!(body, "- Chosen: {}", sentence(rng));
            for _ in 0..rng.between(1, 2) {
                let _ = write!(body, "\n- Rejected: {}", sentence(rng));
            }
        }
        Kind::Evidence => {
            body.push_str(&paragraph(rng, 1, 2));
            body.push_str("\n\n");
            body.push_str(&table(rng, ["Finding", "Measurement"]));
        }
        Kind::Verification => {
            body.push_str(&paragraph(rng, 1, 3));
            if rng.percent(50) {
                body.push_str("\n\n");
                body.push_str(&table(rng, ["Case", "Expectation"]));
            }
        }
    }
    body
}

fn table(rng: &mut Rng, header: [&str; 2]) -> String {
    let mut table = format!("| {} | {} |\n| --- | --- |", header[0], header[1]);
    for _ in 0..rng.between(2, 5) {
        let _ = write!(table, "\n| {} | {} |", phrase(rng, 2, 4), phrase(rng, 2, 6));
    }
    table
}

fn paragraph(rng: &mut Rng, low: usize, high: usize) -> String {
    let sentences: Vec<String> = (0..rng.between(low, high)).map(|_| sentence(rng)).collect();
    sentences.join(" ")
}

/// A sentence of 7–17 words, sometimes with a code span whose name grows the vocabulary.
fn sentence(rng: &mut Rng) -> String {
    let mut words = vec![capitalize(rng.word()), rng.pick(&VERBS).to_string()];
    for _ in 0..rng.between(5, 15) {
        words.push(rng.word().to_string());
    }
    if rng.percent(15) {
        let at = rng.between(2, words.len() - 1);
        let name = format!("`{}_{}_{}`", rng.word(), rng.word(), rng.below(10_000));
        words.insert(at, name);
    }
    words.join(" ") + "."
}

fn phrase(rng: &mut Rng, low: usize, high: usize) -> String {
    let words: Vec<&str> = (0..rng.between(low, high)).map(|_| rng.word()).collect();
    capitalize(&words.join(" "))
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Renders document `index` in the item format of `docs/format.mara.md`.
fn render(index: usize, documents: &[Document], items: &[Item]) -> String {
    let document = &documents[index];
    let mut out = format!("# {}\n\n{}\n", document.title, document.intro);
    for (number, (heading, members)) in document.sections.iter().enumerate() {
        if number > 0 {
            out.push_str("\n---\n");
        }
        let _ = write!(out, "\n## {heading}\n");
        for &member in members {
            out.push_str("\n---\n\n");
            render_item(&mut out, index, documents, &items[member], items);
        }
    }
    if !document.open_questions.is_empty() {
        out.push_str("\n---\n\n## Open questions\n\n");
        for question in &document.open_questions {
            let _ = writeln!(out, "- {question}");
        }
    }
    out
}

fn render_item(out: &mut String, from: usize, documents: &[Document], item: &Item, items: &[Item]) {
    let link = |target: usize| {
        let target = &items[target];
        format!(
            "{}#{}",
            link_path(documents, from, target.document),
            target.mid
        )
    };
    let _ = writeln!(out, "<a id=\"{}\"></a>\n\n### {}\n", item.mid, item.title);
    let _ = writeln!(out, "| Type | {} |\n| --- | --- |", item.kind.names().0);
    let _ = writeln!(out, "| ID | {} |\n| Status | {} |", item.id, item.status);
    for (key, value) in &item.fields {
        let _ = writeln!(out, "| {key} | {value} |");
    }
    for &(key, target) in &item.relations {
        let _ = writeln!(out, "| {key} | [{}]({}) |", items[target].id, link(target));
    }
    let _ = writeln!(out, "\n{}", item.body);
    for &mention in &item.mentions {
        let text = items[mention].title.to_lowercase();
        let _ = writeln!(out, "\nSee [{text}]({}).", link(mention));
    }
}

/// The path of document `to` relative to document `from`; empty within one file.
fn link_path(documents: &[Document], from: usize, to: usize) -> String {
    if from == to {
        return String::new();
    }
    let split = |document: &Document| {
        let (directory, file) = document
            .path
            .rsplit_once('/')
            .expect("documents are in a directory");
        (directory.to_string(), file.to_string())
    };
    let ((from_directory, _), (to_directory, file)) =
        (split(&documents[from]), split(&documents[to]));
    if from_directory == to_directory {
        return file;
    }
    let area = to_directory
        .rsplit_once('/')
        .expect("documents are in an area")
        .1;
    format!("../{area}/{file}")
}

/// The Rust files of one cluster: functions and tests carrying `@mara implements` markers.
fn code_files(cluster: usize, seed: u64, items: &[Item]) -> Vec<(String, String)> {
    let mut rng = Rng::new(seed, (1 << 32) | cluster as u64);
    let base = cluster * CLUSTER_ITEMS;
    let area = cluster / CLUSTERS_PER_AREA;
    let implemented = ids(items, base, &[5, 6, 7, 8, 9, 10, 13, 14, 15, 16]);
    let verifications = ids(items, base, &[20, 21, 22, 23]);
    let mut files = Vec::with_capacity(SOURCE_FILES + TEST_FILES);
    for file in 0..SOURCE_FILES {
        let mut code = format!("//! {}\n", sentence(&mut rng));
        for function in 0..rng.between(3, 8) {
            let _ = write!(code, "\n/// {}\n", sentence(&mut rng));
            if rng.percent(50) {
                let _ = writeln!(code, "// @mara implements {}", rng.pick(&implemented));
            }
            let name = format!("{}_{}_{function}", rng.word(), rng.word());
            let _ = writeln!(
                code,
                "pub fn {name}(input: u64) -> u64 {{\n    let mut value = input;"
            );
            for _ in 0..rng.between(1, 10) {
                let (factor, term) = (rng.between(2, 97), rng.below(1000));
                let _ = writeln!(
                    code,
                    "    value = value.wrapping_mul({factor}).wrapping_add({term});"
                );
            }
            code.push_str("    value\n}\n");
        }
        let name = format!("{}_{}_{file}", rng.word(), rng.word());
        files.push((format!("src/a{area:03}/c{cluster:04}/{name}.rs"), code));
    }
    for file in 0..TEST_FILES {
        let mut code = String::new();
        for test in 0..rng.between(2, 5) {
            let marker = rng.pick(&verifications);
            let name = format!("{}_{}_{test}", rng.word(), rng.word());
            let (value, factor) = (rng.below(1000), rng.between(2, 97));
            let _ = write!(
                code,
                "{}// @mara implements {marker}\n#[test]\nfn {name}() {{\n    \
                 let value = {value}u64.wrapping_mul({factor});\n    \
                 assert_eq!(value / {factor}, {value});\n}}\n",
                if test > 0 { "\n" } else { "" }
            );
        }
        let name = format!("c{cluster:04}_{}_{file}", rng.word());
        files.push((format!("tests/a{area:03}/{name}.rs"), code));
    }
    files
}

/// The IDs of the items at `locals` in the cluster that starts at `base`.
fn ids<'a>(items: &'a [Item], base: usize, locals: &[usize]) -> Vec<&'a str> {
    locals
        .iter()
        .map(|&local| items[base + local].id.as_str())
        .collect()
}

/// Configuration, schema, `.gitignore`, and files that are not inputs: a README, a document
/// outside the include patterns, a document that `.gitignore` excludes, and build output.
fn write_project_files(root: &Path, size: usize, seed: u64, rng: &mut Rng, items: usize) {
    write(root, ".mara/config.toml", CONFIG.as_bytes());
    write(root, ".mara/schema.toml", SCHEMA.as_bytes());
    write(root, ".gitignore", GITIGNORE.as_bytes());
    let readme = format!("# Generated corpus\n\nSize {size}×, seed {seed}.\n");
    write(root, "README.md", readme.as_bytes());
    let undeclared = ["notes/draft.mara.md", "docs/generated/index.mara.md"];
    for (number, path) in undeclared.into_iter().enumerate() {
        let index = items + number;
        let (mid, title) = (mid(MID_TIME + index as u64, rng), phrase(rng, 2, 4));
        let id = format!("REQ-{}-{index}", rng.word().to_uppercase());
        let document = format!(
            "# Not an input\n\n---\n\n<a id=\"{mid}\"></a>\n\n### {title}\n\n\
             | Type | Requirement |\n| --- | --- |\n| ID | {id} |\n| Status | Draft |\n\n{}\n",
            sentence(rng)
        );
        write(root, path, document.as_bytes());
    }
    write(root, "target/debug/app", &bytes(rng, 4096));
    write(
        root,
        "target/debug/app.d",
        b"target/debug/app: src/a000/c0000\n",
    );
}

/// A `.git` directory with one loose object per generated file, never an input.
fn write_git_directory(root: &Path, rng: &mut Rng, files: &[String]) {
    write(root, ".git/HEAD", b"ref: refs/heads/main\n");
    write(
        root,
        ".git/config",
        b"[core]\n\trepositoryformatversion = 0\n",
    );
    write(root, ".git/description", b"Generated corpus.\n");
    write(
        root,
        ".git/refs/heads/main",
        (hex(rng, 20) + "\n").as_bytes(),
    );
    write(root, ".git/index", &bytes(rng, 64 * files.len()));
    for _ in files {
        let name = hex(rng, 20);
        let length = rng.between(64, 512);
        let path = format!(".git/objects/{}/{}", &name[..2], &name[2..]);
        write(root, &path, &bytes(rng, length));
    }
}

fn write(root: &Path, path: &str, contents: &[u8]) {
    let path = root.join(path);
    let parent = path
        .parent()
        .expect("a generated file has a parent directory");
    fs::create_dir_all(parent)
        .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
    fs::write(&path, contents).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

/// The schema of the stress projects: requirements with a single-valued enum field, and an
/// acyclic relation between requirements.
const STRESS_SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n\
    [types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"An obligation.\"\n\n\
    [types.requirement.fields.priority]\nkind = \"enum\"\nvalues = [\"low\", \"high\"]\n\n\
    [relations.refines]\ninverse = \"refined_by\"\ndescription = \"Refines.\"\n\
    targets = { requirement = [\"requirement\"] }\nacyclic = true\n";

/// A shape that one item-and-edge check step does not measure: very many rows, holders or
/// findings where a check could do work per row, per holder or per finding. Each has its own
/// project ([`Stress`]), and the scaling test times `project validate` on it.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// One item repeats its `ID` row.
    RepeatedId,
    /// One item repeats its `Type` row.
    RepeatedType,
    /// One item repeats its `Status` row.
    RepeatedStatus,
    /// One item gives a single-valued enum field another invalid value in each row.
    RepeatedEnum,
    /// One item repeats a relation row to one target.
    RepeatedRelation,
    /// One item has a relation row to each of very many targets.
    ManyTargets,
    /// One item has very many undeclared keys.
    UndeclaredKeys,
    /// Very many items share one ID.
    SharedId,
    /// Very many items share one MID.
    SharedMid,
    /// One line holds very many broken links.
    FindingsOnOneLine,
}

impl Shape {
    pub const ALL: [Shape; 10] = [
        Shape::RepeatedId,
        Shape::RepeatedType,
        Shape::RepeatedStatus,
        Shape::RepeatedEnum,
        Shape::RepeatedRelation,
        Shape::ManyTargets,
        Shape::UndeclaredKeys,
        Shape::SharedId,
        Shape::SharedMid,
        Shape::FindingsOnOneLine,
    ];

    /// The exit status of `project validate` on the shape's project: only `ManyTargets` is
    /// valid.
    pub fn status(self) -> i32 {
        match self {
            Shape::ManyTargets => 0,
            _ => 1,
        }
    }
}

/// The project of one [`Shape`].
pub struct Stress {
    dir: TempDir,
}

impl Stress {
    /// The project of `shape` at `size`: 1000 × size rows or findings of one item, or 500 × size
    /// items, which take longer to parse, in documents of at most 100.
    pub fn generate(shape: Shape, size: usize) -> Stress {
        let dir = tempfile::Builder::new()
            .prefix("mara-stress-")
            .tempdir()
            .expect("create a temporary directory for the stress project");
        let root = dir.path();
        write(root, ".mara/config.toml", CONFIG.as_bytes());
        write(root, ".mara/schema.toml", STRESS_SCHEMA.as_bytes());
        let count = match shape {
            Shape::ManyTargets | Shape::SharedId | Shape::SharedMid => size * Hub::PER_SIZE / 2,
            _ => size * Hub::PER_SIZE,
        };
        let mid = |number: usize| format!("01M70000{number:018}");
        let item = |mid: &str, id: &str, rows: &str, body: &str| {
            format!(
                "\n---\n\n<a id=\"{mid}\"></a>\n\n### Item {id}\n\n| Type | Requirement |\n\
                 | --- | --- |\n| ID | {id} |\n| Status | Draft |\n{rows}\n{body}\n"
            )
        };
        let rows = |row: &dyn Fn(usize) -> String| (0..count).map(row).collect::<String>();
        let subject = |rows: String, body: &str| item(&mid(0), "REQ-A", &rows, body);
        let target = |number: usize| item(&mid(number), &format!("REQ-T{number}"), "", "Target.");
        let items: Vec<String> = match shape {
            Shape::RepeatedId => vec![subject(rows(&|_| "| ID | REQ-A |\n".into()), "A.")],
            Shape::RepeatedType => {
                vec![subject(rows(&|_| "| Type | Requirement |\n".into()), "A.")]
            }
            Shape::RepeatedStatus => vec![subject(rows(&|_| "| Status | Draft |\n".into()), "A.")],
            Shape::RepeatedEnum => {
                vec![subject(
                    rows(&|n| format!("| Priority | Value-{n} |\n")),
                    "A.",
                )]
            }
            Shape::RepeatedRelation => {
                let link = format!("| Refines | [REQ-T1](#{}) |\n", mid(1));
                vec![subject(rows(&|_| link.clone()), "A."), target(1)]
            }
            Shape::ManyTargets => {
                // Target `n` is item `n`, in document `n / PER_DOCUMENT`.
                let link = |n: usize| {
                    let (n, document) = (n + 1, (n + 1) / Hub::PER_DOCUMENT);
                    format!(
                        "| Refines | [REQ-T{n}](p{document:06}.mara.md#{}) |\n",
                        mid(n)
                    )
                };
                let mut items = vec![subject(rows(&link), "A.")];
                items.extend((1..=count).map(target));
                items
            }
            Shape::UndeclaredKeys => vec![subject(rows(&|n| format!("| Key {n} | x |\n")), "A.")],
            Shape::SharedId => (0..count)
                .map(|n| item(&mid(n), "REQ-SAME", "", "One of many."))
                .collect(),
            Shape::SharedMid => (0..count)
                .map(|n| item(&mid(0), &format!("REQ-M{n}"), "", "One of many."))
                .collect(),
            Shape::FindingsOnOneLine => {
                let links = rows(&|n| format!("[gone](#{}) ", mid(n + 1)));
                vec![subject(String::new(), &links)]
            }
        };
        for (index, chunk) in items.chunks(Hub::PER_DOCUMENT).enumerate() {
            let text = format!("# Part {index}\n{}", chunk.concat());
            write(root, &format!("docs/p{index:06}.mara.md"), text.as_bytes());
        }
        Stress { dir }
    }

    /// The project root.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }
}

/// A project in which every item but one links to that one, whose `Type` and `ID` rows follow
/// as many other rows: late metadata and many incoming edges at once. The other rows' keys
/// alternate between the lengths of `Type` and `ID`, so a scan for either compares every other
/// key in full instead of stopping at its length (DES-PERFORMANCE-HARNESS).
pub struct Hub {
    dir: TempDir,
    pub items: usize,
    pub edges: usize,
}

impl Hub {
    /// Sources and rows per unit of size.
    const PER_SIZE: usize = 1_000;
    /// Sources per document.
    const PER_DOCUMENT: usize = 100;

    /// The hub of `size` × 1000 sources under `$TMPDIR`. Its schema declares `refines` acyclic,
    /// so the cycle pass visits every edge too. The project is valid.
    pub fn generate(size: usize) -> Hub {
        let dir = tempfile::Builder::new()
            .prefix("mara-hub-")
            .tempdir()
            .expect("create a temporary directory for the hub");
        let root = dir.path();
        let sources = size * Self::PER_SIZE;
        let mid = |number: usize| format!("01M70000{number:018}");
        write(root, ".mara/config.toml", CONFIG.as_bytes());
        let schema = "format_version = 1\nstatuses = [\"draft\"]\n\n\
            [types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"An obligation.\"\n\n\
            [types.requirement.fields.tags]\nkind = \"text\"\nrepeatable = true\n\n\
            [types.requirement.fields.no]\nkind = \"text\"\nrepeatable = true\n\n\
            [relations.refines]\ninverse = \"refined_by\"\ndescription = \"Refines.\"\n\
            targets = { requirement = [\"requirement\"] }\nacyclic = true\n";
        write(root, ".mara/schema.toml", schema.as_bytes());
        let mut hub = format!("# Hub\n\n---\n\n<a id=\"{}\"></a>\n\n### Hub\n\n", mid(0));
        hub.push_str("| Tags | tag-0 |\n| --- | --- |\n");
        for tag in 1..sources {
            let key = if tag % 2 == 0 { "Tags" } else { "No" };
            let _ = writeln!(hub, "| {key} | tag-{tag} |");
        }
        hub.push_str("| Type | Requirement |\n| ID | REQ-HUB |\n| Status | Draft |\n\nThe hub.\n");
        write(root, "docs/hub.mara.md", hub.as_bytes());
        for first in (1..=sources).step_by(Self::PER_DOCUMENT) {
            let mut text = format!("# Sources from {first}\n");
            for number in first..(first + Self::PER_DOCUMENT).min(sources + 1) {
                let _ = write!(
                    text,
                    "\n---\n\n<a id=\"{}\"></a>\n\n### Source {number}\n\n\
                     | Type | Requirement |\n| --- | --- |\n| ID | REQ-S{number} |\n\
                     | Status | Draft |\n| Refines | [REQ-HUB](hub.mara.md#{}) |\n\nA source.\n",
                    mid(number),
                    mid(0)
                );
            }
            write(root, &format!("docs/s{first:06}.mara.md"), text.as_bytes());
        }
        Hub {
            dir,
            items: sources + 1,
            edges: sources,
        }
    }

    /// The project root.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Moves the hub item's document to another directory by hand, which leaves every source's
    /// link to it stale.
    pub fn move_hub(&self) {
        let docs = self.root().join("docs");
        fs::create_dir_all(docs.join("moved")).unwrap();
        fs::rename(docs.join("hub.mara.md"), docs.join("moved/hub.mara.md")).unwrap();
    }
}

/// Edges on the longest chain of `Decomposes` and `Derives from`, and the most such edges into
/// one item.
fn chain_shape(items: &[Item]) -> (usize, usize) {
    let parents: Vec<Vec<usize>> = items
        .iter()
        .map(|item| {
            let chains = item
                .relations
                .iter()
                .filter(|(key, _)| matches!(*key, "Decomposes" | "Derives from"));
            chains.map(|&(_, target)| target).collect()
        })
        .collect();
    let mut fan_in = vec![0; items.len()];
    for &parent in parents.iter().flatten() {
        fan_in[parent] += 1;
    }
    // Chains point to earlier items only, so one pass in order finds every chain's depth.
    let mut depth = vec![0; items.len()];
    for (index, parents) in parents.iter().enumerate() {
        assert!(
            parents.iter().all(|&parent| parent < index),
            "a chain points forward"
        );
        depth[index] = parents
            .iter()
            .map(|&parent| depth[parent] + 1)
            .max()
            .unwrap_or(0);
    }
    (
        depth.into_iter().max().unwrap_or(0),
        fan_in.into_iter().max().unwrap_or(0),
    )
}

/// A canonical ULID: 48 bits of `time` and 80 random bits, in Crockford base32.
fn mid(time: u64, rng: &mut Rng) -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let random = (u128::from(rng.next()) << 16) | u128::from(rng.next() & 0xFFFF);
    let value = (u128::from(time) << 80) | random;
    let digit = |position: u32| ALPHABET[((value >> (5 * position)) & 31) as usize] as char;
    (0..26).rev().map(digit).collect()
}

fn hex(rng: &mut Rng, length: usize) -> String {
    bytes(rng, length)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn bytes(rng: &mut Rng, length: usize) -> Vec<u8> {
    (0..length).map(|_| rng.next() as u8).collect()
}

/// SplitMix64: a fixed algorithm, so a seed gives the same corpus on every toolchain.
struct Rng(u64);

impl Rng {
    fn new(seed: u64, stream: u64) -> Rng {
        Rng(seed ^ stream.wrapping_mul(0xD1B5_4A32_D192_ED03))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn between(&mut self, low: usize, high: usize) -> usize {
        low + self.below(high - low + 1)
    }

    fn percent(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    /// `value` half of the time.
    fn maybe<T>(&mut self, value: T) -> Option<T> {
        self.percent(50).then_some(value)
    }

    fn pick<'a>(&mut self, choices: &[&'a str]) -> &'a str {
        choices[self.below(choices.len())]
    }

    fn word(&mut self) -> &'static str {
        self.pick(&WORDS)
    }
}

mod tests {
    use std::path::PathBuf;

    use mara_engine::document;
    use mara_engine::work::WorkCounts;

    use super::*;
    use crate::{PULL_REQUEST_SIZES, SEED};

    #[test]
    fn a_seed_gives_the_same_files() {
        let first = files(Corpus::generate(1, SEED).root());
        assert_eq!(first, files(Corpus::generate(1, SEED).root()));
        assert_ne!(first, files(Corpus::generate(1, SEED + 1).root()));
    }

    #[test]
    fn larger_corpora_have_more_clusters_not_deeper_ones() {
        let shapes: Vec<[usize; 4]> = PULL_REQUEST_SIZES
            .iter()
            .map(|&size| {
                let corpus = Corpus::generate(size, SEED);
                let per_size = [corpus.items / size, corpus.documents / size];
                [
                    per_size[0],
                    per_size[1],
                    corpus.max_chain_depth,
                    corpus.max_fan_out,
                ]
            })
            .collect();
        assert_eq!(shapes[0][0], 200, "items at 1×: {shapes:?}");
        assert!(
            shapes[0][2] > 0 && shapes[0][3] > 1,
            "the 1× corpus has chains: {shapes:?}"
        );
        assert!(shapes.iter().all(|shape| *shape == shapes[0]), "{shapes:?}");
    }

    #[test]
    fn the_configuration_and_the_schema_load_without_diagnostics() {
        let corpus = Corpus::generate(1, SEED);
        let root = mara::project::select(corpus.root(), None).expect("the corpus is a project");
        let project = mara::project::load(root, &mut WorkCounts::default());
        assert_eq!(project.diagnostics, []);
        let include: Vec<&str> = project
            .config
            .include
            .iter()
            .flatten()
            .map(|pattern| pattern.path.as_str())
            .collect();
        assert_eq!(include, ["docs/**/*.mara.md"]);
        let schema = project.schema.expect("the schema loads");
        assert_eq!((schema.types.len(), schema.relations.len()), (11, 14));
        assert!(schema.types["term"].fields["aliases"].repeatable);
        assert!(schema.relations["supersedes"].same_type && schema.relations["supersedes"].acyclic);
    }

    /// The generator writes the current item format: every declared document parses without
    /// diagnostics into the items generated for it, with one parse each.
    #[test]
    fn writes_the_current_item_format() {
        for size in PULL_REQUEST_SIZES {
            let corpus = Corpus::generate(size, SEED);
            let mut work = WorkCounts::default();
            let (mut documents, mut items) = (0, 0);
            for (path, contents) in files(corpus.root()) {
                // `docs/**/*.mara.md`, except what `.gitignore` excludes.
                let name = path.to_string_lossy();
                if !name.starts_with("docs/")
                    || name.starts_with("docs/generated/")
                    || !name.ends_with(".mara.md")
                {
                    continue;
                }
                let parsed = document::parse(&contents, &mut work);
                let diagnostics: Vec<String> =
                    parsed.diagnostics(&name).map(|d| d.to_string()).collect();
                assert!(diagnostics.is_empty(), "{}", diagnostics.join("\n"));
                // With lone-CR line endings the document reads alike (DES-REGIONS).
                let lone_cr: Vec<u8> = contents
                    .iter()
                    .map(|&byte| if byte == b'\n' { b'\r' } else { byte })
                    .collect();
                let reread = document::parse(&lone_cr, &mut WorkCounts::default());
                assert_eq!(reread, parsed, "{name} with lone CR");
                documents += 1;
                items += parsed.items.len();
            }
            let expected = (corpus.documents, corpus.items);
            assert_eq!(
                (documents, items),
                expected,
                "documents and items at {size}×"
            );
            let parses = WorkCounts {
                parses: documents as u64,
                ..WorkCounts::default()
            };
            assert_eq!(work, parses, "work at {size}×");
        }
    }

    /// Every file below `root` with its contents, by relative path.
    fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut directories = vec![root.to_path_buf()];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(&directory).expect("read a corpus directory") {
                let path = entry.expect("read a corpus directory entry").path();
                if path.is_dir() {
                    directories.push(path);
                } else {
                    let contents = fs::read(&path).expect("read a corpus file");
                    let relative = path
                        .strip_prefix(root)
                        .expect("below the root")
                        .to_path_buf();
                    files.push((relative, contents));
                }
            }
        }
        files.sort();
        files
    }
}
