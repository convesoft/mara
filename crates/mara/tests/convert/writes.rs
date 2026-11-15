//! The `project convert` cases of VER-PRESERVED-BYTES and VER-REFUSED-EDITS: the bytes a
//! conversion keeps, and the refusals at the pause points of the write procedure.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::mapping::{MID_A, fixture};
use crate::{Project, Step, changed};

const MID_C: &str = "01M7FX0C000000000000000001";
const MID_L: &str = "01M7FX0C000000000000000002";

const SCHEMA: &str = "format_version: 3\nflavours:\n  requirement:\n    description: An obligation.\n    use_when: [x]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: REQ-\n    body: required\n    fields:\n      status: {type: enum, values: [draft]}\nrelations:\n  depends_on:\n    description: Needs.\n    source: [requirement]\n    target: [requirement]\n    inverse: required_by\n";

const CONFIG: &str = "format_version = 3\n\n[project]\nname = \"bytes\"\nschema = \".mara/schema.yaml\"\n\n[content]\ninclude = [\"docs/*.mara.md\"]\n";

#[test]
fn keeps_the_narrative_line_endings_and_modes_and_every_other_file() {
    let project = Project::empty();
    project.write(".mara/project.toml", CONFIG);
    project.write(".mara/schema.yaml", SCHEMA);
    project.write("README.md", "# Readme\n");
    project.write(".gitignore", "target/\n");
    project.write("docs/plain.mara.md", "# Plain\n\nNo 0.3.0 syntax.\n");
    let head = "\u{feff}# CRLF document  \r\n\r\nSome *emphasis* and _underscores_ with trailing spaces   \r\nnaming ";
    let crlf = format!(
        "{head}[[REQ-C]].\r\n\r\n:::mara requirement REQ-C\r\n:mid: {MID_C}\r\n:title: CRLF item\r\n:status: draft\r\n\r\nBody *kept*  \r\n:::\r\n\r\nClosing _narrative_.\r\n"
    );
    project.write("docs/crlf.mara.md", &crlf);
    let front = "---\ntitle: front matter\n---\n# LF document\n";
    let lf = format!(
        "{front}\n:::mara requirement REQ-L\n:mid: {MID_L}\n:title: LF item\n:status: draft\n:depends_on: REQ-C\n\nBody with [[REQ-C]] and `[[REQ-C]]`.\n:::"
    );
    project.write("docs/lf.mara.md", &lf);
    let set_mode = |path: &str, mode| {
        fs::set_permissions(project.path(path), fs::Permissions::from_mode(mode)).unwrap()
    };
    set_mode("docs/crlf.mara.md", 0o600);
    set_mode("docs/lf.mara.md", 0o644);
    let before = project.stamped();
    project.convert();
    let after = project.stamped();

    let crlf = project.read("docs/crlf.mara.md");
    assert!(crlf.starts_with(&format!("{head}[REQ-C](#{MID_C}).\r\n\r\n---\r\n\r\n<a id=\"{MID_C}\"></a>\r\n\r\n## CRLF item\r\n\r\n| Type | Requirement |\r\n")), "{crlf:?}");
    assert!(
        crlf.ends_with(
            "| Status | Draft |\r\n\r\nBody *kept*  \r\n\r\n---\r\n\r\nClosing _narrative_.\r\n"
        ),
        "{crlf:?}"
    );
    let lf = project.read("docs/lf.mara.md");
    let link = format!("[REQ-C](crlf.mara.md#{MID_C})");
    assert_eq!(
        lf,
        format!(
            "{front}\n---\n\n<a id=\"{MID_L}\"></a>\n\n## LF item\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-L |\n| Status | Draft |\n| Depends on | {link} |\n\nBody with {link} and `[[REQ-C]]`.\n"
        )
    );
    for (path, mode) in [("docs/crlf.mara.md", 0o600), ("docs/lf.mara.md", 0o644)] {
        assert_eq!(after[path].2, mode, "{path} keeps its mode");
    }
    for path in [
        "README.md",
        ".gitignore",
        "docs/plain.mara.md",
        ".mara/project.toml",
        ".mara/schema.yaml",
    ] {
        assert_eq!(
            after[path], before[path],
            "{path} keeps its bytes and mtime"
        );
    }
    assert_eq!(project.mara(&["project", "validate"]).status, 0);
}

