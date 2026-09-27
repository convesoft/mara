mod support;
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path, process::Stdio};
use support::*;
use tempfile::TempDir;

fn project() -> TempDir {
    let fixture = fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    fixture
}
fn params(id: &str, file: &str) -> Value {
    json!({"flavour":"requirement","id":id,"file":file,"title":"Created","body":"Body."})
}
fn create(root: &Path, mcp: bool, mut params: Value) -> Result<Value, String> {
    if mcp {
        params["project"] = json!(root);
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "item_create", params),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        if result["isError"] == true {
            Err(result.to_string())
        } else {
            Ok(result["structuredContent"].clone())
        }
    } else {
        let mut args = vec![
            "--format".to_owned(),
            "json".into(),
            "item".into(),
            "create".into(),
        ];
        for key in ["flavour", "id", "file"] {
            args.push(params[key].as_str().unwrap().into());
        }
        args.extend(["--title".into(), params["title"].as_str().unwrap().into()]);
        if let Some(body) = params["body"].as_str() {
            args.extend(["--body".into(), body.into()]);
        }
        if let Some(line) = params["line"].as_u64() {
            args.extend(["--line".into(), line.to_string()]);
        }
        for (property, flag, key, value) in [
            ("fields", "--field", "key", "value"),
            ("relations", "--relation", "relation", "target"),
        ] {
            if let Some(entries) = params[property].as_array() {
                for entry in entries {
                    args.extend([
                        flag.into(),
                        format!(
                            "{}={}",
                            entry[key].as_str().unwrap(),
                            entry[value].as_str().unwrap()
                        ),
                    ]);
                }
            }
        }
        let output = mara(root, &args.iter().map(String::as_str).collect::<Vec<_>>());
        if output.status.success() {
            Ok(serde_json::from_slice(&output.stdout).unwrap())
        } else {
            Err(format!("{}{}", stdout(&output), stderr(&output)))
        }
    }
}
fn diagnostics(root: &Path) -> Vec<mara::Diagnostic> {
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let (corpus, mut diagnostics) = mara::load_corpus_for_validation(&project, &schema).unwrap();
    diagnostics.extend(mara::validate_corpus(&corpus, &schema));
    diagnostics
}
fn read(root: &Path, reference: &str) -> Value {
    let output = mara(root, &["--format", "json", "get", reference]);
    assert!(output.status.success(), "{}", stderr(&output));
    serde_json::from_slice(&output.stdout).unwrap()
}
fn typed_fields(root: &Path) {
    let path = root.join(".mara/schema.yaml");
    let source = fs::read_to_string(&path).unwrap().replace("    id_prefix: REQ-\n    body: required\n    fields: {}", "    id_prefix: REQ-\n    body: required\n    fields:\n      count: {type: integer, required: true}\n      score: {type: number}\n      active: {type: boolean}\n      state: {type: enum, values: [draft, accepted]}\n      tag: {type: string, repeatable: true}");
    fs::write(path, source).unwrap();
}

// @mara implements VER-ITEM-CREATION
// @mara checks REQ-ITEM-CREATION
#[test]
fn creates_complete_items_and_required_body_scaffolds_through_both_surfaces() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        typed_fields(root);
        for (id, body, complete) in [
            ("REQ-COMPLETE", Some("Body."), true),
            ("REQ-OMITTED", None, false),
            ("REQ-EMPTY", Some(""), false),
            ("REQ-BLANK", Some(" \t\n"), false),
        ] {
            let mut request = params(id, "items.mara.md");
            request["body"] = json!(body);
            request["title"] = json!("  Trimmed  ");
            request["fields"] = json!([{"key":"count","value":" 42 "},{"key":"tag","value":"first"},{"key":"tag","value":""}]);
            let result = create(root, mcp, request).unwrap();
            assert_eq!(result["id"], id);
            assert_eq!(result["complete"], complete);
            assert_eq!(
                result["missing"],
                if complete { json!([]) } else { json!(["body"]) }
            );
            let mid = result["mid"].as_str().unwrap();
            assert_eq!(ulid::Ulid::from_string(mid).unwrap().to_string(), mid);
            let get = read(root, mid);
            assert_eq!(get["node"]["id"], id);
            assert_eq!(get["node"]["title"], "Trimmed");
            let source = fs::read_to_string(root.join("items.mara.md")).unwrap();
            assert!(source.contains(&format!(":::mara requirement {id}\n:mid: {mid}\n:title: Trimmed\n:count: 42\n:tag: first\n:tag: \n")));
            assert_eq!(
                source
                    .lines()
                    .nth(result["line"].as_u64().unwrap() as usize - 1)
                    .unwrap(),
                format!(":::mara requirement {id}")
            );
        }
        let errors = diagnostics(root);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(
            errors
                .iter()
                .all(|d| d.code() == mara::DiagnosticCode::FieldInvalid)
        );
    }
}

