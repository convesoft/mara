//! VER-CURSOR-WALK and VER-PAGE-LIMITS: every read operation on a project with more results than
//! a page holds.

use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;

use crate::{CONFIG, Project, SCHEMA, document, item, link, mid};

const PAGE_BYTES: usize = 65_536;
/// Requirements that each hold `pageword` and satisfy NEED-HUB.
const ITEMS: usize = 130;
/// Types the schema declares besides those of [`SCHEMA`].
const EXTRA_TYPES: usize = 110;

/// `docs/a.mara.md` and `docs/b.mara.md` hold [`ITEMS`] requirements and NEED-HUB; the schema
/// declares [`EXTRA_TYPES`] more types. Titles take `title` characters.
fn many(title: usize) -> Project {
    let project = Project::empty();
    project.write(".mara/config.toml", CONFIG);
    let mut schema = SCHEMA.to_owned();
    for n in 0..EXTRA_TYPES {
        schema.push_str(&format!(
            "\n[types.extra{n}]\nid_prefix = \"X{n}-\"\ndescription = \"Extra {n}.\"\n"
        ));
    }
    project.write(".mara/schema.toml", &schema);
    let hub = item(&mid(0), "Need", "NEED-HUB", "Hub", &[], "The hub.");
    let mut halves = [vec![hub], Vec::new()];
    for n in 1..=ITEMS as u32 {
        let path = if n % 2 == 0 { "a.mara.md" } else { "" };
        let title = format!("{n:04} {}", "t".repeat(title.saturating_sub(5)));
        halves[(n % 2) as usize].push(item(
            &mid(n),
            "Requirement",
            &format!("REQ-N{n}"),
            &title,
            &[("Satisfies", &link("NEED-HUB", path, &mid(0)))],
            "Holds pageword.",
        ));
    }
    project.write("docs/a.mara.md", &document(&halves[0]));
    project.write("docs/b.mara.md", &document(&halves[1]));
    project
}

/// Each list read, the name of its records and how many there are.
const READS: [(&[&str], &str, usize); 4] = [
    (&["search", "pageword"], "results", ITEMS),
    (&["item", "list"], "items", ITEMS + 1),
    (&["relation", "list", "NEED-HUB"], "connections", ITEMS),
    (&["schema", "list"], "declarations", 3 + EXTRA_TYPES + 2),
];

/// Every page of `args`, from the first, following each cursor; each page checked against the
/// limits.
fn walk(
    project: &Project,
    args: &[&str],
    records: &str,
    limit: usize,
    extra: &[&str],
) -> Vec<Value> {
    let mut pages = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut all: Vec<&str> = args.to_vec();
        all.extend_from_slice(&["--format", "json"]);
        all.extend_from_slice(extra);
        if let Some(cursor) = &cursor {
            all.extend_from_slice(&["--cursor", cursor]);
        }
        let run = project.run(&all).ok();
        assert!(
            run.stdout.trim_end().len() <= PAGE_BYTES,
            "{args:?}: {} bytes",
            run.stdout.len()
        );
        let page = run.json();
        let count = page[records].as_array().unwrap().len();
        assert!((1..=limit).contains(&count), "{args:?}: {count} records");
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        pages.push(page);
        if cursor.is_none() {
            return pages;
        }
    }
}

#[test]
fn following_every_cursor_yields_each_record_once() {
    let project = many(10);
    for (args, records, total) in READS {
        for (limit, extra) in [(20, &[][..]), (7, &["--limit", "7"][..])] {
            let pages = walk(&project, args, records, limit, extra);
            let all: Vec<String> = pages
                .iter()
                .flat_map(|page| {
                    page[records]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(Value::to_string)
                })
                .collect();
            let distinct: BTreeSet<&String> = all.iter().collect();
            assert_eq!(
                (all.len(), distinct.len()),
                (total, total),
                "{args:?} by {limit}"
            );
            assert_eq!(pages.len(), total.div_ceil(limit), "{args:?} by {limit}");
            assert_eq!(pages.last().unwrap()["next_cursor"], Value::Null);
        }
    }
}

fn first_cursor(project: &Project, args: &[&str]) -> String {
    let mut all = args.to_vec();
    all.extend_from_slice(&["--format", "json"]);
    let page = project.run(&all).ok().json();
    page["next_cursor"]
        .as_str()
        .expect("more than a page")
        .to_owned()
}

fn with_cursor(project: &Project, args: &[&str], cursor: &str) -> crate::Run {
    let mut all = args.to_vec();
    all.extend_from_slice(&["--cursor", cursor]);
    project.run(&all)
}

