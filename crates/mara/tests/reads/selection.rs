//! Project selection and loading through the read operations: VER-NESTED-PROJECTS, the codes of
//! DES-PROJECT-CONFIGURATION, and the refusal of a project whose configuration or schema has an
//! error.

use std::fs;

use crate::{CONFIG, Project, SCHEMA, item, mid, names, run_in};

/// Every read operation, run on a project whose only item is REQ-<name>, which holds `word`.
fn reads(name: &str) -> Vec<Vec<String>> {
    let id = format!("REQ-{name}");
    [
        vec!["search", "word"],
        vec!["get", id.as_str()],
        vec!["relation", "list", id.as_str()],
        vec!["item", "list"],
        vec!["schema", "list"],
        vec!["schema", "get", "requirement"],
    ]
    .into_iter()
    .map(|args| args.into_iter().map(str::to_owned).collect())
    .collect()
}

fn args(read: &[String]) -> Vec<&str> {
    read.iter().map(String::as_str).collect()
}

/// A project at `relative` whose only item is REQ-<name>.
fn add_project(project: &Project, relative: &str, name: &str, n: u32) {
    let at = |path: &str| {
        if relative.is_empty() {
            path.to_owned()
        } else {
            format!("{relative}/{path}")
        }
    };
    project.write(&at(".mara/config.toml"), CONFIG);
    project.write(&at(".mara/schema.toml"), SCHEMA);
    project.write(
        &at("docs/d.mara.md"),
        &item(
            &mid(n),
            "Requirement",
            &format!("REQ-{name}"),
            name,
            &[],
            "Holds word.",
        ),
    );
}

/// An outer project at the root and an inner one at `inner/`; the outer include pattern matches
/// the inner documents.
fn nested() -> Project {
    let project = Project::empty();
    add_project(&project, "", "OUTER", 1);
    add_project(&project, "inner", "INNER", 2);
    project
}

/// Runs every read in `directory` with `extra` arguments and requires that it reads only the
/// project whose item is REQ-<name>, with `documents` documents: the IDs it returns and the files
/// it reads.
fn uses_only(project: &Project, directory: &str, extra: &[&str], name: &str, documents: u64) {
    for read in reads(name) {
        let mut all = args(&read);
        all.extend_from_slice(extra);
        all.extend_from_slice(&["--format", "json"]);
        let run = project.run_in(directory, &all).ok();
        let schema = read[0] == "schema";
        let expected = if schema { 2 } else { 2 + documents };
        assert_eq!(
            run.count("file_reads"),
            expected,
            "{all:?} in {directory}: {run:#?}"
        );
        let page = run.json();
        let found = match read[0].as_str() {
            "search" => names(&page["results"]),
            "item" => names(&page["items"]),
            "get" | "relation" => vec![page["node"]["id"].as_str().unwrap().to_owned()],
            _ => continue,
        };
        assert!(
            found
                .iter()
                .all(|id| id.starts_with(&format!("REQ-{name}"))),
            "{all:?} in {directory}: {found:?}"
        );
        assert!(!found.is_empty(), "{all:?}");
    }
}

#[test]
fn each_read_uses_only_the_selected_project_among_nested_ones() {
    let project = nested();
    uses_only(&project, "inner/docs", &[], "INNER", 1);
    uses_only(&project, "", &[], "OUTER", 1);
    uses_only(&project, "docs", &[], "OUTER", 1);
    uses_only(&project, "inner/docs", &["--project", "../.."], "OUTER", 1);
    uses_only(&project, "", &["--project", "inner"], "INNER", 1);
}

#[test]
fn a_legacy_inner_project_is_skipped_and_a_stray_config_toml_is_not_a_project() {
    let project = nested();
    fs::rename(
        project.path("inner/.mara/config.toml"),
        project.path("inner/.mara/project.toml"),
    )
    .unwrap();
    uses_only(&project, "", &[], "OUTER", 1);
    project.write("sub/config.toml", CONFIG);
    project.write(
        "sub/s.mara.md",
        &item(
            &mid(3),
            "Requirement",
            "REQ-OUTER-SUB",
            "Sub",
            &[],
            "Holds word.",
        ),
    );
    uses_only(&project, "", &[], "OUTER", 2);
    let listed = project
        .run(&["item", "list", "--format", "json"])
        .ok()
        .json();
    assert_eq!(names(&listed["items"]), ["REQ-OUTER", "REQ-OUTER-SUB"]);
}

