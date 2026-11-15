//! A fixture for each row of DES-CONVERSION-MAPPING, include patterns with brace alternatives and
//! character classes, items without a status, and values that would become one label
//! (VER-CONVERSION).

use crate::{Project, report};

pub const MID_A: &str = "01M7FX0A000000000000000001";
pub const MID_B: &str = "01M7FX0A000000000000000002";
pub const MID_D: &str = "01M7FX0A000000000000000003";

pub const CONFIG: &str = r#"format_version = 3

[project]
name = "fixture"
schema = ".mara/schema.yaml"

[content]
include = ["docs/**/*.mara.md"]

[rules]
format_version = 1
files = [".mara/rules.yaml"]

[[code.languages]]
name = "rust"
command = ["rust-analyzer", "scip", ".", "--output", "{output}"]
position_encoding = "utf8"
extensions = ["rs"]
grammar = ".mara/code/rust.wasm"
query = ".mara/code/rust.scm"
"#;

pub const SCHEMA: &str = r#"format_version: 3
flavours:
  requirement:
    description: An obligation.
    use_when:
      - State an obligation.
    avoid_when: []
    distinguish_from: {}
    id_prefix: REQ-
    body: required
    fields:
      status: {type: enum, required: true, values: [draft, accepted]}
      priority: {type: integer}
      weight: {type: number}
      critical: {type: boolean}
      concern: {type: string, repeatable: true}
      kind: {type: enum, values: [functional, quality]}
  design:
    description: A solution.
    use_when: []
    avoid_when: []
    distinguish_from: {}
    id_prefix: DES-
    body: optional
    fields:
      status: {type: enum, required: true, values: [draft, accepted, retired]}
relations:
  satisfies:
    description: The design satisfies the requirement.
    source: [design]
    target: [requirement]
    inverse: satisfied_by
    acyclic: {severity: error}
  depends_on:
    description: The source needs the target.
    source: [requirement, design]
    target: [requirement, design]
    inverse: required_by
  related_to:
    description: An association without direction.
    source: [requirement, design]
    target: [requirement, design]
    symmetric: true
  implements:
    description: Code implements the requirement.
    source: []
    target: [requirement]
    code_source: true
    inverse: implemented_by
  sourced_from:
    description: An external source.
    source: [requirement]
    target: []
    external: true
"#;

pub const REQUIREMENTS: &str = "# Requirements

Narrative names [[REQ-A]], [[01M7FX0A000000000000000003]], [[satisfies:REQ-A]] and [[related_to:REQ-A]].

:::mara requirement REQ-A
:mid: 01M7FX0A000000000000000001
:title: First requirement
:status: accepted
:priority: 3
:weight: 0.5
:critical: true
:concern: safety
:concern: speed
:kind: functional
:depends_on: REQ-B
:related_to: DES-B
:implemented_by: code:src/lib.rs
:sourced_from: external:https://example.com/spec
:satisfied_by: DES-B

Body naming [[DES-B]], [[depends_on:DES-B]], [[required_by:REQ-B]], [[related_to:REQ-B]] and [[implemented_by:code:src/main.rs]].

Section
-------

Before the break.

***

After the break.
:::

:::mara requirement REQ-B
:mid: 01M7FX0A000000000000000002
:title: Second requirement
:status: draft
:::
";

pub const DESIGNS: &str = "# Designs

:::mara design DES-B
:mid: 01M7FX0A000000000000000003
:title: The design
:status: accepted
:satisfies: REQ-A

Satisfies [[REQ-A]].
:::
";

/// The fixture of every mapping row.
pub fn fixture() -> Project {
    let project = Project::empty();
    project.write(".mara/project.toml", CONFIG);
    project.write(".mara/schema.yaml", SCHEMA);
    project.write(".mara/rules.yaml", "- id: rule:example\n");
    project.write("docs/requirements.mara.md", REQUIREMENTS);
    project.write("docs/design/designs.mara.md", DESIGNS);
    project
}