/// A file the conversion writes, changed while it waits at the pause before step 2: the first
/// file, created, and a later one. Then its first file created at the pause before its second
/// check. Each is refused with nothing replaced and no temporary file left.
#[test]
fn refuses_a_conversion_whose_files_change_before_it_writes() {
    let report = ".mara/conversion-report.json";
    let document = "docs/requirements.mara.md";
    for (pause, path) in [(0, report), (0, document), (1, report)] {
        let project = fixture();
        let before = project.tree();
        let run = project.paused(project.command(&["project", "convert"]), |paused, seen| {
            if seen == pause {
                assert_eq!(paused, if pause == 0 { "" } else { report });
                project.write(path, "changed by the test\n");
            }
            Step::Go
        });
        let run = run.unwrap();
        assert_eq!((run.status, run.code()), (1, "changed-file"), "{run:?}");
        assert_eq!(
            run.diagnostics(),
            [(path.to_owned(), 1, "changed-file".to_owned())]
        );
        assert_eq!(
            changed(&before, &project.tree()),
            [path],
            "only the test's change"
        );
        assert_eq!(project.read(path), "changed by the test\n");
        assert_eq!(project.temporary_files(), Vec::<String>::new());
        assert!(project.is_legacy());
    }
}

/// A document already in the item format that holds each problem `project fix` repairs
/// (DES-FIX-REPAIRS), beside the 0.3.0 documents of the fixture: the conversion leaves those
/// problems as they were, byte for byte (the last row of VER-FIX-CASES).
#[test]
fn leaves_the_problems_that_fix_repairs_as_they_were() {
    let project = fixture();
    let mid = |n: u8| format!("01M7FX0H0000000000000000{n:02}");
    let broken = format!(
        "# Broken\n\n---\n\n## No anchor\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-NO-ANCHOR |\n\
         | Status | Draft |\n\nText.\n\n---\n\n<a id=\"{m1}\"></a>\n\n## Stale\n\n\
         | Type | Requirement |\n| --- | --- |\n| ID | REQ-STALE |\n| Status | Draft |\n\
         | Depends on | REQ-A |\n| Depends on | [REQ-OLD](old.mara.md#{MID_A}) |\n\n\
         Mentions [the first](old.mara.md#{MID_A}).\n\n---\n\n<a id=\"{m2}\"></a>\n\
         ## No blank line\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-NO-BLANK |\n\
         | Status | Draft |\n\nText.\n\n<a id=\"{m3}\"></a>\n\n## No separator\n\n\
         | Type | Requirement |\n| --- | --- |\n| ID | REQ-NO-SEPARATOR |\n| Status | Draft |\n\n\
         Text.\n\n---\n\n<a id=\"{m4}\"></a>\n\nSetext\n------\n\n| Type | Requirement |\n\
         | --- | --- |\n| ID | REQ-SETEXT |\n| Status | Draft |\n\nText.\n",
        m1 = mid(1),
        m2 = mid(2),
        m3 = mid(3),
        m4 = mid(4),
    );
    project.write("docs/broken.mara.md", &broken);
    project.convert();
    assert_eq!(project.read("docs/broken.mara.md"), broken);
    let validation = project.mara(&["project", "validate", "--path", "docs/broken.mara.md"]);
    let mut codes: Vec<&str> = validation.json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(
        codes,
        [
            "bare-id-value",
            "incomplete-item",
            "merged-anchor",
            "missing-separator",
            "setext-heading",
            "stale-link-path",
            "stale-link-text"
        ],
        "{validation:?}"
    );
}
