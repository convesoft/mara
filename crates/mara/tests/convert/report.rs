//! The report file: refused when it cannot be kept, and kept by a rerun (VER-CONVERSION,
//! ADR-CONVERSION-REPORT-FILE).

use std::os::unix::fs::symlink;

use crate::mapping::{DESIGNS, REQUIREMENTS, fixture};
use crate::{Project, Step, report};

const REPORT: &str = ".mara/conversion-report.json";

/// Converts `project` until the pause of `path`, then kills the process.
fn stop_at(project: &Project, path: &str) {
    let killed = project.paused(
        project.command(&["project", "convert"]),
        |paused, _| match paused == path {
            true => Step::Kill,
            false => Step::Go,
        },
    );
    assert!(killed.is_none(), "stopped at {path}");
    assert!(project.is_legacy());
}

#[test]
fn refuses_a_report_file_it_cannot_keep_and_writes_nothing() {
    type Make = fn(&Project);
    let cases: [(&str, Make); 4] = [
        ("symbolic-link", |project| {
            project.write(
                ".mara/elsewhere.json",
                "{\"format_version\":1,\"entries\":[]}",
            );
            symlink("elsewhere.json", project.path(REPORT)).unwrap();
        }),
        ("invalid-value", |project| project.write(REPORT, "not JSON")),
        ("invalid-value", |project| {
            project.write(REPORT, "{\"format_version\":1,\"entries\":{}}")
        }),
        ("unsupported-format-version", |project| {
            project.write(REPORT, "{\"format_version\":2,\"entries\":[]}")
        }),
    ];
    for (code, make) in cases {
        let project = fixture();
        make(&project);
        let before = project.stamped();
        let refused = project.mara(&["project", "convert"]);
        assert_eq!((refused.status, refused.code()), (1, code), "{refused:?}");
        assert_eq!(
            refused.diagnostics(),
            [(REPORT.to_owned(), 1, code.to_owned())]
        );
        assert_eq!(project.stamped(), before, "nothing written");
    }
}

/// A mention added by hand to a converted document from which a symmetric relation row was
/// dropped does not drop or repeat that omission.
#[test]
fn a_rerun_keeps_the_omission_of_a_converted_document_once() {
    let project = fixture();
    let designs = DESIGNS.replace(
        ":satisfies: REQ-A\n",
        ":satisfies: REQ-A\n:related_to: REQ-B\n",
    );
    project.write("docs/design/designs.mara.md", &designs);
    let omission = |entries: &[(String, u64, String)]| {
        entries
            .iter()
            .filter(|(path, _, message)| {
                path == "docs/design/designs.mara.md"
                    && message.contains("`:related_to: REQ-B` is left out")
            })
            .count()
    };
    let order = project.order();
    assert_eq!(order[1], "docs/design/designs.mara.md");
    stop_at(&project, &order[2]);
    assert_eq!(omission(&report(&project)), 1);
    let converted = project.read("docs/design/designs.mara.md");
    assert!(converted.contains("| Satisfies |"), "{converted}");
    let edited = converted.replace("Satisfies [REQ-A]", "Satisfies [[REQ-B]] and [REQ-A]");
    let edited = edited.replace("# Designs\n", "# Designs\n\nSee [[REQ-A]].\n");
    project.write("docs/design/designs.mara.md", edited);
    let rerun = project.convert();
    assert_eq!(omission(&rerun.entries()), 1, "{:#?}", rerun.entries());
    assert_eq!(omission(&report(&project)), 1);
    assert_eq!(report(&project), rerun.entries());
}

