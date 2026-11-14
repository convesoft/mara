//! VER-SEEDED-PROBLEMS: one instance of each failure of the built-in checks, several to a file,
//! with an unknown configuration setting and an invalid schema declaration. Also the
//! `project validate` case of VER-INVALID-ROWS and the inventory's problems.

use crate::{CONFIG, Dir, SCHEMA, at, document, item, json, line_of};

const FRONT: &str = "01M7DM82XJX9NQ0D40303V632B";
const S1: &str = "01M7DM82XSE05EQWXVBQM3ZQ48";
const S2: &str = "01M7DM82XZZ4XW8Y1KNFG4K7QC";
const S3: &str = "01M7DM82Y7PZB2QDVTPV1D6NWN";
const S4: &str = "01M7DM82YGX3G4DB0A8QFNBXJB";
const S5: &str = "01M7DM82YQ0VK5MDRBZG07Q0BV";
const M1: &str = "01M7DM82Z13YME6KN3G59NRWN4";
const M2: &str = "01M7DM82ZAWF5VTEN7BPZQ3ZH4";
const M3: &str = "01M7DM82ZH8K4VS7BCA1YZJRCR";
const M4: &str = "01M7DM82ZVHCTJXWZ201ZG3KRW";
const M5: &str = "01M7DM8303EN0412TYTF1Y7S7Z";
const L1: &str = "01M7DM830BDZR0MJN9KS85NBAK";
const L2: &str = "01M7DM830J653Z38S9Z1BJVW2X";
const L3: &str = "01M7DM830WWYPAH723KJ58EZ3S";
const N1: &str = "01M7DM8315V8CZQ1KGMSYFT2F4";
const GONE: &str = "01M7DM831DDXTMEPS2TY9CXAME";
const C1: &str = "01M7DM831MGD7KSR5AQGE6JCY2";
const C2: &str = "01M7DM831VG03EDQSBFY3KXFYG";
const DA: &str = "01M7DM8321YAYV8BYH5CE43FP2";
const DB: &str = "01M7DM8329AA8TPVT7V63AVF6X";
const K1: &str = "01M7DM832GTZ3XA3GA0MH05RSB";
const OTHER: &str = "01M7DM832RRGXDYBMGG5ZNAMWR";

const DRAFT: &str = "| ID | {id} |\n| Status | Draft |\n";

fn rows(id: &str, extra: &str) -> String {
    DRAFT.replace("{id}", id) + extra
}

