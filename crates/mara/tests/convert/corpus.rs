//! The 0.3.0 project of this repository: converted whole, and stopped and rerun
//! (VER-CONVERSION).

use std::collections::HashMap;
use std::fs;

use mara_engine::document::Item;
use mara_engine::write::label;

use crate::{Project, Step, entries, report};

/// The relation names of the 0.3.0 schema of this repository.
const RELATIONS: [&str; 16] = [
    "derives_from",
    "contributes_to",
    "involves",
    "refines",
    "depends_on",
    "satisfies",
    "justifies",
    "supersedes",
    "verifies",
    "validates",
    "evidences",
    "realizes",
    "implements",
    "checks",
    "affects",
    "mitigates",
];

/// The enum fields of that schema; every other field is a string.
const ENUMS: [&str; 7] = [
    "kind",
    "method",
    "level",
    "treatment",
    "impact",
    "likelihood",
    "result",
];

/// A `:::mara` block as the test reads it: outside fenced code, and closed outside it.
struct Legacy {
    path: String,
    id: String,
    flavour: String,
    metadata: Vec<(String, String)>,
}

impl Legacy {
    fn value(&self, key: &str) -> &str {
        let found = self.metadata.iter().find(|(name, _)| name == key);
        found.map_or("", |(_, value)| value.as_str())
    }
}

fn legacy_items(project: &Project) -> Vec<Legacy> {
    let mut items = Vec::new();
    let mut paths: Vec<String> = fs::read_dir(project.path("docs"))
        .unwrap()
        .map(|entry| format!("docs/{}", entry.unwrap().file_name().to_string_lossy()))
        .collect();
    paths.sort();
    for path in paths {
        let (mut fence, mut current, mut metadata) = (false, None::<Legacy>, false);
        for line in project.read(&path).lines() {
            if line.trim_start().starts_with("```") {
                fence = !fence;
                metadata = false;
                continue;
            }
            if fence {
                continue;
            }
            match current.as_mut() {
                None => {
                    if let Some(rest) = line.strip_prefix(":::mara ") {
                        let words: Vec<&str> = rest.split_whitespace().collect();
                        current = Some(Legacy {
                            path: path.clone(),
                            flavour: words[0].to_owned(),
                            id: words[1].to_owned(),
                            metadata: Vec::new(),
                        });
                        metadata = true;
                    }
                }
                Some(item) => {
                    if line == ":::" {
                        items.push(current.take().unwrap());
                    } else if metadata && line.starts_with(':') {
                        let (key, value) = line[1..].split_once(": ").unwrap();
                        item.metadata
                            .push((key.to_owned(), value.trim().to_owned()));
                    } else {
                        metadata = false;
                    }
                }
            }
        }
    }
    items
}

#[test]
fn converts_the_0_3_0_project_with_every_identity_value_and_relation() {
    let project = Project::corpus();
    assert!(
        project.is_legacy(),
        "every other operation refuses a 0.3.0 project"
    );
    assert_eq!(project.mara(&["item", "list"]).code(), "legacy-project");
    let legacy = legacy_items(&project);
    assert_eq!(legacy.len(), 136, "the items of the 0.3.0 corpus");
    let code = inline_code_relations(&project, &legacy);
    let run = project.convert();

    let mut converted: HashMap<String, (String, Item)> = HashMap::new();
    for item in &legacy {
        if converted.values().any(|(path, _)| *path == item.path) {
            continue;
        }
        for read in project.items(&item.path).items {
            let previous = converted.insert(read.mid.clone(), (item.path.clone(), read));
            assert!(previous.is_none(), "one item per MID");
        }
    }
    assert_eq!(converted.len(), legacy.len(), "no item more or less");
    let mid_of: HashMap<&str, &str> = legacy
        .iter()
        .map(|item| (item.id.as_str(), item.value("mid")))
        .collect();
    for item in &legacy {
        let (path, read) = &converted[item.value("mid")];
        assert_eq!(*path, item.path, "{} stays in its document", item.id);
        assert_eq!(read.value("id"), Some(item.id.as_str()));
        assert_eq!(read.value("type"), Some(label(&item.flavour).as_str()));
        assert_eq!(
            read.title,
            item.value("title").replace('`', ""),
            "{}",
            item.id
        );
        for (key, value) in &item.metadata {
            match key.as_str() {
                "mid" | "title" => {}
                "status" => assert_eq!(read.value("status"), Some(label(value).as_str())),
                relation if RELATIONS.contains(&relation) => {
                    let rows = read.rows.iter().filter(|row| {
                        row.name == relation
                            && row.link.as_ref().and_then(|link| link.fragment.as_deref())
                                == Some(mid_of[value.as_str()])
                    });
                    assert_eq!(rows.count(), 1, "{}: {key}: {value}", item.id);
                }
                field => {
                    let expected = match ENUMS.contains(&field) {
                        true => label(value),
                        false => value.clone(),
                    };
                    let found = read
                        .rows
                        .iter()
                        .any(|row| row.name == field && row.value == expected);
                    assert!(found, "{}: {key}: {value}", item.id);
                }
            }
        }
    }

    let validation = project.mara(&["project", "validate"]);
    assert_eq!(
        (validation.status, &validation.json["errors"]),
        (0, &serde_json::json!(0)),
        "{validation:?}"
    );

    // Everything left out is reported, and the report file holds what the result returns.
    let entries = run.entries();
    assert_eq!(report(&project), entries);
    let count = |text: &str| {
        entries
            .iter()
            .filter(|(_, _, message)| message.contains(text))
            .count()
    };
    assert_eq!(count("the guidance"), 11, "{entries:#?}");
    assert_eq!(count("leads to external addresses"), 2);
    assert_eq!(count("rule files"), 1);
    assert_eq!(count("code language `rust`"), 1);
    assert_eq!(count("project name"), 1);
    assert_eq!(count("[[implemented_by:code:"), code);
    assert_eq!(entries.len(), 11 + 2 + 1 + 1 + 1 + code, "{entries:#?}");

    // A `:::mara` block in a converted document is narrative.
    let path = "docs/format.mara.md";
    let mut text = project.read(path);
    text.push_str("\n:::mara requirement REQ-ADDED-LATER\n:mid: 01M7FX0A000000000000000099\n:title: Added\n\nBody.\n:::\n");
    project.write(path, text);
    assert_eq!(
        project.mara(&["get", "REQ-ADDED-LATER"]).code(),
        "unknown-item"
    );
    assert_eq!(project.mara(&["project", "validate"]).status, 0);
}

