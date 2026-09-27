#![cfg(unix)]
use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;

#[cfg(unix)]
fn write_scip_fixture(root: &Path, language: &str, raw: &Value) {
    use protobuf::Message;
    use scip::types;
    let mut index = types::Index::new();
    let mut metadata = types::Metadata::new();
    metadata.project_root = url::Url::from_directory_path(root).unwrap().to_string();
    index.metadata = protobuf::MessageField::some(metadata);
    for document in raw["documents"].as_array().unwrap() {
        let mut doc = types::Document::new();
        doc.relative_path = document["relative_path"].as_str().unwrap().into();
        for occurrence in document["occurrences"].as_array().unwrap() {
            let mut o = types::Occurrence::new();
            o.symbol = occurrence["symbol"].as_str().unwrap_or_default().into();
            o.symbol_roles = occurrence["symbol_roles"].as_i64().unwrap_or_default() as i32;
            o.range = occurrence["range"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap() as i32)
                .collect();
            o.enclosing_range = occurrence["enclosing_range"]
                .as_array()
                .map(|r| r.iter().map(|n| n.as_i64().unwrap() as i32).collect())
                .unwrap_or_default();
            doc.occurrences.push(o);
        }
        index.documents.push(doc);
    }
    fs::write(
        root.join(format!(".mara/{language}.scip")),
        index.write_to_bytes().unwrap(),
    )
    .unwrap();
}

#[cfg(unix)]
fn scip_code_fixture(language: &str) -> TempDir {
    let fixture = support::fixture();
    let root = fixture.path();
    mara::initialize_project(root, mara::Template::Minimal).unwrap();
    let samples = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw: Value = serde_json::from_str(
        &fs::read_to_string(samples.join(format!("tests/fixtures/scip/{language}.json"))).unwrap(),
    )
    .unwrap();
    let source =
        fs::read_to_string(samples.join(format!("tests/fixtures/scip/{language}.source"))).unwrap();
    let path = raw["documents"][0]["relative_path"].as_str().unwrap();
    fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
    fs::write(root.join(path), source).unwrap();
    write_scip_fixture(root, language, &raw);
    let config_path = root.join(".mara/project.toml");
    let mut config = fs::read_to_string(&config_path).unwrap().replacen(
        "format_version = 1",
        "format_version = 3",
        1,
    );
    config.push_str(&format!("\n[[code.languages]]\nname = \"{language}\"\ncommand = [\"cp\", \".mara/{language}.scip\", \"{{output}}\"]\nposition_encoding = \"utf8\"\n"));
    if language != "cpp" {
        fs::create_dir(root.join(".mara/code")).unwrap();
        for extension in ["wasm", "scm"] {
            let file = format!("{language}.{extension}");
            fs::copy(
                samples
                    .join(if language == "typescript" {
                        "tests/fixtures/code"
                    } else {
                        ".mara/code"
                    })
                    .join(&file),
                root.join(".mara/code").join(file),
            )
            .unwrap();
        }
        let extension = if language == "rust" { "rs" } else { "ts" };
        config.push_str(&format!("\nextensions = [\"{extension}\"]\ngrammar = \".mara/code/{language}.wasm\"\nquery = \".mara/code/{language}.scm\"\n"));
    } else {
        config.push_str("extensions = [\"cpp\", \"h\"]\n");
    }
    fs::write(config_path, config).unwrap();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema = fs::read_to_string(&schema_path).unwrap();
    schema.push_str("  code_implements:\n    description: Code implements a requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n");
    fs::write(schema_path, schema).unwrap();
    fs::write(
        root.join("req.mara.md"),
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n\nA.\n:::\n",
    )
    .unwrap();
    fixture
}