#[test]
fn converts_each_mapping_row_and_reports_what_it_leaves_out() {
    let project = fixture();
    let run = project.convert();
    let a = format!("[REQ-A](#{MID_A})");
    let b = format!("[REQ-B](#{MID_B})");
    let d = format!("[DES-B](design/designs.mara.md#{MID_D})");
    let expected = format!(
        "# Requirements

Narrative names {a}, {d}, {a} and [[related_to:REQ-A]].

---

<a id=\"{MID_A}\"></a>

## First requirement

| Type | Requirement |
| --- | --- |
| ID | REQ-A |
| Status | Accepted |
| Priority | 3 |
| Weight | 0.5 |
| Critical | true |
| Concern | safety |
| Concern | speed |
| Kind | Functional |
| Depends on | {b} |
| Depends on | {d} |

Body naming {d}, {d}, {b}, [[related_to:REQ-B]] and [[implemented_by:code:src/main.rs]].

## Section

Before the break.

<hr>

After the break.

---

<a id=\"{MID_B}\"></a>

## Second requirement

| Type | Requirement |
| --- | --- |
| ID | REQ-B |
| Status | Draft |
| Depends on | {a} |
"
    );
    assert_eq!(project.read("docs/requirements.mara.md"), expected);
    let a_from_design = format!("[REQ-A](../requirements.mara.md#{MID_A})");
    let expected = format!(
        "# Designs

---

<a id=\"{MID_D}\"></a>

## The design

| Type | Design |
| --- | --- |
| ID | DES-B |
| Status | Accepted |
| Satisfies | {a_from_design} |

Satisfies {a_from_design}.
"
    );
    assert_eq!(project.read("docs/design/designs.mara.md"), expected);
    assert_eq!(
        project.read(".mara/schema.toml"),
        r#"format_version = 1
statuses = ["draft", "accepted", "retired"]

[types.requirement]
id_prefix = "REQ-"
description = "An obligation."

[types.requirement.fields.priority]
kind = "text"

[types.requirement.fields.weight]
kind = "text"

[types.requirement.fields.critical]
kind = "text"

[types.requirement.fields.concern]
kind = "text"
repeatable = true

[types.requirement.fields.kind]
kind = "enum"
values = ["functional", "quality"]

[types.design]
id_prefix = "DES-"
description = "A solution."

[relations.satisfies]
inverse = "satisfied_by"
description = "The design satisfies the requirement."
targets = { design = ["requirement"] }
acyclic = true

[relations.depends_on]
inverse = "required_by"
description = "The source needs the target."
targets = { requirement = ["requirement", "design"], design = ["requirement", "design"] }

[relations.implements]
inverse = "implemented_by"
description = "Code implements the requirement."
targets = { code = ["requirement"] }
"#
    );
    assert_eq!(
        project.read(".mara/config.toml"),
        "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n"
    );

    let entries = run.entries();
    assert_eq!(report(&project), entries);
    let place = |path: &str, line: u64| -> Vec<&str> {
        entries
            .iter()
            .filter(|(p, l, _)| p == path && *l == line)
            .map(|(_, _, message)| message.as_str())
            .collect()
    };
    let has = |path: &str, line: u64, text: &str| {
        assert!(
            place(path, line)
                .iter()
                .any(|message| message.contains(text)),
            "no entry at {path}:{line} with {text:?}: {entries:#?}"
        );
    };
    let docs = "docs/requirements.mara.md";
    let schema = ".mara/schema.yaml";
    let config = ".mara/project.toml";
    has(config, 4, "project name");
    has(config, 10, "the YAML rule files `.mara/rules.yaml`");
    has(config, 14, "code language `rust`");
    has(
        schema,
        3,
        "the guidance `use_when` of flavour `requirement`",
    );
    has(
        schema,
        13,
        "field `priority` of flavour `requirement` has type `integer`",
    );
    has(schema, 14, "has type `number`");
    has(schema, 15, "has type `boolean`");
    has(schema, 24, "`body: optional` of flavour `design`");
    has(
        schema,
        39,
        "relation `related_to` is left out: it is symmetric",
    );
    has(
        schema,
        50,
        "relation `sourced_from` is left out: it leads to external addresses",
    );
    has(
        docs,
        3,
        "`[[satisfies:REQ-A]]` in narrative outside an item becomes a mention link",
    );
    has(docs, 3, "`[[related_to:REQ-A]]` stays as text");
    has(docs, 16, "`:related_to: DES-B` is left out");
    has(docs, 17, "`:implemented_by: code:src/lib.rs` is left out");
    has(
        docs,
        18,
        "`:sourced_from: external:https://example.com/spec` is left out",
    );
    has(docs, 21, "`[[related_to:REQ-B]]` stays as text");
    has(
        docs,
        21,
        "`[[implemented_by:code:src/main.rs]]` stays as text",
    );
    has(docs, 33, "item `REQ-B` has an empty body");
    assert_eq!(entries.len(), 18, "{entries:#?}");

    // Valid but for the item reported for the author to complete.
    let validation = project.mara(&["project", "validate"]);
    let diagnostics: Vec<(&str, &str)> = validation.json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap(),
                d["id"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(diagnostics, [("empty-body", "REQ-B")], "{validation:?}");
}

#[test]
fn keeps_brace_alternatives_and_character_classes_as_written_and_reports_them() {
    let project = fixture();
    let config = CONFIG.replace(
        "include = [\"docs/**/*.mara.md\"]",
        "include = [\"docs/{requirements,other}.mara.md\", \"docs/design/[dx]esigns.mara.md\"]",
    );
    project.write(".mara/project.toml", config);
    project.write("docs/unmatched.mara.md", "# Not a document\n\n[[REQ-A]]\n");
    let run = project.convert();
    assert!(project.read(".mara/config.toml").ends_with(
        "include = [\"docs/{requirements,other}.mara.md\", \"docs/design/[dx]esigns.mara.md\"]\n"
    ));
    // The documents that 0.3.0 read through those patterns are converted.
    assert!(
        project
            .read("docs/requirements.mara.md")
            .contains(&format!("<a id=\"{MID_A}\"></a>"))
    );
    assert!(
        project
            .read("docs/design/designs.mara.md")
            .contains(&format!("<a id=\"{MID_D}\"></a>"))
    );
    assert_eq!(
        project.read("docs/unmatched.mara.md"),
        "# Not a document\n\n[[REQ-A]]\n"
    );
    let reported: Vec<(String, u64)> = run
        .entries()
        .into_iter()
        .filter(|(_, _, message)| message.contains("is kept as written"))
        .map(|(path, line, _)| (path, line))
        .collect();
    assert_eq!(
        reported,
        [
            (".mara/project.toml".to_owned(), 8),
            (".mara/project.toml".to_owned(), 8)
        ]
    );
}

#[test]
fn gives_items_without_a_status_the_status_named_for_the_run() {
    let project = Project::empty();
    project.write(".mara/project.toml", CONFIG);
    // As the 0.3.0 `minimal` template: no flavour has a status field.
    project.write(
        ".mara/schema.yaml",
        "format_version: 3\nflavours:\n  requirement:\n    description: An obligation.\n    use_when: [x]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: REQ-\n    body: required\n    fields: {}\nrelations: {}\n",
    );
    let document = format!(":::mara requirement REQ-A\n:mid: {MID_A}\n:title: A\n\nBody.\n:::\n");
    project.write("docs/a.mara.md", &document);
    let before = project.stamped();
    let refused = project.mara(&["project", "convert"]);
    assert_eq!(
        (refused.status, refused.code()),
        (2, "invalid-argument"),
        "{refused:?}"
    );
    assert_eq!(
        refused.diagnostics(),
        [("docs/a.mara.md".to_owned(), 1, "missing-key".to_owned())]
    );
    assert_eq!(project.stamped(), before, "nothing written");

    project.convert_with_status("Draft");
    let item = &project.items("docs/a.mara.md").items[0];
    assert_eq!(item.value("status"), Some("Draft"));
    assert!(
        project
            .read(".mara/schema.toml")
            .contains("statuses = [\"draft\"]\n")
    );
    assert_eq!(project.mara(&["project", "validate"]).status, 0);

    // An item without a status among items that have one.
    let project = fixture();
    project.write(
        "docs/design/designs.mara.md",
        DESIGNS.replace(":status: accepted\n", ""),
    );
    project.convert_with_status("retired");
    let item = &project.items("docs/design/designs.mara.md").items[0];
    assert_eq!(item.value("status"), Some("Retired"));
    assert!(
        project
            .read(".mara/schema.toml")
            .contains("statuses = [\"draft\", \"accepted\", \"retired\"]\n")
    );
}

#[test]
fn writes_nothing_when_two_values_or_statuses_become_one_label() {
    let project = fixture();
    let schema = SCHEMA
        .replace(
            "values: [functional, quality]",
            "values: [In progress, in_progress, done]",
        )
        .replace(
            "values: [draft, accepted, retired]",
            "values: [Draft, accepted, retired]",
        );
    project.write(".mara/schema.yaml", schema);
    let before = project.stamped();
    let refused = project.mara(&["project", "convert"]);
    assert_eq!(
        (refused.status, refused.code()),
        (1, "invalid-project"),
        "{refused:?}"
    );
    assert_eq!(
        refused.diagnostics(),
        [
            (
                ".mara/schema.yaml".to_owned(),
                17,
                "duplicate-name".to_owned()
            ),
            (
                ".mara/schema.yaml".to_owned(),
                26,
                "duplicate-name".to_owned()
            ),
        ]
    );
    let messages = refused.json["error"]["diagnostics"].to_string();
    assert!(
        messages.contains("`In progress` and `in_progress`"),
        "{messages}"
    );
    assert!(messages.contains("`draft` and `Draft`"), "{messages}");
    assert_eq!(project.stamped(), before, "nothing written");
}

impl Project {
    pub fn convert_with_status(&self, status: &str) {
        let run = self.mara(&["project", "convert", "--status", status]);
        assert_eq!(run.status, 0, "{run:?}");
    }
}

/// The booleans 0.3.0 read, in any spelling it took; a value that cannot be read as what its
/// key needs, and a YAML merge key, are reported (review findings 3 and 4).
#[test]
fn reads_the_yaml_booleans_0_3_0_read_and_reports_what_it_cannot_read() {
    let project = fixture();
    let schema = SCHEMA
        .replace(
            "      concern: {type: string, repeatable: true}\n",
            "      concern: {type: string, required: True, repeatable: YES}\n      notes: &notes {type: string, required: maybe}\n      rationale: {<<: *notes}\n",
        )
        .replace("acyclic: {severity: error}", "acyclic: true");
    project.write(".mara/schema.yaml", schema);
    let run = project.convert();
    let converted = project.read(".mara/schema.toml");
    assert!(
        converted.contains("[types.requirement.fields.concern]\nkind = \"text\"\nrequired = true\nrepeatable = true\n"),
        "{converted}"
    );
    assert!(
        converted.contains("[types.requirement.fields.notes]\nkind = \"text\"\n\n"),
        "{converted}"
    );
    let messages: Vec<(u64, String)> = run
        .entries()
        .into_iter()
        .filter(|(path, _, _)| path == ".mara/schema.yaml")
        .map(|(_, line, message)| (line, message))
        .collect();
    let has = |line: u64, text: &str| {
        assert!(
            messages.iter().any(|(l, m)| *l == line && m.contains(text)),
            "no entry at line {line} with {text:?}: {messages:#?}"
        );
    };
    has(
        17,
        "`required` of field `notes` of flavour `requirement` is left out: it is not `true` or `false`",
    );
    has(18, "the YAML merge key `<<` is not applied");
    has(
        35,
        "`acyclic` of relation `satisfies` is left out: it is not a mapping",
    );
}

/// A field that the flavour declares is no relation, even when another flavour's relation has
/// its name as its inverse (review finding 2).
#[test]
fn reads_a_declared_field_as_a_field_before_a_relation_of_its_name() {
    let project = fixture();
    let schema = SCHEMA.replace(
        "relations:\n",
        "  note:\n    description: A note.\n    use_when: []\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: NOTE-\n    body: required\n    fields:\n      status: {type: enum, values: [draft]}\n      required_by: {type: string}\nrelations:\n",
    );
    project.write(".mara/schema.yaml", schema);
    project.write(
        "docs/notes.mara.md",
        "# Notes\n\n:::mara note NOTE-A\n:mid: 01M7FX0A000000000000000009\n:title: A note\n:status: draft\n:required_by: Compliance policy\n\nText.\n:::\n",
    );
    let run = project.convert();
    let item = &project.items("docs/notes.mara.md").items[0];
    assert_eq!(item.value("required_by"), Some("Compliance policy"));
    let about = run
        .entries()
        .into_iter()
        .filter(|(path, _, _)| path == "docs/notes.mara.md")
        .collect::<Vec<_>>();
    assert_eq!(about, [], "nothing of the note is left out");
    let validation = project.mara(&["project", "validate"]);
    let codes: Vec<&str> = validation.json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["empty-body"], "{validation:?}");
}
