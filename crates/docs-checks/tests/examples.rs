//! The examples that this repository's pages show load in the real code: the item format, the
//! rule consumers and the fact declarations of the rules page, and the project, schema and code
//! examples of the project and code pages. The tests read the pages from the checkout at run time.

use std::fs;
use std::path::{Path, PathBuf};

use mara::project::{load, select};
use mara_engine::config::{DEFAULT_TIME_LIMIT_SECONDS, EntryKind, Named, Rules, parse_config};
use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::{self, Document};
use mara_engine::facts::{MARA_DL, MARA_DL_VERSION};
use mara_engine::schema::FieldKind;
use mara_engine::work::WorkCounts;

/// The file at `relative` of this repository.
fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// Every diagnostic of a document.
fn diagnostics(document: &Document) -> Vec<Diagnostic> {
    document.diagnostics("docs/a.mara.md").collect()
}

#[test]
fn reads_the_documented_example() {
    let page = read("docs/format.mara.md");
    let (_, example) = page.split_once("```markdown\n").expect("an example");
    let (example, _) = example.split_once("\n```\n").expect("the example ends");
    let document = document::parse(example.as_bytes(), &mut WorkCounts::default());
    assert_eq!(diagnostics(&document), []);
    assert!(document.mentions.is_empty());
    let [requirement, decision] = &document.items[..] else {
        panic!("two items: {:?}", document.items);
    };

    assert_eq!(requirement.mid, "01M1PXP2KGVW5ZF2JGP9K4XE9B");
    assert_eq!(requirement.title, "Phishing-resistant authentication");
    assert_eq!(requirement.line, 7);
    let rows: Vec<_> = requirement
        .rows
        .iter()
        .map(|row| (row.line, row.name.as_str(), row.value.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (11, "type", "Requirement"),
            (13, "id", "REQ-PHISHING-RESISTANT-AUTH"),
            (14, "status", "Accepted"),
            (
                15,
                "satisfies",
                "[NEED-SECURE-SIGN-IN](../needs.mara.md#01M1PXQ0R8T4D2VW6YH3KJ5N7A)"
            ),
        ]
    );
    let link = requirement.rows[3].link.as_ref().expect("a relation link");
    assert_eq!(link.text, "NEED-SECURE-SIGN-IN");
    assert_eq!(link.path, "../needs.mara.md");
    assert_eq!(link.fragment.as_deref(), Some("01M1PXQ0R8T4D2VW6YH3KJ5N7A"));
    assert_eq!(link.line, 15);
    assert_eq!(&example[link.span.clone()], requirement.rows[3].value);
    let body = &example[requirement.body.clone()];
    assert!(body.starts_with("Users must authenticate"), "{body:?}");
    assert!(body.ends_with("- Passkeys must be supported."), "{body:?}");
    assert!(example[requirement.span.clone()].starts_with("<a id="));

    assert_eq!(decision.value("id"), Some("ADR-USE-WEBAUTHN"));
    let link = decision.rows[3].link.as_ref().expect("a same-file link");
    assert_eq!(
        (link.path.as_str(), link.fragment.as_deref()),
        ("", Some(requirement.mid.as_str()))
    );
    assert_eq!(
        &example[decision.body.clone()],
        "WebAuthn provides phishing-resistant authentication."
    );
}

#[test]
fn mara_dl_is_the_version_that_des_fact_bridge_shows() {
    let page = read("docs/rules/index.mara.md");
    assert!(page.contains(&format!("```souffle\n{MARA_DL}```")));
    assert!(MARA_DL.starts_with(".once\n"));
    assert!(MARA_DL.contains(&format!("format version {MARA_DL_VERSION} ")));
}

const HEAD: &str = "format_version = 1\nschema = \"s.toml\"\n[documents]\nformat_version = 1\ninclude = [\"*.mara.md\"]\n";

/// The rules that a configuration with `text` after a valid head declares, and its diagnostics.
fn parse_rules(text: &str) -> (Option<Rules>, Vec<Diagnostic>) {
    let (config, diagnostics) = parse_config(format!("{HEAD}{text}").as_bytes());
    (config.rules, diagnostics)
}

/// The example of DES-RULE-CONSUMERS in `docs/rules/index.mara.md` loads as written.
#[test]
fn loads_the_example_of_the_design() {
    let page = read("docs/rules/index.mara.md");
    let start = page.find("```toml\n[rules]").expect("the example") + "```toml\n".len();
    let example = &page[start..start + page[start..].find("```").unwrap()];
    let (rules, diagnostics) = parse_rules(example);
    assert_eq!(diagnostics, []);
    let rules = rules.unwrap();
    assert_eq!(rules.programs[0].path, ".mara/rules/*.dl");
    assert_eq!(rules.time_limit_seconds(), DEFAULT_TIME_LIMIT_SECONDS);
    let kinds: Vec<(EntryKind, &str)> = rules
        .entries
        .iter()
        .map(|entry| (entry.kind, entry.name.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (EntryKind::Query, "requirement-basis"),
            (EntryKind::Invariant, "requirement-verified"),
            (EntryKind::Gate, "verified"),
        ]
    );
    let invariant = &rules.entries[1];
    assert_eq!(
        invariant.description,
        "Accepted requirement has no accepted verification"
    );
    let gate = &rules.entries[2];
    let names = |list: &[&Named]| list.iter().map(|n| n.name.clone()).collect::<Vec<_>>();
    assert_eq!(
        names(&gate.relations()),
        [
            "requirement",
            "not_accepted",
            "not_implemented",
            "verification_not_passed",
            "code_unavailable",
            "result_unavailable"
        ]
    );
    assert_eq!(
        names(&gate.inputs.iter().collect::<Vec<_>>()),
        ["candidate", "verification_result"]
    );
    assert_eq!(
        rules.declaring_line(".mara/rules/gates.dl"),
        rules.programs[0].line
    );
}