// @mara checks REQ-CODE-TRACEABILITY
// @mara implements VER-CODE-DISCOVERY
// Exercise derived ownership through both public transports.
#[cfg(unix)]
#[test]
fn scip_grouped_markers_preserve_exact_endpoints_and_occurrence_spans() {
    let fixture = scip_code_fixture("rust");
    let root = fixture.path();
    let source_path = root.join("src/lib.rs");
    let original = fs::read_to_string(&source_path).unwrap();
    let prefix = "// @mara code_implements REQ-B\n/// Ordinary documentation.\n\n";
    let source = format!("{prefix}{original}");
    fs::write(&source_path, &source).unwrap();
    let mut raw: Value = serde_json::from_str(include_str!("fixtures/scip/rust.json")).unwrap();
    // Translate the captured index's locations with the prepended source lines.
    for occurrence in raw["documents"][0]["occurrences"].as_array_mut().unwrap() {
        for field in ["range", "enclosing_range"] {
            if let Some(range) = occurrence[field].as_array_mut() {
                range[0] = json!(range[0].as_u64().unwrap() + 3);
                if range.len() == 4 {
                    range[2] = json!(range[2].as_u64().unwrap() + 3);
                }
            }
        }
    }
    write_scip_fixture(root, "rust", &raw);
    let document_path = root.join("req.mara.md");
    let mut document = fs::read_to_string(&document_path).unwrap();
    document.push_str(
        "\n:::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: B\n\nB.\n:::\n",
    );
    fs::write(&document_path, &document).unwrap();
    let reference = "code:src/lib.rs::rust::run().";
    for target in ["REQ-A", "REQ-B"] {
        let related = scip_retrieval_parity(
            root,
            &["related", target],
            "related",
            json!({"reference":target}),
        );
        let connections = related["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["neighbour"]["kind"] == "code")
            .collect::<Vec<_>>();
        assert_eq!(connections.len(), 1, "{related:#}");
        assert_eq!(
            connections[0]["neighbour"]["reference"], reference,
            "{target}"
        );
        let edge = scip_retrieval_parity(
            root,
            &["relation", "get", target, "implemented_by_code", reference],
            "relation_get",
            json!({"source":target,"relation":"implemented_by_code","target":reference}),
        );
        assert_eq!(edge["occurrence_count"], 1, "{edge:#}");
        let occurrences = edge["occurrences"].as_array().unwrap();
        assert_eq!(occurrences.len(), 1);
        let location = &occurrences[0]["source"];
        let marker = format!("// @mara code_implements {target}");
        let start = source.find(&marker).unwrap();
        assert_eq!(location["path"], "src/lib.rs");
        assert_eq!(location["start_byte"], start);
        assert_eq!(location["end_byte"], start + marker.len());
    }
    let read = scip_retrieval_parity(
        root,
        &["get", reference],
        "get",
        json!({"reference":reference}),
    );
    assert_eq!(read["content"], "pub fn run() -> u32 { 42 }");
    assert_eq!(fs::read_to_string(source_path).unwrap(), source);
    assert_eq!(fs::read_to_string(document_path).unwrap(), document);
}

#[cfg(unix)]
#[test]
fn scip_symbols_roundtrip_markers_and_inverse_relations_through_cli_and_mcp() {
    let fixture = scip_code_fixture("rust");
    let root = fixture.path();
    let reference = "code:src/lib.rs::rust::run().";
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let read = scip_retrieval_parity(
        root,
        &["get", reference],
        "get",
        json!({"reference":reference}),
    );
    assert_eq!(read["content"], "pub fn run() -> u32 { 42 }");
    let related = scip_retrieval_parity(
        root,
        &["related", reference],
        "related",
        json!({"reference":reference}),
    );
    assert_eq!(related["connections"][0]["neighbour"]["id"], "REQ-A");
    let path = root.join("req.mara.md");
    let source = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        source.replace(
            ":title: A\n",
            &format!(":title: A\n:implemented_by_code: {reference}\n"),
        ),
    )
    .unwrap();
    let edge = relation_tool(
        root,
        "relation_get",
        json!({"source":"REQ-A","relation":"implemented_by_code","target":reference}),
    );
    assert_eq!(edge["occurrence_count"], 2, "{edge:#}");
    let removed = relation_tool(
        root,
        "relation_remove",
        json!({"source":"REQ-A","relation":"implemented_by_code","target":reference}),
    );
    assert_eq!(removed["remaining_occurrences"], 1);
    assert_eq!(removed["edge_exists"], true);
    assert!(
        !mara(root, &["get", "code:src/lib.rs::run"])
            .status
            .success()
    );
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
}