// @mara checks REQ-ITEM-CREATION
#[test]
fn cli_reads_stdin_and_mcp_treats_dash_as_literal_body() {
    let fixture = project();
    let root = fixture.path();
    let mut child = command(root)
        .args([
            "item",
            "create",
            "requirement",
            "REQ-STDIN",
            "items.mara.md",
            "--title",
            "From stdin",
            "--body",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all("Unicode 🦀\n".as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("complete: true"));
    assert_eq!(read(root, "REQ-STDIN")["content"], "Unicode 🦀\n");
    let mut request = params("REQ-LITERAL", "items.mara.md");
    request["body"] = json!("-");
    create(root, true, request).unwrap();
    assert_eq!(read(root, "REQ-LITERAL")["content"], "-\n");
    assert!(diagnostics(root).is_empty());
}

// @mara checks REQ-ITEM-CREATION
#[test]
fn invalid_identity_scalar_fields_and_duplicates_preserve_destination() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        typed_fields(root);
        let original = "Narrative.\n";
        fs::write(root.join("items.mara.md"), original).unwrap();
        let mut base = params("REQ-NEW", "items.mara.md");
        base["fields"] = json!([{"key":"count","value":"1"}]);
        let mut cases = vec![];
        for (key, value) in [
            ("id", "INVALID"),
            ("id", "WRONG-ID"),
            ("flavour", "unknown"),
            ("title", " "),
            ("title", "two\nlines"),
        ] {
            let mut x = base.clone();
            x[key] = json!(value);
            cases.push(x);
        }
        for fields in [
            json!([]),
            json!([{"key":"count","value":"nope"}]),
            json!([{"key":"count","value":"1"},{"key":"count","value":"2"}]),
            json!([{"key":"count","value":"1"},{"key":"score","value":"NaN"}]),
            json!([{"key":"count","value":"1"},{"key":"active","value":"yes"}]),
            json!([{"key":"count","value":"1"},{"key":"state","value":"unknown"}]),
            json!([{"key":"count","value":"1"},{"key":"mid","value":"01ARZ3NDEKTSV4RRFFQ69G5F00"}]),
            json!([{"key":"count","value":"1"},{"key":"tag","value":"two\nlines"}]),
        ] {
            let mut x = base.clone();
            x["fields"] = fields;
            cases.push(x);
        }
        for request in cases {
            assert!(create(root, mcp, request).is_err());
            assert_eq!(
                fs::read_to_string(root.join("items.mara.md")).unwrap(),
                original
            );
        }
        create(root, mcp, base.clone()).unwrap();
        let before = fs::read(root.join("items.mara.md")).unwrap();
        assert!(create(root, mcp, base).is_err());
        assert_eq!(fs::read(root.join("items.mara.md")).unwrap(), before);
    }
}

// @mara checks REQ-ITEM-INSERTION-SAFETY
#[test]
fn inserts_at_safe_lines_with_destination_newlines_and_permissions() {
    for mcp in [false, true] {
        for newline in ["\n", "\r\n"] {
            let fixture = project();
            let root = fixture.path();
            let path = root.join("items.mara.md");
            let original = format!("Before.{newline}{newline}After.{newline}");
            fs::write(&path, &original).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
            }
            let permissions = fs::metadata(&path).unwrap().permissions();
            let mut request = params("REQ-INSERTED", "items.mara.md");
            request["line"] = json!(3);
            request["body"] = json!(format!("Body.{newline}"));
            let result = create(root, mcp, request).unwrap();
            assert_eq!(result["line"], 3);
            let source = fs::read_to_string(&path).unwrap();
            assert!(source.starts_with(&format!("Before.{newline}{newline}:::mara")));
            assert!(source.ends_with(&format!(":::{newline}{newline}After.{newline}")));
            assert_eq!(fs::metadata(&path).unwrap().permissions(), permissions);
            if newline == "\r\n" {
                assert!(!source.replace("\r\n", "").contains('\n'));
            }
            for line in [0, 4, source.lines().count() + 2] {
                let mut request = params("REQ-REJECTED", "items.mara.md");
                request["line"] = json!(line);
                assert!(create(root, mcp, request).is_err());
                assert_eq!(fs::read_to_string(&path).unwrap(), source);
            }
            let mut request = params("REQ-END", "items.mara.md");
            request["line"] = json!(source.lines().count() + 1);
            create(root, mcp, request).unwrap();
            assert!(diagnostics(root).is_empty());
        }
    }
}

