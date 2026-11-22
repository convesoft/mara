//! `project init` writes this repository's project: the schema and the programs of the
//! `engineering` template are the same text as `.mara/`, its configuration and that of `empty`
//! start with the example of DES-PROJECT-CONFIGURATION, and the template's schema declares the
//! vocabulary that `docs/template/vocabulary.mara.md` shows, as `schema get` returns it. The test
//! reads the template, `.mara/` and the pages from the checkout at run time.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use mara_engine::read::{Paging, SchemaLayer, schema_get, schema_layer, schema_list};
use mara_engine::schema::parse_schema;
use serde_json::{Map, Value, json};

/// The files of the `engineering` template that this repository's `.mara/` holds unchanged.
const SAME: [&str; 4] = [
    "schema.toml",
    "rules/knowledge.dl",
    "rules/readiness.dl",
    "rules/risk-matrix.dl",
];

/// The file at `relative` of this repository.
fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn the_engineering_template_is_this_repositorys_project_and_its_vocabulary_page() {
    let template = |file: &str| read(&format!("crates/mara/templates/{file}"));
    for file in SAME {
        let ours = read(&format!(".mara/{file}"));
        assert_eq!(template(&format!("engineering/{file}")), ours, "{file}");
    }
    let example = documented_example("DES-PROJECT-CONFIGURATION");
    assert_eq!(template("empty/config.toml"), example);
    let config = template("engineering/config.toml");
    assert!(config.starts_with(&example), "{config}");
    assert!(
        config[example.len()..].starts_with("\n[rules]\n"),
        "{config}"
    );

    let text = template("engineering/schema.toml");
    let (schema, diagnostics) = parse_schema(".mara/schema.toml", text.as_bytes());
    assert_eq!(diagnostics, []);
    let schema = schema.expect("the template's schema loads");
    let layer = SchemaLayer {
        schema: &schema,
        key: schema_layer(None, None),
        path: ".mara/schema.toml",
    };
    let paging = Paging {
        limit: 100,
        cursor: None,
    };
    let expected = vocabulary();
    let names = |declarations: &[Value]| -> BTreeSet<(String, String)> {
        declarations
            .iter()
            .map(|declaration| {
                let text = |key: &str| declaration[key].as_str().unwrap().to_owned();
                (text("kind"), text("name"))
            })
            .collect()
    };
    let listed = schema_list(&layer, &paging).expect("schema list");
    assert_eq!(listed.next_cursor, None);
    let listed: Value = serde_json::from_str(&listed.json).unwrap();
    let listed = listed["declarations"].as_array().unwrap();
    assert_eq!(names(listed), names(&expected));
    assert_eq!(listed.len(), expected.len());
    for declaration in expected {
        let name = declaration["name"].as_str().unwrap();
        let got = schema_get(&layer, name, &paging).expect("schema get");
        let got: Value = serde_json::from_str(&got.json).unwrap();
        let got = got["declarations"].as_array().unwrap();
        assert_eq!(got.len(), 1, "{name}: {got:?}");
        assert_eq!(sorted_targets(got[0].clone()), declaration, "{name}");
    }
}