#[cfg(unix)]
#[test]
fn scip_cpp_overloads_are_distinct_and_missing_identity_never_falls_back() {
    let fixture = scip_code_fixture("cpp");
    let root = fixture.path();
    let int_ref = "code:service.cpp::cpp::parse(7864480464b09eea).";
    let double_ref = "code:service.cpp::cpp::parse(620b9ac44d2573a6).";
    for reference in [int_ref, double_ref] {
        let added = mara(
            root,
            &["relation", "add", "REQ-A", "implemented_by_code", reference],
        );
        assert!(added.status.success(), "{}", stderr(&added));
        let read = scip_retrieval_parity(
            root,
            &["get", reference],
            "get",
            json!({"reference":reference}),
        );
        assert_eq!(read["content"], "parse");
    }
    let related = scip_retrieval_parity(
        root,
        &["related", "REQ-A"],
        "related",
        json!({"reference":"REQ-A"}),
    );
    assert_eq!(
        related["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["neighbour"]["kind"] == "code")
            .count(),
        2
    );
    let samples = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut raw: Value = serde_json::from_str(
        &fs::read_to_string(samples.join("tests/fixtures/scip/cpp.json")).unwrap(),
    )
    .unwrap();
    raw["documents"][0]["occurrences"]
        .as_array_mut()
        .unwrap()
        .retain(|o| {
            !o["symbol"]
                .as_str()
                .unwrap_or_default()
                .contains("7864480464b09eea")
        });
    write_scip_fixture(root, "cpp", &raw);
    assert!(!mara(root, &["get", int_ref]).status.success());
    assert!(mara(root, &["get", double_ref]).status.success());
    let invalid = validation_with_parity(root, &[]);
    assert_eq!(invalid["valid"], false);
    assert!(
        invalid["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "code_missing")
    );
}

#[cfg(unix)]
#[test]
fn scip_typescript_shared_callable_survives_package_version_change() {
    let fixture = scip_code_fixture("typescript");
    let root = fixture.path();
    let reference = "code:service.ts::typescript::`service.ts`/parse().";
    let added = mara(
        root,
        &["relation", "add", "REQ-A", "implemented_by_code", reference],
    );
    assert!(added.status.success(), "{}", stderr(&added));
    let read = scip_retrieval_parity(
        root,
        &["get", reference],
        "get",
        json!({"reference":reference}),
    );
    assert_eq!(
        read["content"]
            .as_str()
            .unwrap()
            .matches("function parse")
            .count(),
        3
    );
    let samples = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = fs::read_to_string(samples.join("tests/fixtures/scip/typescript.json"))
        .unwrap()
        .replace("mara-scip-probe 1.0.0", "mara-scip-probe 1.0.1");
    write_scip_fixture(root, "typescript", &serde_json::from_str(&raw).unwrap());
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    assert!(mara(root, &["get", reference]).status.success());
    let item = root.join("req.mara.md");
    let text = fs::read_to_string(&item).unwrap();
    fs::write(
        &item,
        text.replace("A.\n", &format!("A. [[implemented_by_code:{reference}]]\n")),
    )
    .unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let edge = relation_tool(
        root,
        "relation_get",
        json!({"source":"REQ-A","relation":"implemented_by_code","target":reference}),
    );
    assert_eq!(edge["occurrence_count"], 2);
    let related = scip_retrieval_parity(
        root,
        &["related", "REQ-A"],
        "related",
        json!({"reference":"REQ-A"}),
    );
    assert!(
        related["connections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|connection| connection["neighbour"]["reference"] == reference)
    );
}

#[cfg(unix)]
#[test]
fn scip_indexer_failures_do_not_return_partial_code_relations() {
    let fixture = scip_code_fixture("rust");
    let root = fixture.path();
    let path = root.join(".mara/project.toml");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original.replace("\"cp\"", "\"false\"")).unwrap();
    let invalid = validation_with_parity(root, &[]);
    assert_eq!(invalid["valid"], false);
    assert_eq!(invalid["evaluation_complete"], false);
    assert!(!mara(root, &["related", "REQ-A"]).status.success());
    for version in [2, 4] {
        fs::write(
            &path,
            original.replace("format_version = 3", &format!("format_version = {version}")),
        )
        .unwrap();
        let invalid = validation_with_parity(root, &[]);
        assert_eq!(invalid["valid"], false);
        if version == 4 {
            assert!(
                invalid["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "format_unsupported")
            );
        }
    }
}

// @mara checks REQ-CODE-TRACEABILITY
// @mara implements VER-CODE-DISCOVERY
#[cfg(unix)]
#[test]
fn scip_indexers_skip_empty_languages_and_resume_when_sources_appear() {
    for (language, path, reference) in [
        ("rust", "src/lib.rs", "code:src/lib.rs::rust::run()."),
        (
            "cpp",
            "service.cpp",
            "code:service.cpp::cpp::parse(7864480464b09eea).",
        ),
    ] {
        let fixture = scip_code_fixture(language);
        let root = fixture.path();
        let source = fs::read(root.join(path)).unwrap();
        fs::remove_file(root.join(path)).unwrap();
        let config = root.join(".mara/project.toml");
        let original = fs::read_to_string(&config).unwrap();
        fs::write(&config, original.replace("\"cp\"", "\"false\"")).unwrap();
        fs::create_dir(root.join("ignored")).unwrap();
        fs::write(
            root.join("ignored")
                .join(Path::new(path).file_name().unwrap()),
            &source,
        )
        .unwrap();
        fs::write(root.join(".gitignore"), "/ignored/\n").unwrap();
        fs::write(
            root.join("notes.txt"),
            "Unrelated files do not activate a language.\n",
        )
        .unwrap();
        let empty = validation_with_parity(root, &[]);
        assert_eq!(empty["valid"], true, "{language}: {empty:#}");
        assert_eq!(empty["evaluation_complete"], true);
        let related = scip_retrieval_parity(
            root,
            &["related", "REQ-A"],
            "related",
            json!({"reference":"REQ-A"}),
        );
        assert!(
            related["connections"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["neighbour"]["kind"] != "code")
        );
        assert!(!mara(root, &["get", reference]).status.success());

        // An in-project file symlink is a source even if its backing path is ignored.
        std::os::unix::fs::symlink(
            root.join("ignored")
                .join(Path::new(path).file_name().unwrap()),
            root.join(path),
        )
        .unwrap();
        let linked = validation_with_parity(root, &[]);
        assert_eq!(linked["evaluation_complete"], false);
        fs::remove_file(root.join(path)).unwrap();

        fs::write(root.join(path), &source).unwrap();
        let failed = validation_with_parity(root, &[]);
        assert_eq!(failed["valid"], false);
        assert_eq!(failed["evaluation_complete"], false);
        fs::write(&config, &original).unwrap();
        assert_eq!(validation_with_parity(root, &[])["valid"], true);
        scip_retrieval_parity(
            root,
            &["get", reference],
            "get",
            json!({"reference":reference}),
        );

        fs::remove_file(root.join(path)).unwrap();
        let empty_again = validation_with_parity(root, &[]);
        assert_eq!(empty_again["valid"], true);
        assert_eq!(empty_again["evaluation_complete"], true);
    }
}

#[cfg(unix)]
#[test]
fn scip_language_configuration_requires_command_and_complete_grammar_settings() {
    let fixture = scip_code_fixture("rust");
    let root = fixture.path();
    let path = root.join(".mara/project.toml");
    let original = fs::read_to_string(&path).unwrap();
    let source_path = root.join("src/lib.rs");
    let source = fs::read(&source_path).unwrap();
    for omitted in ["command =", "extensions =", "query ="] {
        let candidate = original
            .lines()
            .filter(|line| !line.starts_with(omitted))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, candidate).unwrap();
        assert_eq!(validation_with_parity(root, &[])["valid"], false);
        assert!(
            !mara(root, &["get", "code:src/lib.rs::rust::run()."])
                .status
                .success()
        );
        fs::remove_file(&source_path).unwrap();
        assert_eq!(validation_with_parity(root, &[])["valid"], false);
        fs::write(&source_path, &source).unwrap();
    }
}

#[cfg(unix)]
fn scip_retrieval_parity(root: &Path, args: &[&str], tool: &str, params: Value) -> Value {
    let mut cli_args = vec!["--format", "json"];
    cli_args.extend_from_slice(args);
    let cli = mara(root, &cli_args);
    assert!(cli.status.success(), "{}", stderr(&cli));
    let value: Value = serde_json::from_slice(&cli.stdout).unwrap();
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, tool, params),
        ],
    );
    assert_eq!(
        value,
        mcp_response(&replies, 2)["result"]["structuredContent"]
    );
    value
}