#[test]
fn selection_problems_refuse_every_read() {
    let outside = Project::empty();
    let project = Project::empty();
    add_project(&project, "legacy", "LEGACY", 1);
    fs::rename(
        project.path("legacy/.mara/config.toml"),
        project.path("legacy/.mara/project.toml"),
    )
    .unwrap();
    add_project(&project, "linked", "LINKED", 2);
    fs::rename(project.path("linked/.mara"), project.path("linked/real")).unwrap();
    std::os::unix::fs::symlink("real", project.path("linked/.mara")).unwrap();
    fs::create_dir_all(project.path("plain")).unwrap();
    for read in reads("ANY") {
        let read = args(&read);
        let cases = [
            (outside.root(), vec![], "no-project"),
            (project.path("legacy/docs"), vec![], "legacy-project"),
            (project.path("linked/docs"), vec![], "symbolic-link"),
            (project.root(), vec!["--project", "plain"], "not-a-project"),
            (project.root(), vec!["--project", "linked"], "symbolic-link"),
        ];
        for (directory, extra, code) in cases {
            let mut all = read.clone();
            all.extend_from_slice(&extra);
            let run = run_in(&directory, &all).status(1);
            assert!(
                run.stderr.contains(&format!("error[{code}]")),
                "{all:?}: {}",
                run.stderr
            );
            assert_eq!(
                (
                    run.stdout.as_str(),
                    run.count("parses"),
                    run.count("file_reads")
                ),
                ("", 0, 0),
                "{all:?}"
            );
            if code == "legacy-project" {
                assert!(
                    run.stderr.contains("mara project convert"),
                    "{}",
                    run.stderr
                );
            }
        }
    }
}

#[test]
fn a_configuration_or_schema_error_refuses_every_read() {
    let broken_config = format!("{CONFIG}unknown_setting = true\n");
    let broken_schema = SCHEMA.replace("id_prefix = \"NEED-\"", "id_prefix = \"need\"");
    for (file, contents, code) in [
        (".mara/config.toml", broken_config.as_str(), "unknown-key"),
        (".mara/schema.toml", broken_schema.as_str(), "invalid-value"),
    ] {
        let project = Project::empty();
        add_project(&project, "", "ANY", 1);
        project.write(file, contents);
        for read in reads("ANY") {
            let read = args(&read);
            let run = project.run(&read).status(1);
            assert!(
                run.stderr.contains(&format!("{file}:"))
                    && run.stderr.contains(&format!("error[{code}]")),
                "{read:?}: {}",
                run.stderr
            );
            assert_eq!(
                (run.stdout.as_str(), run.count("parses")),
                ("", 0),
                "{read:?}"
            );
        }
    }
}

#[test]
fn an_undeclared_argument_is_rejected_before_anything_runs() {
    let project = Project::empty();
    add_project(&project, "", "ANY", 1);
    for read in reads("ANY") {
        for extra in [
            &["--no-such-option"][..],
            &["--relation"],
            &["extra", "argument"],
        ] {
            let mut all = args(&read);
            all.extend_from_slice(extra);
            let run = project.run(&all).status(2);
            assert!(run.stderr.starts_with("mara: "), "{all:?}: {}", run.stderr);
            assert_eq!(run.count("file_reads"), 0, "{all:?}");
        }
    }
    let run = project.run(&["get", "REQ-ANY", "--limit", "5"]).status(2);
    assert!(
        run.stderr
            .contains("error[invalid-argument]: unknown option `--limit` of `get`"),
        "{}",
        run.stderr
    );
}

#[test]
fn every_read_of_documents_carries_the_inventory_s_problems() {
    let project = Project::empty();
    add_project(&project, "", "ANY", 1);
    project.write("elsewhere/x.md", "Not a document.\n");
    std::os::unix::fs::symlink("../elsewhere/x.md", project.path("docs/linked.mara.md")).unwrap();
    for read in reads("ANY") {
        let mut all = args(&read);
        all.extend_from_slice(&["--format", "json"]);
        let page = project.run(&all).ok().json();
        let codes: Vec<&str> = page["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["code"].as_str().unwrap())
            .collect();
        if read[0] == "schema" {
            assert_eq!(codes, [] as [&str; 0], "{all:?}");
        } else {
            assert_eq!(codes, ["symbolic-link"], "{all:?}");
            assert_eq!(page["diagnostics"][0]["path"], "docs/linked.mara.md");
        }
        // The human form prints them with the result, on stdout.
        let human = project.run(&args(&read)).ok();
        assert_eq!(
            human
                .stdout
                .contains("docs/linked.mara.md:1: error[symbolic-link]"),
            read[0] != "schema",
            "{read:?}"
        );
    }
}