/// The declarations of DES-ENGINEERING-VOCABULARY, read from its page, as `schema get` returns
/// them.
fn vocabulary() -> Vec<Value> {
    let page = read("docs/template/vocabulary.mara.md");
    let name = |label: &str| label.to_lowercase().replace(' ', "_");
    let names = |labels: &str| -> Vec<String> { labels.split(", ").map(name).collect() };
    let statuses: Vec<String> = page
        .lines()
        .find_map(|line| line.strip_prefix("Every type has `Status`:"))
        .expect("the page names the statuses")
        .split('`')
        .skip(1)
        .step_by(2)
        .map(name)
        .collect();
    let mut declarations = Vec::new();
    for row in table(&page, "| Type | ID prefix | Use for | Extra fields |") {
        let [kind, prefix, use_for, extra] = &row[..] else {
            panic!("a type row has four cells: {row:?}");
        };
        let mut fields: Vec<Value> = extra
            .split('`')
            .skip(1)
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|pair| {
                let spec = pair[1].trim_start_matches(':').trim().trim_end_matches('.');
                let (repeatable, spec) = match spec.strip_prefix("repeatable") {
                    Some(rest) => (true, rest.trim_start_matches(';').trim()),
                    None => (false, spec),
                };
                let (kind, values) = match spec {
                    "text" => ("text", Value::Null),
                    values => ("enum", json!(names(values))),
                };
                json!({"name": name(pair[0]), "kind": kind, "values": values,
                       "required": false, "repeatable": repeatable})
            })
            .collect();
        fields.sort_by_key(|field| field["name"].as_str().unwrap().to_owned());
        declarations.push(json!({
            "kind": "type",
            "name": name(kind),
            "id_prefix": prefix.trim_matches('`'),
            "description": format!("{use_for}."),
            "statuses": statuses,
            "fields": fields,
        }));
    }
    let header = "| Key on the source item | Shown on the target as | Source → target types | Constraint | Means |";
    for row in table(&page, header) {
        let [key, shown, ends, constraint, means] = &row[..] else {
            panic!("a relation row has five cells: {row:?}");
        };
        let mut targets = Map::new();
        let mut same_type = false;
        for group in ends.split("; ") {
            let (sources, types) = group.split_once(" → ").expect("sources → targets");
            for source in names(sources) {
                let types = match types {
                    "the same type" => {
                        same_type = true;
                        vec![source.clone()]
                    }
                    types => names(types),
                };
                targets.insert(source, json!(types));
            }
        }
        let acyclic = match constraint.as_str() {
            "No cycles" => true,
            "" | "Child → parent" => false,
            other => panic!("an unknown constraint {other:?}"),
        };
        declarations.push(sorted_targets(json!({
            "kind": "relation",
            "name": name(key),
            "inverse": name(shown),
            "description": format!("{means}."),
            "targets": targets,
            "same_type": same_type,
            "acyclic": acyclic,
        })));
    }
    // `Implements`, which only code markers carry.
    let line = page
        .lines()
        .find(|line| line.starts_with("`Implements` (code → "))
        .expect("the page declares `Implements`");
    let between = |start: &str, end: char| {
        let rest = &line[line.find(start).expect(start) + start.len()..];
        rest[..rest.find(end).unwrap()].to_owned()
    };
    declarations.push(sorted_targets(json!({
        "kind": "relation",
        "name": "implements",
        "inverse": name(&between("shown on the target as `", '`')),
        "description": between("Its schema description is \"", '"'),
        "targets": {"code": names(&between("(code → ", ';'))},
        "same_type": false,
        "acyclic": false,
    })));
    declarations
}

/// The cells of each row of the table under `header` on `page`.
fn table(page: &str, header: &str) -> Vec<Vec<String>> {
    let mut lines = page.lines().skip_while(|line| *line != header).skip(2);
    let mut rows = Vec::new();
    for line in lines.by_ref() {
        let Some(inner) = line
            .strip_prefix('|')
            .and_then(|line| line.strip_suffix('|'))
        else {
            break;
        };
        rows.push(
            inner
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect(),
        );
    }
    assert!(!rows.is_empty(), "no table under {header:?}");
    rows
}

/// A declaration with each relation's target types sorted, which the comparison ignores.
fn sorted_targets(mut declaration: Value) -> Value {
    if let Some(targets) = declaration
        .get_mut("targets")
        .and_then(Value::as_object_mut)
    {
        for types in targets.values_mut() {
            types
                .as_array_mut()
                .unwrap()
                .sort_by_key(|name| name.to_string());
        }
    }
    declaration
}

/// The first TOML code block of the item with `id` on `docs/project.mara.md`.
fn documented_example(id: &str) -> String {
    let page = read("docs/project.mara.md");
    let start = page
        .find(&format!("| ID | {id} |"))
        .unwrap_or_else(|| panic!("docs/project.mara.md has no item {id}"));
    let block = page[start..]
        .split_once("```toml\n")
        .unwrap_or_else(|| panic!("{id} has no TOML example"))
        .1;
    block
        .split_once("```")
        .unwrap_or_else(|| panic!("the TOML example of {id} does not end"))
        .0
        .to_owned()
}
