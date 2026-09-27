mod support;

use mara::resolve_project;
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::*;

// @mara checks REQ-PROJECT-INITIALIZATION
#[test]
fn initializes_the_current_directory_without_touching_existing_content() {
    let fixture = fixture();
    let existing = fixture.path().join("README.md");
    fs::write(&existing, "keep me\n").unwrap();

    let output = mara(fixture.path(), &["project", "init"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(fixture.path().join(".mara/project.toml").is_file());
    assert!(fixture.path().join(".mara/schema.yaml").is_file());
    assert_eq!(fs::read_to_string(existing).unwrap(), "keep me\n");
    let schema = fs::read_to_string(fixture.path().join(".mara/schema.yaml")).unwrap();
    for flavour in ["scenario", "requirement", "design", "decision"] {
        assert!(schema.contains(&format!("  {flavour}:\n")));
    }
}

// @mara checks REQ-PROJECT-INITIALIZATION
#[test]
fn initializes_a_named_missing_or_existing_directory() {
    let fixture = fixture();
    let missing = fixture.path().join("missing");
    let output = mara(fixture.path(), &["project", "init", "missing"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(missing.join(".mara/project.toml").is_file());

    let existing = fixture.path().join("existing");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("notes.txt"), "untouched").unwrap();
    let output = mara(fixture.path(), &["project", "init", "existing"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(existing.join("notes.txt")).unwrap(),
        "untouched"
    );

    let explicit = fixture.path().join("explicit");
    let output = mara(
        fixture.path(),
        &["project", "init", "--project", explicit.to_str().unwrap()],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(explicit.join(".mara/project.toml").is_file());
}

// @mara checks REQ-PROJECT-INITIALIZATION
#[test]
fn rejects_ambiguous_initialization_targets() {
    let fixture = fixture();

    let output = mara(
        fixture.path(),
        &["project", "init", "named", "--project", "explicit"],
    );

    assert!(!output.status.success());
    assert!(stderr(&output).contains("cannot be used together"));
    assert!(!fixture.path().join("named").exists());
    assert!(!fixture.path().join("explicit").exists());
}

// @mara checks REQ-PROJECT-INITIALIZATION
#[test]
fn refuses_to_overwrite_an_existing_project_or_target_file() {
    let fixture = fixture();
    let first = mara(fixture.path(), &["project", "init"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let original_project = fs::read(fixture.path().join(".mara/project.toml")).unwrap();

    let repeated = mara(fixture.path(), &["project", "init"]);

    assert!(!repeated.status.success());
    assert!(stderr(&repeated).contains("already exists"));
    assert_eq!(
        fs::read(fixture.path().join(".mara/project.toml")).unwrap(),
        original_project
    );

    let conflict = fixture.path().join("conflict");
    fs::create_dir_all(conflict.join(".mara")).unwrap();
    fs::write(conflict.join(".mara/schema.yaml"), "do not replace\n").unwrap();
    let output = mara(fixture.path(), &["project", "init", "conflict"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("refusing to overwrite"));
    assert_eq!(
        fs::read_to_string(conflict.join(".mara/schema.yaml")).unwrap(),
        "do not replace\n"
    );
    assert!(!conflict.join(".mara/project.toml").exists());
}

// @mara checks REQ-PROJECT-INITIALIZATION
#[test]
fn empty_template_creates_no_project_flavours() {
    let fixture = fixture();

    let output = mara(fixture.path(), &["project", "init", "--template", "empty"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(fixture.path().join(".mara/schema.yaml")).unwrap(),
        "format_version: 3\nflavours: {}\nrelations: {}\n"
    );
}

// @mara checks REQ-PROJECT-DISCOVERY
#[test]
fn real_cli_initializes_projects_resolved_by_nearest_and_explicit_roots() {
    let fixture = fixture();
    let outer = fixture.path().join("outer");
    let nested = outer.join("nested");
    fs::create_dir_all(&nested).unwrap();
    for root in [&outer, &nested] {
        let output = mara(fixture.path(), &["project", "init", root.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
    }
    let deep = nested.join("a/b");
    fs::create_dir_all(&deep).unwrap();

    let discovered = resolve_project(None, &deep).unwrap();
    assert_eq!(discovered.root(), nested.canonicalize().unwrap());

    let explicit = resolve_project(Some(Path::new("../../..")), &deep).unwrap();
    assert_eq!(explicit.root(), outer.canonicalize().unwrap());
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn engineering_init_preserves_existing_policy_and_check_files() {
    for file in [
        "engineering-rules.yaml",
        "engineering-checks.yaml",
        "engineering-execution.yaml",
    ] {
        let fixture = fixture();
        let root = fixture.path();
        fs::create_dir(root.join(".mara")).unwrap();
        fs::write(root.join(".mara").join(file), "project-owned content").unwrap();
        let init = mara(root, &["project", "init", "--template", "engineering"]);
        assert!(!init.status.success());
        assert_eq!(fs::read_dir(root.join(".mara")).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(root.join(".mara").join(file)).unwrap(),
            "project-owned content"
        );
    }
}

// @mara implements VER-PROJECT-INSPECTION
// @mara checks REQ-ENGINEERING-TEMPLATE
// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn bundled_templates_initialize_and_inspect_equally_through_cli_and_mcp() {
    for (template, flavour_count) in [("minimal", 4), ("empty", 0), ("engineering", 11)] {
        let fixture = fixture();
        let cli_root = fixture.path().join("cli");
        let mcp_root = fixture.path().join("mcp");
        let init = mara(
            fixture.path(),
            &[
                "project",
                "init",
                cli_root.to_str().unwrap(),
                "--template",
                template,
            ],
        );
        assert!(init.status.success(), "{}", stderr(&init));
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "project_init",
                    json!({"project":mcp_root,"template":template}),
                ),
                mcp_call(3, "schema_get", json!({"project":mcp_root})),
                mcp_call(
                    5,
                    "project_init",
                    json!({"project":mcp_root,"template":template}),
                ),
            ],
        );
        assert_ne!(mcp_response(&responses, 2)["result"]["isError"], true);
        assert_eq!(mcp_response(&responses, 5)["result"]["isError"], true);
        let output = mara(&cli_root, &["--format", "json", "schema", "get"]);
        assert!(output.status.success(), "{}", stderr(&output));
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            cli,
            mcp_response(&responses, 3)["result"]["structuredContent"]
        );
        assert_eq!(cli["schema"]["format_version"], 3);
        let flavours = cli["schema"]["flavours"].as_object().unwrap();
        assert_eq!(flavours.len(), flavour_count);
        for declaration in flavours.values() {
            assert!(!declaration["use_when"].as_array().unwrap().is_empty());
            assert!(declaration["avoid_when"].is_array());
            assert!(declaration["distinguish_from"].is_object());
        }
        let schema = fs::read(cli_root.join(".mara/schema.yaml")).unwrap();
        assert_eq!(
            schema,
            fs::read(mcp_root.join(".mara/schema.yaml")).unwrap()
        );
        assert!(
            !mara(&cli_root, &["project", "init", "--template", template])
                .status
                .success()
        );
        assert_eq!(
            schema,
            fs::read(cli_root.join(".mara/schema.yaml")).unwrap()
        );
        for root in [&cli_root, &mcp_root] {
            assert_eq!(fs::read_dir(root).unwrap().count(), 1);
            assert_eq!(
                fs::read_dir(root.join(".mara")).unwrap().count(),
                if template == "engineering" { 5 } else { 2 }
            );
            let config = fs::read_to_string(root.join(".mara/project.toml")).unwrap();
            assert!(config.starts_with(if template == "engineering" {
                "format_version = 2"
            } else {
                "format_version = 1"
            }));
        }
    }
}

// @mara checks REQ-FLAVOUR-AUTHORING-GUIDANCE
// @mara checks DES-FLAVOUR-AUTHORING-GUIDANCE
#[test]
fn schema_guidance_rejects_invalid_declarations_through_cli_and_mcp() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let path = fixture.path().join(".mara/schema.yaml");
    let valid = "format_version: 3\nflavours:\n  note:\n    description: A project note.\n    use_when: [Record useful context.]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: NOTE-\n    body: optional\nrelations: {}\n";
    fs::write(&path, valid).unwrap();
    let accepted = mara(fixture.path(), &["schema", "get"]);
    assert!(accepted.status.success(), "{}", stderr(&accepted));
    let cases = [
        ("format_version: 3", "format_version: 1", "migrate"),
        ("    description: A project note.\n", "", "description"),
        (
            "description: A project note.",
            "description: '  '",
            "description",
        ),
        (
            "description: A project note.",
            "description: 42",
            "description",
        ),
        ("    use_when: [Record useful context.]\n", "", "use_when"),
        (
            "use_when: [Record useful context.]",
            "use_when: []",
            "use_when",
        ),
        (
            "use_when: [Record useful context.]",
            "use_when: ['  ']",
            "use_when",
        ),
        (
            "use_when: [Record useful context.]",
            "use_when: context",
            "use_when",
        ),
        (
            "use_when: [Record useful context.]",
            "use_when: [true]",
            "use_when",
        ),
        ("    avoid_when: []\n", "", "avoid_when"),
        ("avoid_when: []", "avoid_when: ['  ']", "avoid_when"),
        ("avoid_when: []", "avoid_when: {}", "avoid_when"),
        ("avoid_when: []", "avoid_when: [42]", "avoid_when"),
        ("    distinguish_from: {}\n", "", "distinguish_from"),
        (
            "distinguish_from: {}",
            "distinguish_from: []",
            "distinguish_from",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {note: '  '}",
            "distinguish_from",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {note: true}",
            "distinguish_from",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {note: Same flavour.}",
            "itself",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {missing: Unknown flavour.}",
            "unknown flavour 'missing'",
        ),
        (
            "avoid_when: []",
            "avoid_when: []\n    guidance: {}",
            "unknown configuration key 'guidance'",
        ),
    ];
    for (from, to, expected) in cases {
        let source = valid.replace(from, to);
        fs::write(&path, &source).unwrap();
        let cli = mara(fixture.path(), &["schema", "get"]);
        assert!(!cli.status.success(), "accepted {to}");
        assert!(stderr(&cli).contains(expected), "{}", stderr(&cli));
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "schema_get", json!({})),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        assert_eq!(result["isError"], true, "{result}");
        assert!(result.to_string().contains(expected), "{result}");
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

// @mara checks REQ-PROJECT-INITIALIZATION
// @mara checks REQ-PROJECT-DISCOVERY
#[test]
fn mcp_starts_outside_a_project_and_initializes_an_absolute_target() {
    let fixture = fixture();
    let project = fixture.path().join("new-project");

    let initialized = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(
                2,
                "project_init",
                json!({ "project": project, "template": "minimal" }),
            ),
        ],
    );
    let result = &mcp_response(&initialized, 2)["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"]["project"]["root"],
        project.to_string_lossy().as_ref()
    );
    assert!(project.join(".mara/project.toml").is_file());
    assert!(project.join(".mara/schema.yaml").is_file());

    let validated = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "schema_get", json!({ "project": project })),
            mcp_call(3, "schema_get", json!({ "project": "new-project" })),
        ],
    );
    assert_eq!(
        mcp_response(&validated, 2)["result"]["structuredContent"]["kind"],
        "schema"
    );
    assert_eq!(mcp_response(&validated, 3)["result"]["isError"], true);
    assert!(
        mcp_response(&validated, 3)
            .to_string()
            .contains("must be absolute")
    );
}

// @mara checks REQ-PROJECT-INITIALIZATION
// @mara checks DES-OPERATION-PROJECT-CONTEXT
#[test]
fn mcp_bound_server_initializes_its_selected_target_without_an_override() {
    let fixture = fixture();
    let project = fixture.path().join("new-project");
    let project_path = project.to_str().unwrap();

    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["mcp", "--project", project_path],
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_init", json!({ "template": "minimal" })),
            mcp_call(3, "project_init", json!({ "project": project_path })),
        ],
    );

    let instructions = mcp_response(&responses, 1)["result"]["instructions"]
        .as_str()
        .unwrap();
    assert!(instructions.contains("explicit destination only when the server is unbound"));
    assert!(
        instructions.contains("omit request-level project selection, including for project_init")
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], false);
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"]["project"]["root"],
        project_path
    );
    assert!(project.join(".mara/project.toml").is_file());
    assert!(project.join(".mara/schema.yaml").is_file());
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 3)
            .to_string()
            .contains("started with --project")
    );
}