/// The examples of DES-PROJECT-CONFIGURATION and DES-SCHEMA-MODEL, taken from the page as
/// written, load as one project without diagnostics.
#[test]
fn the_documented_examples_load_as_a_project() {
    let dir = tempfile::tempdir().expect("create a temporary directory");
    let root = write_project(
        dir.path(),
        &documented_example("DES-PROJECT-CONFIGURATION"),
        &documented_example("DES-SCHEMA-MODEL"),
    );
    let project = load(root, &mut WorkCounts::default());
    assert_eq!(project.diagnostics, []);
    let include = project.config.include.unwrap();
    assert_eq!(
        (include[0].path.as_str(), include[0].line, include.len()),
        ("docs/**/*.mara.md", 6, 1)
    );

    let schema = project.schema.unwrap();
    assert_eq!(schema.statuses, ["draft", "accepted", "retired"]);
    assert_eq!(
        schema.types.keys().collect::<Vec<_>>(),
        ["need", "requirement", "risk", "term"]
    );
    assert_eq!(
        schema.types["risk"].fields["likelihood"].kind,
        FieldKind::Enum(
            ["rare", "unlikely", "possible", "likely", "almost-certain"]
                .map(str::to_owned)
                .to_vec()
        )
    );
    let aliases = &schema.types["term"].fields["aliases"];
    assert!(aliases.kind == FieldKind::Text && aliases.repeatable && !aliases.required);
    assert_eq!(
        schema.relations.keys().collect::<Vec<_>>(),
        ["implements", "satisfies", "supersedes"]
    );
    assert_eq!(
        schema.relations["implements"]
            .targets
            .keys()
            .collect::<Vec<_>>(),
        ["code"]
    );
    let supersedes = &schema.relations["supersedes"];
    assert_eq!(supersedes.inverse.as_deref(), Some("superseded_by"));
    assert_eq!(
        supersedes.targets.keys().collect::<Vec<_>>(),
        ["need", "requirement"]
    );
    assert!(supersedes.same_type && supersedes.acyclic);
}

/// The code language example of DES-CODE-MARKERS, added to the configuration example, loads
/// without diagnostics.
#[test]
fn the_documented_code_example_loads() {
    let code = read("docs/code/index.mara.md");
    let dir = tempfile::tempdir().expect("create a temporary directory");
    let example = format!(
        "{}\n{}",
        documented_example("DES-PROJECT-CONFIGURATION"),
        example_on(&code, "DES-CODE-MARKERS")
    );
    let root = write_project(
        dir.path(),
        &example,
        &documented_example("DES-SCHEMA-MODEL"),
    );
    let project = load(root, &mut WorkCounts::default());
    assert_eq!(project.diagnostics, []);
    let rust = &project.config.code[0];
    assert_eq!(
        (rust.name.as_str(), rust.include[0].path.as_str()),
        ("rust", "src/**/*.rs")
    );
    assert_eq!(rust.prefixes, ["//", "///", "*"]);
}

/// Writes the project with `config` and `schema` at `p` under `dir` and selects it.
fn write_project(dir: &Path, config: &str, schema: &str) -> PathBuf {
    let mara = dir.join("p/.mara");
    fs::create_dir_all(&mara).unwrap();
    fs::write(mara.join("config.toml"), config).unwrap();
    fs::write(mara.join("schema.toml"), schema).unwrap();
    select(&dir.join("p"), None).unwrap()
}

/// The first TOML code block of the item with `id` on `docs/project.mara.md`, before the next
/// item's anchor.
fn documented_example(id: &str) -> String {
    example_on(&read("docs/project.mara.md"), id).to_owned()
}

/// The first TOML code block of the item with `id` on `page`, before the next item's anchor.
fn example_on<'p>(page: &'p str, id: &str) -> &'p str {
    let start = page
        .find(&format!("| ID | {id} |"))
        .unwrap_or_else(|| panic!("the page has no item {id}"));
    let item = &page[start..];
    let item = &item[..item.find("<a id=").unwrap_or(item.len())];
    let (_, block) = item
        .split_once("```toml\n")
        .unwrap_or_else(|| panic!("{id} has no TOML example"));
    let (example, _) = block
        .split_once("```\n")
        .unwrap_or_else(|| panic!("the TOML example of {id} does not end"));
    example
}