#[cfg(unix)]
#[test]
fn scip_local_scoping_rejects_distinct_full_identity_collisions() {
    let fixture = scip_code_fixture("cpp");
    let root = fixture.path();
    let mut raw: Value = serde_json::from_str(include_str!("fixtures/scip/cpp.json")).unwrap();
    let occurrences = raw["documents"][0]["occurrences"].as_array_mut().unwrap();
    let mut conflicting = occurrences
        .iter()
        .find(|o| {
            o["symbol_roles"] == 1
                && o["symbol"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("7864480464b09eea")
        })
        .unwrap()
        .clone();
    conflicting["symbol"] = json!("cxx other other 1 parse(7864480464b09eea).");
    occurrences.push(conflicting);
    write_scip_fixture(root, "cpp", &raw);
    let result = validation_with_parity(root, &[]);
    assert_eq!(result["valid"], false);
    assert_eq!(result["evaluation_complete"], false);
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "source_invalid")
    );
    assert!(
        !mara(
            root,
            &["get", "code:service.cpp::cpp::parse(7864480464b09eea)."]
        )
        .status
        .success()
    );
}

fn validation_with_parity(root: &Path, paths: &[&str]) -> Value {
    assert!(paths.is_empty());
    let output = mara(root, &["--format", "json", "project", "validate"]);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.success(), value["valid"] == true, "{value:#}");
    assert_eq!(value, relation_tool(root, "project_validate", json!({})));
    value
}
fn relation_tool(root: &Path, tool: &str, params: Value) -> Value {
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, tool, params),
        ],
    );
    let response = &mcp_response(&replies, 2)["result"];
    assert_eq!(response["isError"], false, "{response:#}");
    response["structuredContent"].clone()
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn scip_marker_owner_requires_the_complete_definition_name_span() {
    let fixture = scip_code_fixture("rust");
    let mut raw: Value = serde_json::from_str(include_str!("fixtures/scip/rust.json")).unwrap();
    let occurrence = raw["documents"][0]["occurrences"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|o| {
            o["symbol_roles"] == 1 && o["symbol"].as_str().unwrap_or_default().ends_with("run().")
        })
        .unwrap();
    let range = occurrence["range"].as_array_mut().unwrap();
    let last = range.len() - 1;
    range[last] = json!(range[last].as_i64().unwrap() - 1);
    write_scip_fixture(fixture.path(), "rust", &raw);
    let result = validation_with_parity(fixture.path(), &[]);
    assert_eq!(result["valid"], false, "{result:#}");
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "code_unsupported"),
        "{result:#}"
    );
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn scip_invalid_enclosing_ranges_are_not_silently_discarded() {
    let fixture = scip_code_fixture("rust");
    let mut raw: Value = serde_json::from_str(include_str!("fixtures/scip/rust.json")).unwrap();
    let occurrence = raw["documents"][0]["occurrences"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|o| {
            o["symbol_roles"] == 1 && o["symbol"].as_str().unwrap_or_default().ends_with("run().")
        })
        .unwrap();
    occurrence["enclosing_range"] = json!([999, 0, 999, 3]);
    write_scip_fixture(fixture.path(), "rust", &raw);
    let result = validation_with_parity(fixture.path(), &[]);
    assert_eq!(result["evaluation_complete"], false, "{result:#}");
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "source_invalid"),
        "{result:#}"
    );
}