#[test]
fn a_cursor_goes_stale_only_when_what_its_result_reads_changes() {
    for (args, _, _) in READS {
        let project = many(10);
        let cursor = first_cursor(&project, args);
        project.write("README.md", "# Not an input\n");
        project.write("docs/notes.txt", "Not a document.\n");
        with_cursor(&project, args, &cursor).ok();
        let text = project.read("docs/a.mara.md");
        project.write("docs/a.mara.md", &format!("{text}\n---\n\nAn edit.\n"));
        let after_edit = with_cursor(&project, args, &cursor);
        let schema = args[0] == "schema";
        if schema {
            // A schema read reads no document.
            after_edit.ok();
            let text = project.read(".mara/schema.toml");
            project.write(
                ".mara/schema.toml",
                &format!("{text}\n[types.late]\nid_prefix = \"L-\"\ndescription = \"Late.\"\n"),
            );
            let run = with_cursor(&project, args, &cursor).status(1);
            assert!(
                run.stderr.contains("error[stale-cursor]"),
                "{args:?}: {}",
                run.stderr
            );
        } else {
            let run = after_edit.status(1);
            assert!(
                run.stderr.contains("error[stale-cursor]"),
                "{args:?}: {}",
                run.stderr
            );
            assert_eq!(run.stdout, "");
        }
        // A cursor of another request belongs to another result.
        let other = with_cursor(&project, &["item", "list", "--type", "need"], &cursor).status(1);
        assert!(
            other.stderr.contains("error[stale-cursor]"),
            "{}",
            other.stderr
        );
        let garbage = with_cursor(&project, args, "not-a-cursor").status(2);
        assert!(
            garbage.stderr.contains("error[invalid-argument]"),
            "{}",
            garbage.stderr
        );
    }
}