// @mara checks REQ-ITEM-INSERTION-SAFETY
#[test]
fn rejects_escaping_bodies_but_allows_fenced_examples() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        for file in ["new.mara.md", "existing.mara.md"] {
            if file.starts_with("existing") {
                fs::write(root.join(file), "Keep.\n").unwrap();
            }
            let before = fs::read(root.join(file)).ok();
            for body in [
                ":::\nrest",
                ":::mara requirement REQ-NESTED\n:title: Nested\n\nBody.\n:::",
                "Body.\n:::\n\n:::mara requirement REQ-INJECTED\n:title: Injected\n\nBody.",
            ] {
                let mut request = params("REQ-ESCAPE", file);
                request["body"] = json!(body);
                assert!(create(root, mcp, request).is_err());
                assert_eq!(fs::read(root.join(file)).ok(), before);
            }
        }
        let mut request = params("REQ-FENCED", "new.mara.md");
        request["body"] = json!("```markdown\n:::\n:::mara requirement REQ-EXAMPLE\n```\n");
        create(root, mcp, request).unwrap();
        assert!(diagnostics(root).is_empty());
    }
}

// @mara checks REQ-ITEM-CREATION
#[test]
fn destinations_must_be_confined_discoverable_files_with_existing_parents() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        fs::create_dir(root.join("docs")).unwrap();
        let path = root.join(".mara/project.toml");
        fs::write(
            &path,
            fs::read_to_string(&path)
                .unwrap()
                .replace("**/*.mara.md", "docs/**/*.mara.md"),
        )
        .unwrap();
        fs::write(root.join(".gitignore"), "docs/ignored.mara.md\n").unwrap();
        for file in [
            "outside.mara.md",
            "docs/ignored.mara.md",
            "missing/items.mara.md",
            "docs/not-markdown.txt",
            "../outside.mara.md",
            "/outside.mara.md",
        ] {
            assert!(
                create(root, mcp, params("REQ-HIDDEN", file)).is_err(),
                "{file}"
            );
        }
        assert!(!root.join("missing").exists());
        assert!(!root.join("docs/ignored.mara.md").exists());
        create(root, mcp, params("REQ-VISIBLE", "docs/visible.mara.md")).unwrap();
        assert!(diagnostics(root).is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let outside = tempfile::tempdir().unwrap();
            symlink(outside.path(), root.join("docs/outside")).unwrap();
            symlink(root.join("docs"), root.join("docs/linked")).unwrap();
            symlink(
                root.join("docs/visible.mara.md"),
                root.join("docs/file.mara.md"),
            )
            .unwrap();
            let before = fs::read(root.join("docs/visible.mara.md")).unwrap();
            for file in [
                "docs/outside/new.mara.md",
                "docs/linked/new.mara.md",
                "docs/file.mara.md",
            ] {
                assert!(create(root, mcp, params("REQ-SYMLINK", file)).is_err());
            }
            assert!(!outside.path().join("new.mara.md").exists());
            assert!(!root.join("docs/new.mara.md").exists());
            assert_eq!(fs::read(root.join("docs/visible.mara.md")).unwrap(), before);
        }
    }
}
fn relations_schema(root: &Path) {
    let path = root.join(".mara/schema.yaml");
    let source = fs::read_to_string(&path).unwrap()
        + "\n  follows:\n    description: Dependency\n    source: [requirement]\n    target: [requirement]\n    inverse: followed_by\n  associated:\n    description: Association\n    source: [requirement]\n    target: [requirement]\n    symmetric: true\n  tracked_by:\n    description: Ticket\n    source: [requirement]\n    target: []\n    external: true\n";
    fs::write(path, source).unwrap();
}