/// The inline relations to code in the corpus's documents, outside fenced code.
fn inline_code_relations(project: &Project, legacy: &[Legacy]) -> usize {
    let mut paths: Vec<&str> = legacy.iter().map(|item| item.path.as_str()).collect();
    paths.dedup();
    paths
        .iter()
        .map(|path| {
            let mut fence = false;
            project
                .read(path)
                .lines()
                .filter(|line| {
                    if line.trim_start().starts_with("```") {
                        fence = !fence;
                    }
                    !fence
                })
                .map(|line| line.matches("[[implemented_by:code:").count())
                .sum::<usize>()
        })
        .sum()
}

/// Stopped while it writes the report file, and after each document, then rerun: before the
/// rerun the project is a 0.3.0 project, and after it its files are those of an uninterrupted
/// conversion (VER-CONVERSION, VER-INTERRUPTED-EDITS, ADR-CONVERSION-RERUN).
#[test]
fn a_conversion_of_the_corpus_stopped_anywhere_completes_when_rerun() {
    let base = Project::corpus();
    let reference = base.copy();
    let complete = reference.convert();
    let order = base.order();
    assert_eq!(
        order.first().map(String::as_str),
        Some(".mara/conversion-report.json")
    );
    assert_eq!(
        &order[order.len() - 2..],
        [".mara/schema.toml", ".mara/config.toml"]
    );
    let documents = order.len() - 3;
    assert!(documents >= 18, "{order:?}");
    // Stop at the report file's pause, then at the pause of the file after the report and
    // after each document: the files before it are replaced.
    for stop in 0..=documents + 1 {
        let project = base.copy();
        let killed = project.paused(project.command(&["project", "convert"]), |path, _| {
            match path == order[stop] {
                true => Step::Kill,
                false => Step::Go,
            }
        });
        assert!(killed.is_none(), "stopped at {}", order[stop]);
        assert!(project.is_legacy(), "stopped at {}", order[stop]);
        match stop {
            0 => {
                assert!(!project.exists(".mara/conversion-report.json"));
                for path in &order[1..=documents] {
                    assert_eq!(
                        project.read(path),
                        base.read(path),
                        "{path} is not converted"
                    );
                }
            }
            _ => {
                // The report file holds every omission of the documents already converted.
                assert_eq!(report(&project), entries(&complete.json["entries"]));
                let last = &order[stop - 1];
                assert_eq!(project.read(last), reference.read(last));
                let next = &order[stop];
                if base.exists(next) {
                    assert_eq!(
                        project.read(next),
                        base.read(next),
                        "{next} is not converted"
                    );
                }
            }
        }
        let rerun = project.convert();
        assert_eq!(
            project.tree(),
            reference.tree(),
            "rerun after a stop at {}",
            order[stop]
        );
        assert_eq!(rerun.entries(), complete.entries());
    }
}