// @mara implements VER-CODE-DISCOVERY
// @mara checks REQ-CODE-TRACEABILITY
#[test]
fn configured_rust_analyzer_resolves_real_symbols_and_preserves_identity() {
    let fixture = scip_code_fixture("rust");
    let root = fixture.path();
    let config_path = root.join(".mara/project.toml");
    let config = fs::read_to_string(&config_path).unwrap().replace(
        "command = [\"cp\", \".mara/rust.scip\", \"{output}\"]",
        "command = [\"rust-analyzer\", \"scip\", \".\", \"--output\", \"{output}\", \"--num-threads\", \"2\"]");
    fs::write(config_path, config).unwrap();
    fs::remove_file(root.join(".mara/rust.scip")).unwrap();
    fs::write(root.join(".gitignore"), "/target/\n").unwrap();
    let manifest = "[package]\nname = \"mara-scip-live-check\"\nversion = \"1.0.0\"\nedition = \"2024\"\n[workspace]\n";
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let lock = || {
        let output = isolated_command("cargo", root)
            .args(["generate-lockfile", "--offline"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", stderr(&output));
    };
    lock();
    let source_path = root.join("src/lib.rs");
    let source = fs::read_to_string(&source_path).unwrap();
    let reference = "code:src/lib.rs::rust::run().";
    let related = scip_retrieval_parity(
        root,
        &["related", "REQ-A"],
        "related",
        json!({"reference":"REQ-A"}),
    );
    assert!(
        related["connections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["neighbour"]["reference"] == reference),
        "{related:#}"
    );
    let read = scip_retrieval_parity(
        root,
        &["get", reference],
        "get",
        json!({"reference":reference}),
    );
    assert_eq!(read["content"], "pub fn run() -> u32 { 42 }");
    assert_eq!(fs::read_to_string(&source_path).unwrap(), source);
    fs::write(root.join("Cargo.toml"), manifest.replace("1.0.0", "1.1.0")).unwrap();
    fs::write(&source_path, source.replace("42", "43")).unwrap();
    lock();
    let read = scip_retrieval_parity(
        root,
        &["get", reference],
        "get",
        json!({"reference":reference}),
    );
    assert_eq!(read["node"]["reference"], reference);
    assert_eq!(read["content"], "pub fn run() -> u32 { 43 }");
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn scip_encoding_escaping_and_local_definitions_use_exact_identity() {
    use protobuf::Message;
    use scip::types;
    let fixture = support::fixture();
    let root = fixture.path();
    mara::initialize_project(root, mara::Template::Minimal).unwrap();
    code_index::configure(root, "rust", &["rs"], false);
    fs::write(root.join("source.rs"), "😀é run").unwrap();
    let reference = "code:source.rs::rust::`a%20b%5Bx%5D%25%3C%3E%7C`().";
    for (encoding, start, end) in [
        (
            types::PositionEncoding::UTF8CodeUnitOffsetFromLineStart,
            7,
            10,
        ),
        (
            types::PositionEncoding::UTF16CodeUnitOffsetFromLineStart,
            4,
            7,
        ),
        (
            types::PositionEncoding::UTF32CodeUnitOffsetFromLineStart,
            3,
            6,
        ),
    ] {
        let mut index = types::Index::new();
        let mut metadata = types::Metadata::new();
        metadata.project_root = url::Url::from_directory_path(root).unwrap().to_string();
        index.metadata = protobuf::MessageField::some(metadata);
        let mut document = types::Document::new();
        document.relative_path = "source.rs".into();
        document.position_encoding = encoding.into();
        let mut definition = types::Occurrence::new();
        definition.symbol = "test test pkg 1 `a b[x]%<>|`().".into();
        definition.symbol_roles = 1;
        definition.range = vec![0, start, end];
        document.occurrences.push(definition.clone());
        definition.symbol = "local 1".into();
        document.occurrences.push(definition);
        index.documents.push(document);
        let path = root.join(".mara/rust.scip");
        fs::write(&path, index.write_to_bytes().unwrap()).unwrap();
        let read = scip_retrieval_parity(
            root,
            &["get", reference],
            "get",
            json!({"reference":reference}),
        );
        assert_eq!(read["content"], "run");
        assert_eq!(read["node"]["source"]["start_byte"], 7);
        assert!(
            !mara(root, &["get", "code:source.rs::rust::local%201"])
                .status
                .success()
        );
        index.documents[0].occurrences[0].range = vec![0, 999, 1000];
        fs::write(&path, index.write_to_bytes().unwrap()).unwrap();
        assert_eq!(
            validation_with_parity(root, &[])["evaluation_complete"],
            false
        );
    }
    let config_path = root.join(".mara/project.toml");
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replace("position_encoding = \"utf8\"\n", "");
    fs::write(config_path, config).unwrap();
    code_index::write_index(root, "rust", &[("source.rs", &[(7, 10, "run().".into())])]);
    let path = root.join(".mara/rust.scip");
    let mut index = types::Index::parse_from_bytes(&fs::read(&path).unwrap()).unwrap();
    index.documents[0].position_encoding = Default::default();
    fs::write(path, index.write_to_bytes().unwrap()).unwrap();
    assert_eq!(
        validation_with_parity(root, &[])["evaluation_complete"],
        false
    );
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn scip_command_and_output_failures_are_strict_and_sanitized() {
    use protobuf::Message;
    use scip::types;
    let fixture = scip_code_fixture("rust");
    let root = fixture.path();
    let path = root.join(".mara/project.toml");
    let config = fs::read_to_string(&path).unwrap();
    let original_command = "command = [\"cp\", \".mara/rust.scip\", \"{output}\"]";
    for command in [
        json!(["mara-test-no-such-indexer", "{output}"]),
        json!(["true", "{output}"]),
        json!([
            "sh",
            "-c",
            "echo DO_NOT_EXPOSE >&2; exit 3",
            "sh",
            "{output}"
        ]),
        json!([
            "sh",
            "-c",
            "printf changed >> input.txt; cp .mara/rust.scip \"$1\"",
            "sh",
            "{output}"
        ]),
    ] {
        fs::write(
            &path,
            config.replace(original_command, &format!("command = {command}")),
        )
        .unwrap();
        let result = validation_with_parity(root, &[]);
        assert_eq!(result["evaluation_complete"], false, "{result:#}");
        assert!(!result.to_string().contains("DO_NOT_EXPOSE"));
        assert!(!mara(root, &["get", "REQ-A"]).status.success());
    }
    fs::write(&path, config).unwrap();
    let index_path = root.join(".mara/rust.scip");
    let original = fs::read(&index_path).unwrap();
    let mut wrong_root = types::Index::parse_from_bytes(&original).unwrap();
    wrong_root.metadata.as_mut().unwrap().project_root = "file:///not-this-project".into();
    for bytes in [
        vec![],
        b"invalid protobuf".to_vec(),
        wrong_root.write_to_bytes().unwrap(),
    ] {
        fs::write(&index_path, bytes).unwrap();
        assert_eq!(
            validation_with_parity(root, &[])["evaluation_complete"],
            false
        );
    }
    fs::write(index_path, original).unwrap();
    fs::write(root.join(".mara-stage-unpublished"), "candidate bytes").unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn scip_ambiguous_marker_identity_never_selects_the_first_symbol() {
    let fixture = scip_code_fixture("rust");
    let mut raw: Value = serde_json::from_str(include_str!("fixtures/scip/rust.json")).unwrap();
    let occurrences = raw["documents"][0]["occurrences"].as_array_mut().unwrap();
    let mut other = occurrences
        .iter()
        .find(|o| {
            o["symbol_roles"] == 1 && o["symbol"].as_str().unwrap_or_default().ends_with("run().")
        })
        .unwrap()
        .clone();
    other["symbol"] = json!("rust-analyzer cargo mara-scip-probe 1.0.0 other().");
    occurrences.push(other);
    write_scip_fixture(fixture.path(), "rust", &raw);
    let result = validation_with_parity(fixture.path(), &[]);
    assert_eq!(result["valid"], false);
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "code_unsupported"),
        "{result:#}"
    );
    assert!(!mara(fixture.path(), &["get", "REQ-A"]).status.success());
}

// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn scip_code_cursor_detects_source_changes() {
    let fixture = scip_code_fixture("rust");
    let root = fixture.path();
    let args = ["--format", "json", "project", "validate", "--limit", "1"];
    // Two unresolved links produce a continuation that must expire on source changes.
    let item = root.join("req.mara.md");
    let source = fs::read_to_string(&item).unwrap();
    fs::write(&item, source.replace(":title: A\n", ":title: A\n:implemented_by_code: code:missing.rs\n:implemented_by_code: code:other.rs\n")).unwrap();
    let first: Value = serde_json::from_slice(&mara(root, &args).stdout).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    let mut continued = args.to_vec();
    continued.extend(["--cursor", cursor]);
    fs::write(
        root.join("src/lib.rs"),
        "// @mara code_implements REQ-A\npub fn run() -> u32 { 43 }\n",
    )
    .unwrap();
    let stale: Value = serde_json::from_slice(&mara(root, &continued).stdout).unwrap();
    assert_eq!(stale["error"]["code"], "stale_cursor", "{stale:#}");
}