// @mara checks REQ-ITEM-CREATION
// @mara checks DES-ITEM-CREATION
#[test]
fn initial_relations_resolve_mids_and_reload_equivalent_get_and_related_results() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        relations_schema(root);
        let target = create(root, mcp, params("REQ-TARGET", "target.mara.md")).unwrap();
        let target_mid = target["mid"].as_str().unwrap();
        let original = fs::read(root.join("target.mara.md")).unwrap();
        let mut request = params("REQ-NEW", "new.mara.md");
        request["relations"] = json!([{"relation":"follows","target":target_mid},{"relation":"associated","target":"REQ-TARGET"},{"relation":"tracked_by","target":"external:https://example.test/ticket?x=1"}]);
        let created = create(root, mcp, request).unwrap();
        assert!(
            fs::read_to_string(root.join("new.mara.md"))
                .unwrap()
                .contains(":follows: REQ-TARGET\n")
        );
        assert_eq!(fs::read(root.join("target.mara.md")).unwrap(), original);
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "get", json!({"reference":"REQ-NEW"})),
                mcp_call(
                    3,
                    "related",
                    json!({"reference":target_mid,"relations":["follows"],"direction":"incoming"}),
                ),
            ],
        );
        assert_eq!(
            mcp_response(&responses, 2)["result"]["structuredContent"],
            read(root, "REQ-NEW")
        );
        assert_eq!(read(root, "REQ-NEW")["node"]["mid"], created["mid"]);
        let output = mara(
            root,
            &[
                "--format",
                "json",
                "related",
                target_mid,
                "--relation",
                "follows",
                "--direction",
                "incoming",
            ],
        );
        assert!(output.status.success());
        let related: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            mcp_response(&responses, 3)["result"]["structuredContent"],
            related
        );
        assert_eq!(related["connections"][0]["neighbour"]["id"], "REQ-NEW");
        let mut inverse = params("REQ-INVERSE", "new.mara.md");
        inverse["relations"] = json!([{"relation":"followed_by","target":target_mid}]);
        create(root, mcp, inverse).unwrap();
        assert!(diagnostics(root).is_empty());
    }
}

// @mara checks REQ-ITEM-CREATION
#[test]
fn initial_edge_refusals_leave_existing_and_new_destinations_untouched() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        relations_schema(root);
        let target = create(root, mcp, params("REQ-TARGET", "target.mara.md")).unwrap();
        let mut scenario = params("SCN-TARGET", "scenario.mara.md");
        scenario["flavour"] = json!("scenario");
        create(root, mcp, scenario).unwrap();
        fs::write(root.join("existing.mara.md"), "Keep.\n").unwrap();
        for file in ["new.mara.md", "existing.mara.md"] {
            let before = fs::read(root.join(file)).ok();
            for edges in [
                json!([{"relation":"unknown","target":"REQ-TARGET"}]),
                json!([{"relation":"follows","target":"REQ-MISSING"}]),
                json!([{"relation":"satisfies","target":"REQ-TARGET"}]),
                json!([{"relation":"follows","target":"SCN-TARGET"}]),
                json!([{"relation":"follows","target":"REQ-TARGET"},{"relation":"follows","target":target["mid"]}]),
                json!([{"relation":"tracked_by","target":"external:ftp://example.test"}]),
                json!([{"relation":"follows","target":"REQ-NEW"},{"relation":"followed_by","target":"REQ-NEW"}]),
            ] {
                let mut request = params("REQ-NEW", file);
                request["relations"] = edges;
                assert!(create(root, mcp, request).is_err());
                assert_eq!(fs::read(root.join(file)).ok(), before);
            }
            for body in ["[[REQ-MISSING]]", "[[follows:REQ-TARGET]]"] {
                let mut request = params("REQ-NEW", file);
                request["relations"] = json!([{"relation":"follows","target":"REQ-TARGET"}]);
                request["body"] = json!(body);
                assert!(create(root, mcp, request).is_err());
                assert_eq!(fs::read(root.join(file)).ok(), before);
            }
        }
        // Initial edges require unambiguous existing identities, unlike unrelated conformance errors.
        fs::write(
            root.join("legacy.mara.md"),
            ":::mara requirement REQ-LEGACY\n:title: Legacy\n\nBody.\n:::\n",
        )
        .unwrap();
        let mut request = params("REQ-NEW", "new.mara.md");
        request["relations"] = json!([{"relation":"follows","target":"REQ-TARGET"}]);
        assert!(create(root, mcp, request).is_err());
        assert!(!root.join("new.mara.md").exists());
    }
}