/// A seeded problem: its code, the file and text at whose line it is reported, and the edits
/// that remove it, each a file, a text in it and its replacement.
struct Seed {
    code: &'static str,
    path: &'static str,
    marker: String,
    fixes: Vec<(&'static str, String, String)>,
}

/// A problem reported where `seeded` starts and removed by replacing it with `fixed`.
fn seed(path: &'static str, code: &'static str, seeded: &str, fixed: &str) -> Seed {
    Seed {
        code,
        path,
        marker: seeded.to_owned(),
        fixes: vec![(path, seeded.to_owned(), fixed.to_owned())],
    }
}

/// A problem reported where `marker` starts and removed by `fixes`.
fn seed_at(
    path: &'static str,
    code: &'static str,
    marker: &str,
    fixes: &[(&'static str, &str, &str)],
) -> Seed {
    Seed {
        code,
        path,
        marker: marker.to_owned(),
        fixes: fixes
            .iter()
            .map(|(path, from, to)| (*path, (*from).to_owned(), (*to).to_owned()))
            .collect(),
    }
}

/// The files of the seeded project, with every problem in place.
fn files() -> Vec<(&'static str, String)> {
    let config = CONFIG.replace(
        "schema = \".mara/schema.toml\"\n",
        "schema = \".mara/schema.toml\"\ncolour = \"blue\"\n",
    );
    let schema = SCHEMA.replace(
        "[types.note]\n",
        "[types.requirement.fields.weight]\nkind = \"number\"\n\n[types.note]\n",
    );
    let without_body = format!(
        "<a id=\"{S4}\"></a>\n\n### Without body\n\n| Type | Requirement |\n| --- | --- |\n{}",
        rows("REQ-S4", "")
    );
    let structure = format!(
        "---\ntitle: Structure\n<a id=\"{FRONT}\"></a>\n---\n\nStructure\n=========\n\n---\n\n{}\n{}\n---\n\n{}\n---\n\n<a id=\"{S5}\"></a>\n\nIncomplete: no heading.\n\n---\n\n<script>\nlet x = 1;\n</SCRIPT>\n",
        item(
            S1,
            "First structured",
            "Requirement",
            &rows("REQ-S1", "| Priority |\n"),
            "Body."
        ),
        item(
            S2,
            "Second structured",
            "Requirement",
            &rows("REQ-S2", ""),
            &format!("Body text with an anchor <a id=\"{S3}\"></a> merged.")
        ),
        without_body,
    );
    let metadata = document(
        "Metadata",
        &[
            item(
                M1,
                "Metadata problems",
                "Requirement",
                &rows(
                    "REQ-M1",
                    "| Status | Accepted |\n| Priority | Medium |\n| Tags | x |\n| Tags | x |\n| Colour | red |\n",
                ),
                "Body.",
            ),
            item(
                M2,
                "Unknown type",
                "Story",
                "| ID | STORY-M2 |\n| Status | Done |\n",
                "Body.",
            ),
            item(
                M3,
                "Missing status",
                "Requirement",
                "| ID | REQ-M3 |\n",
                "Body.",
            ),
            item(M4, "Bad ID", "Requirement", &rows("REQ-m4", ""), "Body."),
            item(M5, "Wrong prefix", "Note", &rows("REQ-M5", ""), "Body."),
        ],
    );
    let links_a = document(
        "Links",
        &[
            item(
                L1,
                "Linking requirement",
                "Requirement",
                &rows(
                    "REQ-L1",
                    &format!("| Refines | [REQ-L2](wrong.mara.md#{L2}) |\n"),
                ),
                "Body.",
            ),
            item(
                L3,
                "More links",
                "Requirement",
                &rows(
                    "REQ-L3",
                    &format!(
                        "| Refines | [OLD-NAME](b.mara.md#{L2}) |\n| Refines | [REQ-GONE](b.mara.md#{GONE}) |\n\
                         | Refines | REQ-L2 |\n| Refines | somewhere |\n"
                    ),
                ),
                "Body.",
            ),
        ],
    );
    let links_b = document(
        "Targets",
        &[
            item(
                L2,
                "Target requirement",
                "Requirement",
                &rows("REQ-L2", ""),
                "Body.",
            ),
            item(
                N1,
                "A note",
                "Note",
                &rows("NOTE-N1", &format!("| Explains | [NOTE-N1](#{N1}) |\n")),
                "Body.",
            ),
        ],
    );
    let cycle = document(
        "Cycle",
        &[
            item(
                C1,
                "First of a cycle",
                "Requirement",
                &rows("REQ-C1", &format!("| Refines | [REQ-C2](#{C2}) |\n")),
                "Body.",
            ),
            item(
                C2,
                "Second of a cycle",
                "Requirement",
                &rows("REQ-C2", &format!("| Refines | [REQ-C1](#{C1}) |\n")),
                "Body.",
            ),
        ],
    );
    let duplicates = document(
        "Duplicates",
        &[
            item(
                DA,
                "Repeats an ID",
                "Requirement",
                &rows("REQ-L1", ""),
                "Body.",
            ),
            item(
                DB,
                "First of a MID",
                "Requirement",
                &rows("REQ-DB1", ""),
                "Body.",
            ),
            item(
                DB,
                "Second of a MID",
                "Requirement",
                &rows("REQ-DB2", ""),
                "Body.",
            ),
        ],
    );
    let clean = document(
        "Clean",
        &[item(
            K1,
            "Clean requirement",
            "Requirement",
            &rows("REQ-K1", ""),
            "Body.",
        )],
    );
    vec![
        (".mara/config.toml", config),
        (".mara/schema.toml", schema),
        ("docs/binary.mara.md", "# Binary\n".to_owned()),
        ("docs/structure.mara.md", structure),
        ("docs/metadata.mara.md", metadata),
        ("docs/links/a.mara.md", links_a),
        ("docs/links/b.mara.md", links_b),
        ("docs/cycle.mara.md", cycle),
        ("docs/dup.mara.md", duplicates),
        ("docs/clean.mara.md", clean),
    ]
}

/// Every seeded problem.
fn seeds() -> Vec<Seed> {
    const STRUCTURE: &str = "docs/structure.mara.md";
    const METADATA: &str = "docs/metadata.mara.md";
    const LINKS: &str = "docs/links/a.mara.md";
    let anchor = |mid: &str| format!("<a id=\"{mid}\"></a>");
    vec![
        seed(
            ".mara/config.toml",
            "unknown-key",
            "colour = \"blue\"\n",
            "",
        ),
        seed(
            ".mara/schema.toml",
            "invalid-value",
            "kind = \"number\"",
            "kind = \"text\"",
        ),
        seed(
            "docs/binary.mara.md",
            "invalid-utf8",
            "# Binary",
            "# Binary",
        ),
        seed(
            STRUCTURE,
            "item-in-front-matter",
            &format!("{}\n", anchor(FRONT)),
            "",
        ),
        seed(STRUCTURE, "setext-heading", "=========\n", ""),
        seed(
            STRUCTURE,
            "malformed-row",
            "| Priority |\n",
            "| Priority | Low |\n",
        ),
        seed(
            STRUCTURE,
            "missing-separator",
            &anchor(S2),
            &format!("---\n\n{}", anchor(S2)),
        ),
        seed(
            STRUCTURE,
            "merged-anchor",
            &format!("with an anchor {} merged", anchor(S3)),
            "without an anchor",
        ),
        seed_at(
            STRUCTURE,
            "empty-body",
            &anchor(S4),
            &[(
                STRUCTURE,
                "| ID | REQ-S4 |\n| Status | Draft |\n",
                "| ID | REQ-S4 |\n| Status | Draft |\n\nBody.\n",
            )],
        ),
        seed(
            STRUCTURE,
            "incomplete-item",
            &format!("{}\n\nIncomplete: no heading.\n", anchor(S5)),
            &format!(
                "{}\n\n### Complete\n\n| Type | Note |\n| --- | --- |\n| ID | NOTE-S5 |\n| Status | Draft |\n\nComplete.\n",
                anchor(S5)
            ),
        ),
        seed(STRUCTURE, "html-end-tag-case", "</SCRIPT>", "</script>"),
        seed(METADATA, "repeated-key", "| Status | Accepted |\n", ""),
        seed(
            METADATA,
            "invalid-value",
            "| Priority | Medium |\n",
            "| Priority | High |\n",
        ),
        seed(
            METADATA,
            "duplicate-row",
            "| Tags | x |\n| Colour",
            "| Colour",
        ),
        seed(METADATA, "undeclared-key", "| Colour | red |\n", ""),
        seed(
            METADATA,
            "undeclared-type",
            "| Type | Story |",
            "| Type | Note |",
        ),
        seed_at(
            METADATA,
            "invalid-value",
            "| Status | Done |",
            &[(
                METADATA,
                "| ID | STORY-M2 |\n| Status | Done |",
                "| ID | NOTE-M2 |\n| Status | Draft |",
            )],
        ),
        seed_at(
            METADATA,
            "missing-row",
            &anchor(M3),
            &[(
                METADATA,
                "| ID | REQ-M3 |\n",
                "| ID | REQ-M3 |\n| Status | Draft |\n",
            )],
        ),
        seed(METADATA, "invalid-id", "| ID | REQ-m4 |", "| ID | REQ-M4 |"),
        seed(
            METADATA,
            "wrong-id-prefix",
            "| ID | REQ-M5 |",
            "| ID | NOTE-M5 |",
        ),
        // Reported at the later holder in path order; removed by renaming the earlier one.
        seed_at(
            LINKS,
            "duplicate-id",
            "| ID | REQ-L1 |",
            &[("docs/dup.mara.md", "| ID | REQ-L1 |", "| ID | REQ-DA |")],
        ),
        seed(LINKS, "stale-link-path", "(wrong.mara.md#", "(b.mara.md#"),
        seed(LINKS, "stale-link-text", "[OLD-NAME]", "[REQ-L2]"),
        seed(
            LINKS,
            "broken-link",
            &format!("| Refines | [REQ-GONE](b.mara.md#{GONE}) |\n"),
            "",
        ),
        seed(LINKS, "bare-id-value", "| Refines | REQ-L2 |\n", ""),
        seed(
            LINKS,
            "invalid-relation-value",
            "| Refines | somewhere |\n",
            "",
        ),
        seed(
            "docs/links/b.mara.md",
            "disallowed-target",
            &format!("| Explains | [NOTE-N1](#{N1}) |"),
            &format!("| Explains | [REQ-L2](#{L2}) |"),
        ),
        // Reported at the first link of the cycle; removed by dropping the second.
        seed_at(
            "docs/cycle.mara.md",
            "relation-cycle",
            &format!("| Refines | [REQ-C2](#{C2}) |"),
            &[(
                "docs/cycle.mara.md",
                &format!("| Refines | [REQ-C1](#{C1}) |\n"),
                "",
            )],
        ),
        seed(
            "docs/dup.mara.md",
            "duplicate-mid",
            &format!("{}\n\n### Second", anchor(DB)),
            &format!("{}\n\n### Second", anchor(OTHER)),
        ),
    ]
}

/// The seeded project, or with `fixed`, the same project with every problem removed.
fn project(fixed: bool) -> Dir {
    let dir = Dir::new();
    let seeds = seeds();
    for (path, mut text) in files() {
        if fixed {
            for (_, from, to) in seeds.iter().flat_map(|s| &s.fixes).filter(|f| f.0 == path) {
                assert!(text.contains(from.as_str()), "{path}: {from:?}");
                text = text.replacen(from.as_str(), to, 1);
            }
        }
        let mut bytes = text.into_bytes();
        if path == "docs/binary.mara.md" && !fixed {
            bytes.insert("# Binary".len(), 0xFF);
        }
        dir.write(path, bytes);
    }
    dir
}

/// Where each seeded problem is reported, in path and line order.
fn expected(dir: &Dir) -> Vec<(String, String, u64)> {
    let mut expected: Vec<_> = seeds()
        .iter()
        .map(|seed| {
            let text =
                String::from_utf8_lossy(&std::fs::read(dir.path(seed.path)).unwrap()).into_owned();
            let line = line_of(&text, &seed.marker);
            at(seed.code, seed.path, line)
        })
        .collect();
    expected.sort_by(|a, b| (&a.1, a.2).cmp(&(&b.1, b.2)));
    expected
}

#[test]
fn project_validate_reports_each_seeded_problem_once_and_nothing_else() {
    let dir = project(false);
    let run = json(&dir.root(), &["project", "validate", "--limit", "100"]);
    assert_eq!(run.status, 1, "{run:?}");
    let result = run.json();
    let expected = expected(&dir);
    let mut reported = run.diagnostics();
    // Several problems on one line keep the order of their checks.
    reported.sort_by(|a, b| (&a.1, a.2, &a.0).cmp(&(&b.1, b.2, &b.0)));
    let mut sorted = expected.clone();
    sorted.sort_by(|a, b| (&a.1, a.2, &a.0).cmp(&(&b.1, b.2, &b.0)));
    assert_eq!(reported, sorted);
    assert_eq!(result["valid"], false);
    assert_eq!(result["errors"], expected.len() as u64);
    assert_eq!(result["warnings"], 0);
    assert_eq!(result["next_cursor"], serde_json::Value::Null);
    // Every diagnostic of an item names its ID and MID.
    let first = &result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "duplicate-id")
        .unwrap();
    assert_eq!(
        (first["id"].as_str(), first["mid"].as_str()),
        (Some("REQ-L1"), Some(L1))
    );
    for diagnostic in result["diagnostics"].as_array().unwrap() {
        assert_eq!(diagnostic["severity"], "error");
        assert!(!diagnostic["message"].as_str().unwrap().is_empty());
    }
}

#[test]
fn the_human_form_prints_each_diagnostic_and_the_verdict() {
    let dir = project(false);
    let run = crate::mara(&dir.root(), &["project", "validate", "--limit", "100"]);
    assert_eq!(run.status, 1);
    let lines: Vec<&str> = run.stdout.lines().collect();
    let count = expected(&dir).len();
    assert_eq!(lines.len(), count + 1, "{}", run.stdout);
    assert!(
        lines[0].starts_with(".mara/config.toml:3: error[unknown-key]: "),
        "{}",
        lines[0]
    );
    assert_eq!(lines[count], format!("invalid: {count} errors, 0 warnings"));
}

#[test]
fn schema_validate_reports_the_schema_problem_and_no_document() {
    let dir = project(false);
    let run = json(&dir.root(), &["schema", "validate"]);
    assert_eq!(run.status, 1);
    let line = line_of(&dir.read(".mara/schema.toml"), "kind = \"number\"");
    assert_eq!(
        run.diagnostics(),
        [at("invalid-value", ".mara/schema.toml", line)]
    );
    assert_eq!((run.file_reads(), run.parses()), (2, 0));
}

#[test]
fn item_validate_reports_the_problems_of_one_item_only() {
    let dir = project(false);
    let text = dir.read("docs/links/a.mara.md");
    let expected = [
        at(
            "duplicate-id",
            "docs/links/a.mara.md",
            line_of(&text, "| ID | REQ-L1 |"),
        ),
        at(
            "stale-link-path",
            "docs/links/a.mara.md",
            line_of(&text, "(wrong.mara.md#"),
        ),
    ];
    let run = json(&dir.root(), &["item", "validate", L1]);
    assert_eq!(run.status, 1, "{run:?}");
    assert_eq!(run.diagnostics(), expected);
    assert_eq!(run.json()["errors"], 2);
    // The other holder of the ID has the repetition in its own validation, at its own row.
    let run = json(&dir.root(), &["item", "validate", DA]);
    let text = dir.read("docs/dup.mara.md");
    assert_eq!(
        run.diagnostics(),
        [at(
            "duplicate-id",
            "docs/dup.mara.md",
            line_of(&text, "| ID | REQ-L1 |")
        )]
    );
    // An ID that two items have names neither; a MID does.
    let run = json(&dir.root(), &["item", "validate", "REQ-L1"]);
    assert_eq!(run.status, 1);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "ambiguous-item");
    assert!(
        error["message"].as_str().unwrap().contains("by its MID"),
        "{error}"
    );
    let run = json(&dir.root(), &["item", "validate", "REQ-K1"]);
    assert_eq!((run.status, run.json()["valid"].clone()), (0, true.into()));
    let run = json(&dir.root(), &["item", "validate", "REQ-NONE"]);
    assert_eq!(
        (run.status, run.json()["error"]["code"].clone()),
        (1, "unknown-item".into())
    );
}

#[test]
fn a_page_or_a_path_keeps_the_validity_and_counts_of_the_whole_project() {
    let dir = project(false);
    let expected = expected(&dir);
    let run = json(&dir.root(), &["project", "validate", "--limit", "1"]);
    assert_eq!(run.status, 1);
    let first = run.json();
    assert_eq!(first["valid"], false);
    assert_eq!(first["errors"], expected.len() as u64);
    assert_eq!(first["diagnostics"].as_array().unwrap().len(), 1);
    // Following every cursor returns each diagnostic once, in path and line order.
    let mut all = run.diagnostics();
    let mut cursor = first["next_cursor"].as_str().unwrap().to_owned();
    loop {
        let run = json(
            &dir.root(),
            &["project", "validate", "--limit", "7", "--cursor", &cursor],
        );
        assert_eq!(run.status, 1);
        all.extend(run.diagnostics());
        match run.json()["next_cursor"].as_str() {
            Some(next) => cursor = next.to_owned(),
            None => break,
        }
    }
    let full = json(&dir.root(), &["project", "validate", "--limit", "100"]).diagnostics();
    assert_eq!(all, full);
    // A cursor of another request is refused.
    let run = json(&dir.root(), &["schema", "validate", "--cursor", &cursor]);
    assert_eq!(
        (run.status, run.json()["error"]["code"].clone()),
        (1, "stale-cursor".into())
    );

    for path in ["docs/clean.mara.md", "docs/links/b.mara.md/x", "docs/none"] {
        let run = json(&dir.root(), &["project", "validate", "--path", path]);
        assert_eq!(run.status, 1, "{path}");
        let result = run.json();
        assert_eq!(result["valid"], false);
        assert_eq!(result["errors"], expected.len() as u64);
        assert_eq!(result["diagnostics"], serde_json::json!([]));
    }
    let run = json(
        &dir.root(),
        &[
            "project",
            "validate",
            "--path",
            "docs/links",
            "--path",
            ".mara/config.toml",
        ],
    );
    let paths: Vec<String> = run.diagnostics().into_iter().map(|d| d.1).collect();
    assert_eq!(paths.len(), 8, "{paths:?}");
    assert!(
        paths
            .iter()
            .all(|p| p.starts_with("docs/links/") || p == ".mara/config.toml")
    );
}

/// `--path` reads `.` parts, repeated and trailing `/` as 0.3.0 did, and refuses a path
/// outside the project (DES-DIAGNOSTICS).
#[test]
fn every_spelling_of_a_path_selects_the_same_diagnostics_and_key() {
    let dir = project(false);
    let page = |paths: &[&str]| {
        let mut args = vec!["project", "validate", "--limit", "2"];
        for path in paths {
            args.extend(["--path", path]);
        }
        let run = json(&dir.root(), &args);
        assert_eq!(run.status, 1, "{paths:?}: {run:?}");
        run.json()
    };
    let expected = page(&["docs/links"]);
    assert_eq!(expected["diagnostics"].as_array().unwrap().len(), 2);
    for paths in [
        &["docs/links/"][..],
        &["./docs/links"],
        &["./docs/./links/"],
        &["docs//links"],
        &["docs/links", "./docs/links/"],
    ] {
        assert_eq!(page(paths), expected, "{paths:?}");
    }
    // The cursor of one spelling continues another.
    let cursor = expected["next_cursor"].as_str().unwrap();
    let args = [
        "project",
        "validate",
        "--path",
        "./docs/links/",
        "--cursor",
        cursor,
    ];
    let run = json(&dir.root(), &args);
    assert_eq!(run.status, 1);
    let all = json(
        &dir.root(),
        &["project", "validate", "--path", "docs/links"],
    )
    .diagnostics();
    assert_eq!(run.diagnostics(), all[2..]);

    for path in ["/docs", "../docs", "docs/../x", ".", "./", ""] {
        let run = json(&dir.root(), &["project", "validate", "--path", path]);
        assert_eq!(run.status, 2, "{path:?}");
        assert_eq!(run.json()["error"]["code"], "invalid-argument", "{path:?}");
        assert_eq!(run.file_reads(), 0, "{path:?}");
    }
}

#[test]
fn the_project_without_its_problems_is_valid() {
    let dir = project(true);
    for args in [
        &["project", "validate"][..],
        &["schema", "validate"],
        &["item", "validate", "REQ-L1"],
    ] {
        let run = json(&dir.root(), args);
        assert_eq!(
            (run.status, run.json()["valid"].clone(), run.diagnostics()),
            (0, true.into(), vec![]),
            "{args:?}: {run:?}"
        );
    }
    let run = crate::mara(&dir.root(), &["project", "validate"]);
    assert_eq!(
        (run.status, run.stdout.as_str()),
        (0, "valid: 0 errors, 0 warnings\n")
    );
}

/// VER-INVALID-ROWS: an undeclared key, a value outside its field's allowed set and a row
/// that repeats another give one diagnostic each.
#[test]
fn reports_each_invalid_row_once() {
    let dir = Dir::project(SCHEMA);
    let rows = "| ID | REQ-ROWS |\n| Status | Draft |\n| Owner | me |\n| Priority | Urgent |\n\
                | Tags | x |\n| Tags | x |\n";
    let text = document("Rows", &[item(S1, "Rows", "Requirement", rows, "Body.")]);
    dir.write("docs/rows.mara.md", &text);
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.status, 1);
    assert_eq!(
        run.diagnostics(),
        [
            at(
                "undeclared-key",
                "docs/rows.mara.md",
                line_of(&text, "| Owner")
            ),
            at(
                "invalid-value",
                "docs/rows.mara.md",
                line_of(&text, "| Priority")
            ),
            at(
                "duplicate-row",
                "docs/rows.mara.md",
                line_of(&text, "| Tags | x |\n|") + 1
            ),
        ]
    );
}

/// The inventory's problems are reported with the rest (REQ-REPORT-EVERY-PROBLEM).
#[cfg(unix)]
#[test]
fn reports_the_inventory_problems_with_the_documents_problems() {
    let dir = Dir::project(SCHEMA);
    let text = document(
        "Valid",
        &[item(
            S1,
            "Valid",
            "Requirement",
            &rows("REQ-V", "| Colour | red |\n"),
            "Body.",
        )],
    );
    dir.write("docs/valid.mara.md", &text);
    dir.write("docs/target.md", "# Not a document\n");
    std::os::unix::fs::symlink("target.md", dir.path("docs/link.mara.md")).unwrap();
    dir.write("docs/bad/.gitignore", b"ok\n\xFF\n");
    dir.write("docs/bad/hidden.mara.md", "# Hidden\n");
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.status, 1);
    assert_eq!(
        run.diagnostics(),
        [
            at("unsupported-gitignore", "docs/bad/.gitignore", 2),
            at("symbolic-link", "docs/link.mara.md", 1),
            at(
                "undeclared-key",
                "docs/valid.mara.md",
                line_of(&text, "| Colour")
            ),
        ]
    );
}