// @mara checks REQ-PROJECT-INITIALIZATION
#[test]
fn mcp_unbound_project_init_requires_an_absolute_target() {
    let fixture = fixture();

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_init", json!({})),
        ],
    );

    let instructions = mcp_response(&responses, 1)["result"]["instructions"]
        .as_str()
        .unwrap();
    assert!(instructions.contains("explicit destination only when the server is unbound"));
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 2)
            .to_string()
            .contains("requires an absolute project path")
    );
    assert!(!fixture.path().join(".mara/project.toml").exists());
}

// @mara checks REQ-PROJECT-DISCOVERY
#[test]
fn mcp_project_option_after_the_command_binds_the_server() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project = fixture.path().to_str().unwrap();

    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["mcp", "--project", project],
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "schema_get", json!({})),
            mcp_call(3, "schema_get", json!({ "project": project })),
        ],
    );

    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"]["kind"],
        "schema"
    );
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 3)
            .to_string()
            .contains("started with --project")
    );
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn real_cli_discovers_and_inspects_the_effective_minimal_schema() {
    let fixture = fixture();
    let project_root = fixture.path().join("project");
    let init = mara(
        fixture.path(),
        &["project", "init", project_root.to_str().unwrap()],
    );
    assert!(init.status.success(), "{}", stderr(&init));
    let nested = project_root.join("nested/deeper");
    fs::create_dir_all(&nested).unwrap();

    let complete = mara(&nested, &["schema", "get"]);
    assert!(complete.status.success(), "{}", stderr(&complete));
    let complete = stdout(&complete);
    assert!(complete.contains("format_version: 3"));
    assert!(complete.contains("requirement:"));
    assert!(complete.contains("satisfies:"));

    let flavours = mara(&nested, &["schema", "list", "flavour"]);
    assert!(flavours.status.success(), "{}", stderr(&flavours));
    let flavours = stdout(&flavours);
    assert!(flavours.contains("requirement\tAn independently verifiable obligation."));
    assert!(flavours.contains("design\tA solution or interface contract"));

    let relations = mara(&nested, &["schema", "list", "relation"]);
    assert!(relations.status.success(), "{}", stderr(&relations));
    let relations = stdout(&relations);
    assert!(relations.contains("derives_from\tThe source originates"));
    assert!(relations.contains("supersedes\tThe source replaces"));

    let relation = mara(&nested, &["schema", "get", "relation", "satisfies"]);
    assert!(relation.status.success(), "{}", stderr(&relation));
    let relation = stdout(&relation);
    assert!(relation.starts_with("satisfies:\n"));
    assert!(relation.contains("source:\n  - design"));
    assert!(relation.contains("target:\n  - requirement"));

    for (args, tool, arguments) in [
        (
            vec!["schema", "list", "flavour"],
            "schema_list",
            json!({"kind":"flavour"}),
        ),
        (
            vec!["schema", "list", "relation"],
            "schema_list",
            json!({"kind":"relation"}),
        ),
        (
            vec!["schema", "get", "flavour", "requirement"],
            "schema_get",
            json!({"kind":"flavour", "name":"requirement"}),
        ),
        (
            vec!["schema", "get", "relation", "satisfies"],
            "schema_get",
            json!({"kind":"relation", "name":"satisfies"}),
        ),
    ] {
        let mut cli_args = vec!["--format", "json"];
        cli_args.extend(args);
        let output = mara(&nested, &cli_args);
        assert!(output.status.success(), "{}", stderr(&output));
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        let responses = mcp_exchange(
            &nested,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
                mcp_call(2, tool, arguments),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"], cli);
    }
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_commands_load_the_schema_configured_by_the_selected_project() {
    let fixture = fixture();
    let selected = fixture.path().join("selected");
    let init = mara(
        fixture.path(),
        &["project", "init", selected.to_str().unwrap()],
    );
    assert!(init.status.success(), "{}", stderr(&init));

    let project_file = selected.join(".mara/project.toml");
    let project_source = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project_source.replace(".mara/schema.yaml", ".mara/custom.yaml"),
    )
    .unwrap();
    fs::write(
        selected.join(".mara/custom.yaml"),
        r#"format_version: 3
flavours:
  note:
    description: A concise project note.
    use_when: [Record project knowledge.]
    avoid_when: []
    distinguish_from: {}
    id_prefix: NOTE-
    body: optional
    fields:
      text:
        type: string
      count:
        type: integer
      ratio:
        type: number
      enabled:
        type: boolean
      status:
        type: enum
        required: true
        repeatable: false
        values: [draft, accepted]
relations:
  depends_on:
    description: The source requires the target.
    source: [note]
    target: [note]
"#,
    )
    .unwrap();

    let note = mara(
        fixture.path(),
        &[
            "--project",
            selected.to_str().unwrap(),
            "schema",
            "get",
            "flavour",
            "note",
        ],
    );
    assert!(note.status.success(), "{}", stderr(&note));
    let note = stdout(&note);
    assert!(note.starts_with("note:\n"));
    assert!(note.contains("id_prefix: NOTE-"));
    assert!(note.contains("type: enum"));
    assert!(note.contains("values:\n      - draft\n      - accepted"));
    assert!(!note.contains("values: null"));
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_get_rejects_an_unknown_declaration() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));

    let get = mara(fixture.path(), &["schema", "get", "flavour", "missing"]);

    assert!(!get.status.success());
    assert!(stderr(&get).contains("unknown flavour 'missing'"));
}
// @mara implements VER-PROJECT-INSPECTION
#[test]
fn bootstrap_advertises_only_its_available_operations() {
    let fixture = fixture();
    for (args, present, absent) in [
        (
            vec!["--help"],
            vec![
                "project", "schema", "item", "mcp", "search", "get", "related", "relation",
            ],
            vec!["trace"],
        ),
        (
            vec!["project", "--help"],
            vec!["init"],
            vec!["validate", "mid-backfill"],
        ),
        (
            vec!["schema", "--help"],
            vec!["get", "list", "validate"],
            vec![],
        ),
        (
            vec!["relation", "--help"],
            vec!["get", "add", "remove"],
            vec![],
        ),
        (
            vec!["item", "--help"],
            vec!["create", "list"],
            vec!["get", "validate", "search"],
        ),
    ] {
        let output = mara(fixture.path(), &args);
        assert!(output.status.success(), "{}", stderr(&output));
        let help = stdout(&output);
        for command in present {
            assert!(
                help.lines()
                    .any(|line| line.starts_with(&format!("  {command} "))),
                "{help}"
            );
        }
        for command in absent {
            assert!(
                !help
                    .lines()
                    .any(|line| line.starts_with(&format!("  {command} "))),
                "{help}"
            );
        }
    }
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_request(2, "tools/list", json!({})),
        ],
    );
    let mut names: Vec<_> = mcp_response(&responses, 2)["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "get",
            "item_create",
            "item_list",
            "project_init",
            "project_mid_backfill",
            "project_transaction_rollback",
            "related",
            "relation_add",
            "relation_get",
            "relation_remove",
            "schema_get",
            "schema_list",
            "schema_validate",
            "search"
        ]
    );
}