// @mara checks REQ-ITEM-CREATION
#[test]
fn self_relations_allow_scaffolds_and_empty_relations_allow_optional_bodies() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        relations_schema(root);
        let mut request = params("REQ-SELF", "items.mara.md");
        request["body"] = Value::Null;
        request["relations"] = json!([{"relation":"follows","target":"REQ-SELF"}]);
        let result = create(root, mcp, request).unwrap();
        assert_eq!(result["complete"], false);
        assert_eq!(result["missing"], json!(["body"]));
        let path = root.join(".mara/schema.yaml");
        fs::write(
            &path,
            fs::read_to_string(&path)
                .unwrap()
                .replace("body: required", "body: optional"),
        )
        .unwrap();
        for (id, edges) in [("REQ-OMITTED", None), ("REQ-EMPTY", Some(json!([])))] {
            let mut request = params(id, "items.mara.md");
            request["body"] = Value::Null;
            if let Some(edges) = edges {
                request["relations"] = edges;
            }
            let result = create(root, mcp, request).unwrap();
            assert_eq!(result["complete"], true);
            assert_eq!(result["missing"], json!([]));
        }
        assert!(diagnostics(root).is_empty());
    }
}

// @mara checks REQ-ITEM-INSERTION-SAFETY
#[test]
fn creation_rejects_retargeted_heading_links_and_preserves_valid_destinations() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        let source = "# Same\n\nOriginal text.\n\n[Keep target](#same)\n";
        fs::write(root.join("items.mara.md"), source).unwrap();
        let mut request = params("REQ-NEW", "items.mara.md");
        request["line"] = json!(1);
        request["body"] = json!("# Same\n\nNew text.\n");
        let error = create(root, mcp, request.clone()).unwrap_err();
        assert!(error.contains("surviving references"), "{error}");
        assert_eq!(
            fs::read_to_string(root.join("items.mara.md")).unwrap(),
            source
        );
        request["body"] = json!("# Distinct\n\nNew text.\n");
        create(root, mcp, request).unwrap();
        assert!(
            fs::read_to_string(root.join("items.mara.md"))
                .unwrap()
                .ends_with(source)
        );
        assert!(diagnostics(root).is_empty());
    }
}

// @mara checks DES-ITEM-CREATION
#[test]
fn unrelated_broken_references_do_not_exempt_new_broken_references() {
    for mcp in [false, true] {
        for source in [
            "[broken](#absent)\n",
            "<a name=\"same\"></a>\n\nFirst.\n\n<a name=\"same\"></a>\n\nSecond.\n\n[ambiguous](#same)\n",
        ] {
            let fixture = project();
            let root = fixture.path();
            fs::write(root.join("items.mara.md"), source).unwrap();
            let mut request = params("REQ-BROKEN", "items.mara.md");
            request["body"] = json!("[new broken](#missing)");
            assert!(create(root, mcp, request).is_err());
            assert_eq!(
                fs::read_to_string(root.join("items.mara.md")).unwrap(),
                source
            );
            create(root, mcp, params("REQ-VALID", "items.mara.md")).unwrap();
            assert!(
                fs::read_to_string(root.join("items.mara.md"))
                    .unwrap()
                    .starts_with(source)
            );
        }
    }
}

// @mara checks DES-ITEM-CREATION
// @mara checks REQ-RECOVERABLE-MUTATION
#[test]
fn creation_respects_pending_journals_and_active_writer_locks() {
    for mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        let source = "Keep these bytes.\n";
        fs::write(root.join("items.mara.md"), source).unwrap();
        fs::write(root.join(".mara/transaction.json"), "pending").unwrap();
        assert!(
            create(root, mcp, params("REQ-NEW", "items.mara.md"))
                .unwrap_err()
                .contains("pending transaction")
        );
        assert_eq!(
            fs::read_to_string(root.join("items.mara.md")).unwrap(),
            source
        );
        fs::remove_file(root.join(".mara/transaction.json")).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(".mara/mutation.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(
            create(root, mcp, params("REQ-NEW", "items.mara.md"))
                .unwrap_err()
                .contains("another Mara mutation")
        );
        assert_eq!(
            fs::read_to_string(root.join("items.mara.md")).unwrap(),
            source
        );
        lock.unlock().unwrap();
        create(root, mcp, params("REQ-NEW", "items.mara.md")).unwrap();
        assert!(diagnostics(root).is_empty());
    }
}