/// An unconverted document left unchanged gives each entry once; one edited so that an omission
/// moves keeps the recorded entry and adds the one at its new line.
#[test]
fn a_rerun_keeps_every_recorded_entry_and_adds_the_moved_ones() {
    let project = fixture();
    let order = project.order();
    stop_at(&project, &order[1]);
    let recorded = report(&project);
    project.write(
        "docs/requirements.mara.md",
        REQUIREMENTS.replace("# Requirements\n", "# Requirements\n\nA new line.\n"),
    );
    let rerun = project.convert();
    let entries = rerun.entries();
    assert_eq!(report(&project), entries);
    for entry in &recorded {
        assert_eq!(
            entries.iter().filter(|other| *other == entry).count(),
            1,
            "{entry:?}"
        );
    }
    let message = "`:related_to: DES-B` is left out: relation `related_to` is symmetric, and a 0.4.0 relation has a direction";
    let at = |line| {
        entries
            .iter()
            .filter(|e| e.0 == "docs/requirements.mara.md" && e.1 == line && e.2 == message)
            .count()
    };
    assert_eq!((at(16), at(18)), (1, 1), "{entries:#?}");
    // Every entry of the unchanged document once, and of the edited one at its old and new lines.
    let designs = |entries: &[(String, u64, String)]| {
        entries
            .iter()
            .filter(|e| e.0 == "docs/design/designs.mara.md")
            .count()
    };
    assert_eq!(designs(&entries), designs(&recorded));
    let moved = recorded
        .iter()
        .filter(|e| e.0 == "docs/requirements.mara.md")
        .count();
    let now = entries
        .iter()
        .filter(|e| e.0 == "docs/requirements.mara.md")
        .count();
    assert_eq!(now, 2 * moved, "{entries:#?}");
}

/// A MID that the conversion generates is in the report file before any document is written,
/// so that a rerun after a stop gives the item the same MID and the links that converted
/// documents already hold keep resolving (review finding 1).
#[test]
fn a_rerun_gives_an_item_the_mid_that_the_stopped_run_gave_it() {
    let project = Project::empty();
    project.write(
        ".mara/project.toml",
        "format_version = 3\n\n[project]\nname = \"mids\"\nschema = \".mara/schema.yaml\"\n\n[content]\ninclude = [\"docs/*.mara.md\"]\n",
    );
    project.write(
        ".mara/schema.yaml",
        "format_version: 3\nflavours:\n  requirement:\n    description: An obligation.\n    use_when: [x]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: REQ-\n    body: required\n    fields:\n      status: {type: enum, values: [draft]}\nrelations:\n  depends_on:\n    description: Needs.\n    source: [requirement]\n    target: [requirement]\n    inverse: required_by\n",
    );
    project.write(
        "docs/a.mara.md",
        "# A\n\n:::mara requirement REQ-A\n:mid: 01M7FX0G000000000000000001\n:title: A\n:status: draft\n:depends_on: REQ-B\n\nNames [[REQ-B]].\n:::\n",
    );
    project.write(
        "docs/b.mara.md",
        "# B\n\n:::mara requirement REQ-B\n:title: B\n:status: draft\n\nBody.\n:::\n",
    );
    let reference = project.copy();
    reference.convert();
    stop_at(&project, "docs/b.mara.md");
    let generated = project.items("docs/a.mara.md").items[0].rows[3]
        .link
        .clone()
        .and_then(|link| link.fragment)
        .expect("the converted A links to B");
    project.convert();
    assert_eq!(project.items("docs/b.mara.md").items[0].mid, generated);
    let validation = project.mara(&["project", "validate"]);
    assert_eq!(
        (validation.status, &validation.json["errors"]),
        (0, &serde_json::json!(0)),
        "{validation:?}"
    );
    // Apart from the generated MID, the files are those of an uninterrupted conversion.
    let generic = |project: &Project| {
        let mid = project.items("docs/b.mara.md").items[0].mid.clone();
        project
            .tree()
            .into_iter()
            .map(|(path, bytes)| (path, String::from_utf8(bytes).unwrap().replace(&mid, "MID")))
            .collect::<Vec<_>>()
    };
    assert_eq!(generic(&project), generic(&reference));
}