#[test]
fn get_returns_a_large_item_in_portions_that_concatenate_to_it() {
    let project = Project::new();
    let long_line = "word \"quoted\" ".repeat(PAGE_BYTES / 10);
    let body = format!(
        "{long_line}\nA short line.\n{}Last line.",
        "A line of the body.\n".repeat(5000)
    );
    project.write(
        "docs/a.mara.md",
        &item(
            &mid(1),
            "Requirement",
            "REQ-BIG",
            "Big",
            &[("Owner", "Ann")],
            &body,
        ),
    );
    let (mut rows, mut content, mut cursor, mut pages) =
        (Vec::new(), String::new(), None::<String>, 0);
    loop {
        let mut args = vec!["get", "REQ-BIG", "--format", "json"];
        if let Some(cursor) = &cursor {
            args.extend_from_slice(&["--cursor", cursor]);
        }
        let run = project.run(&args).ok();
        assert!(run.stdout.trim_end().len() <= PAGE_BYTES);
        let page = run.json();
        let page_rows = page["rows"].as_array().unwrap();
        assert!(
            page_rows.is_empty() || content.is_empty(),
            "rows come before the content"
        );
        rows.extend(
            page_rows
                .iter()
                .map(|row| row["key"].as_str().unwrap().to_owned()),
        );
        content.push_str(page["content"].as_str().unwrap());
        pages += 1;
        match page["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    assert!(pages >= 3, "{pages} pages");
    assert_eq!(rows, ["Type", "ID", "Status", "Owner"]);
    assert_eq!(content, body);
}

#[test]
fn a_record_larger_than_a_page_fails_with_a_diagnostic_naming_it() {
    let project = Project::new();
    let huge = format!("Huge {}", "h".repeat(PAGE_BYTES));
    project.write(
        "docs/a.mara.md",
        &document(&[
            item(
                &mid(1),
                "Requirement",
                "REQ-HUGE",
                &huge,
                &[],
                "Holds hugeword.",
            ),
            item(
                &mid(2),
                "Requirement",
                "REQ-LINKS",
                "Links",
                &[("Refines", &link("REQ-HUGE", "", &mid(1)))],
                "Body.",
            ),
        ]),
    );
    let link_line = 1 + project
        .read("docs/a.mara.md")
        .lines()
        .position(|l| l.starts_with("| Refines"))
        .unwrap();
    let cases: [(&[&str], String); 3] = [
        (
            &["item", "list"],
            "docs/a.mara.md:1: error[record-too-large]".to_owned(),
        ),
        (
            &["search", "hugeword"],
            "docs/a.mara.md:1: error[record-too-large]".to_owned(),
        ),
        (
            &["relation", "list", "REQ-LINKS"],
            format!("docs/a.mara.md:{link_line}: error[record-too-large]"),
        ),
    ];
    for (args, expected) in cases {
        let run = project.run(args).status(1);
        assert!(run.stderr.contains(&expected), "{args:?}: {}", run.stderr);
        assert_eq!(run.stdout, "");
    }
    let text = project.read(".mara/schema.toml");
    project.write(
        ".mara/schema.toml",
        &text.replace(
            "description = \"A design.\"",
            &format!("description = \"{huge}\""),
        ),
    );
    let run = project.run(&["schema", "list"]).status(1);
    assert!(
        run.stderr
            .contains(".mara/schema.toml:1: error[record-too-large]"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_page_holds_twenty_records_or_the_number_asked_for() {
    let project = many(10);
    for (args, records, _) in READS {
        let count = |extra: &[&str]| {
            let mut all = args.to_vec();
            all.extend_from_slice(&["--format", "json"]);
            all.extend_from_slice(extra);
            project.run(&all).ok().json()[records]
                .as_array()
                .unwrap()
                .len()
        };
        assert_eq!(count(&[]), 20, "{args:?}");
        assert_eq!(count(&["--limit", "1"]), 1, "{args:?}");
        assert_eq!(count(&["--limit", "100"]), 100, "{args:?}");
        for limit in ["0", "101", "many"] {
            let mut all = args.to_vec();
            all.extend_from_slice(&["--limit", limit]);
            let run = project.run(&all).status(2);
            assert!(run.stderr.contains("--limit"), "{}", run.stderr);
            assert_eq!(run.count("file_reads"), 0, "rejected before anything runs");
        }
    }
}

#[test]
fn no_page_exceeds_the_byte_limit() {
    let project = many(2000);
    for (args, records, total) in READS {
        let pages = walk(&project, args, records, 100, &["--limit", "100"]);
        let count: usize = pages
            .iter()
            .map(|page| page[records].as_array().unwrap().len())
            .sum();
        assert_eq!(count, total, "{args:?}");
        if args[0] != "schema" {
            assert!(
                pages.len() > total.div_ceil(100),
                "{args:?}: the bytes, not the records, end a page"
            );
        }
    }
}

/// A project with REQ-1 and REQ-2, and `links` symbolic links with long names that the include
/// pattern matches as documents: one `symbolic-link` diagnostic each.
fn with_inventory_problems(links: usize) -> Project {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &document(&[
            item(&mid(1), "Requirement", "REQ-1", "One", &[], "Holds word."),
            item(
                &mid(2),
                "Requirement",
                "REQ-2",
                "Two",
                &[("Refines", &link("REQ-1", "", &mid(1)))],
                "Holds word.",
            ),
        ]),
    );
    fs::create_dir_all(project.path("links")).unwrap();
    for n in 0..links {
        let name = format!("links/{n:04}-{}.mara.md", "long-name-".repeat(20));
        std::os::unix::fs::symlink("../docs/a.mara.md", project.path(&name)).unwrap();
    }
    project
}

#[test]
fn many_inventory_problems_keep_every_page_within_the_byte_limit() {
    let project = with_inventory_problems(400);
    let reads: [&[&str]; 5] = [
        &["search", "nothing"],
        &["search", "word"],
        &["relation", "list", "REQ-1"],
        &["item", "list"],
        &["get", "REQ-1"],
    ];
    for args in reads {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--format", "json"]);
        let run = project.run(&all).ok();
        assert!(
            run.stdout.trim_end().len() <= PAGE_BYTES,
            "{args:?}: {} bytes",
            run.stdout.len()
        );
        let page = run.json();
        assert_eq!(page["diagnostics_total"], 400, "{args:?}");
        let shown = page["diagnostics"].as_array().unwrap();
        assert!(
            !shown.is_empty() && shown.len() < 400,
            "{args:?}: {} shown",
            shown.len()
        );
        assert_eq!(shown[0]["code"], "symbolic-link");
        let human = project.run(args).ok();
        assert!(
            human
                .stdout
                .contains(&format!("and {} more problems", 400 - shown.len()))
                && human.stdout.contains("mara project validate"),
            "{args:?}: {}",
            human.stdout
        );
    }
}

#[test]
fn a_document_read_s_cursor_goes_stale_when_its_inventory_problems_change() {
    let reads: [&[&str]; 3] = [
        &["item", "list", "--limit", "1"],
        &["search", "word", "--limit", "1"],
        &["relation", "list", "REQ-1", "--limit", "1"],
    ];
    for args in reads {
        let project = with_inventory_problems(0);
        // Each item mentions REQ-1, so that REQ-1 has more connections than a page of one.
        let text = project.read("docs/a.mara.md");
        let mention = format!("Holds word, see {}.\n", link("one", "", &mid(1)));
        project.write("docs/a.mara.md", &text.replace("Holds word.\n", &mention));
        let cursor = first_cursor(&project, args);
        std::os::unix::fs::symlink("missing.mara.md", project.path("docs/new.mara.md")).unwrap();
        let run = with_cursor(&project, args, &cursor).status(1);
        assert!(
            run.stderr.contains("error[stale-cursor]"),
            "{args:?}: {}",
            run.stderr
        );
    }
}
