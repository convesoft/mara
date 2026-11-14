//! VER-SCHEMA-INSPECTION: `schema list` and `schema get` against the schema file.

use serde_json::{Value, json};

use crate::{CONFIG, Project};

const SCHEMA: &str = r#"format_version = 1
statuses = ["open", "closed"]

[types.finding]
id_prefix = "FND-"
description = "Something found."

[types.finding.fields.severity]
kind = "enum"
values = ["low", "high"]
required = true

[types.finding.fields.tags]
kind = "text"
repeatable = true

[types.fix]
id_prefix = "FIX-"
description = "A change that removes a finding."

[relations.resolves]
inverse = "resolved_by"
description = "The fix removes the finding."
targets = { fix = ["finding"], code = ["finding", "fix"] }

[relations.replaces]
inverse = "replaced_by"
description = "The source replaces the target."
targets = { finding = ["finding"], fix = ["fix"] }
same_type = true
acyclic = true
"#;

fn project() -> Project {
    let project = Project::empty();
    project.write(".mara/config.toml", CONFIG);
    project.write(".mara/schema.toml", SCHEMA);
    project
}

fn declarations(project: &Project, args: &[&str]) -> Value {
    let mut all = vec!["schema"];
    all.extend_from_slice(args);
    all.extend_from_slice(&["--format", "json"]);
    project.run(&all).ok().json()["declarations"].clone()
}

#[test]
fn schema_list_holds_every_declared_type_and_relation() {
    let project = project();
    let listed: Vec<String> = declarations(&project, &["list"])
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{} {}",
                d["kind"].as_str().unwrap(),
                d["name"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            "type finding",
            "type fix",
            "relation replaces",
            "relation resolves"
        ]
    );
}

#[test]
fn schema_get_of_a_type_returns_it_as_declared() {
    let project = project();
    assert_eq!(
        declarations(&project, &["get", "finding"]),
        json!([{
            "kind": "type",
            "name": "finding",
            "id_prefix": "FND-",
            "description": "Something found.",
            "statuses": ["open", "closed"],
            "fields": [
                {"name": "severity", "kind": "enum", "values": ["low", "high"], "required": true, "repeatable": false},
                {"name": "tags", "kind": "text", "values": null, "required": false, "repeatable": true},
            ],
        }])
    );
}

#[test]
fn schema_get_of_a_relation_returns_it_as_declared() {
    let project = project();
    let resolves = json!([{
        "kind": "relation",
        "name": "resolves",
        "inverse": "resolved_by",
        "description": "The fix removes the finding.",
        "targets": {"code": ["finding", "fix"], "fix": ["finding"]},
        "same_type": false,
        "acyclic": false,
    }]);
    assert_eq!(declarations(&project, &["get", "resolves"]), resolves);
    assert_eq!(declarations(&project, &["get", "Resolved by"]), resolves);
    assert_eq!(
        declarations(&project, &["get", "replaces"]),
        json!([{
            "kind": "relation",
            "name": "replaces",
            "inverse": "replaced_by",
            "description": "The source replaces the target.",
            "targets": {"finding": ["finding"], "fix": ["fix"]},
            "same_type": true,
            "acyclic": true,
        }])
    );
    let run = project.run(&["schema", "get", "nothing"]).status(1);
    assert!(
        run.stderr.contains("error[unknown-declaration]"),
        "{}",
        run.stderr
    );
}
