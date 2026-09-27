use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;

fn invoke(
    root: &Path,
    mcp: bool,
    operation: &str,
    source: &str,
    relation: &str,
    target: &str,
    occurrence: Option<&str>,
) -> Value {
    let result = if mcp {
        let mut params = json!({"source":source,"relation":relation,"target":target});
        if let Some(token) = occurrence {
            params["occurrence"] = json!(token);
        }
        let replies = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, &format!("relation_{operation}"), params),
            ],
        );
        let response = &mcp_response(&replies, 2)["result"];
        assert!(response.get("structuredContent").is_some(), "{response}");
        let result = response["structuredContent"].clone();
        assert_eq!(response["isError"], result.get("error").is_some());
        result
    } else {
        let mut args = vec![
            "--format", "json", "relation", operation, source, relation, target,
        ];
        if let Some(token) = occurrence {
            args.extend(["--occurrence", token]);
        }
        let output = mara(root, &args);
        let result: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("{}", stderr(&output)));
        assert_eq!(
            output.status.success(),
            result.get("error").is_none(),
            "{result}"
        );
        result
    };
    assert_eq!(result["format_version"], 1);
    result
}

fn valid(root: &Path) {
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let corpus = mara::load_corpus(&project, &schema).unwrap();
    assert!(mara::validate_corpus(&corpus, &schema).is_empty());
}
fn code_fixture(language: &str, extension: &str, separator: &str) -> TempDir {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = if matches!(language, "python" | "typescript") {
        manifest.join("tests/fixtures/code")
    } else {
        manifest.join(".mara/code")
    };
    fs::create_dir(fixture.path().join(".mara/code")).unwrap();
    for suffix in ["wasm", "scm"] {
        let name = format!("{language}.{suffix}");
        fs::copy(
            assets.join(&name),
            fixture.path().join(".mara/code").join(name),
        )
        .unwrap();
    }
    let config = fixture.path().join(".mara/project.toml");
    let mut source = fs::read_to_string(&config).unwrap().replacen(
        "format_version = 1",
        "format_version = 3",
        1,
    );
    source.push_str(&format!("\n[[code.languages]]\nname = {language:?}\nextensions = [{extension:?}]\ngrammar = \".mara/code/{language}.wasm\"\nquery = \".mara/code/{language}.scm\"\nseparator = {separator:?}\n"));
    fs::write(config, source).unwrap();
    fixture
}

fn relation_fixture() -> TempDir {
    let fixture = support::fixture();
    let root = fixture.path();
    mara::initialize_project(root, mara::Template::Minimal).unwrap();
    let path = root.join(".mara/schema.yaml");
    let schema = fs::read_to_string(&path).unwrap()
        + "\n  follows:\n    description: Dependency\n    source: [requirement]\n    target: [requirement]\n    inverse: followed_by\n  associated_with:\n    description: Association\n    source: [requirement]\n    target: [requirement]\n    symmetric: true\n  tracked_by:\n    description: External ticket\n    source: [requirement]\n    target: []\n    external: true\n";
    fs::write(path, schema).unwrap();
    for (id, mid, file, newline) in [
        ("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "a.mara.md", "\r\n"),
        ("REQ-B", "01ARZ3NDEKTSV4RRFFQ69G5F01", "b.mara.md", "\n"),
    ] {
        let text = format!(
            ":::mara requirement {id}\n:mid: {mid}\n:title: {id}\n\nPreserved prose.\n:::\n"
        )
        .replace('\n', newline);
        fs::write(root.join(file), text).unwrap();
    }
    fixture
}

// @mara implements VER-RELATION-MUTATION
// @mara checks REQ-RELATION-MUTATION
// @mara checks DES-RELATION-MUTATION
#[test]
fn aliases_mids_selectors_and_symmetric_edges_share_identity() {
    for mcp in [false, true] {
        let fixture = relation_fixture();
        let root = fixture.path();
        let a_path = root.join("a.mara.md");
        let b_path = root.join("b.mara.md");
        let a = fs::read_to_string(&a_path).unwrap();
        let b = fs::read_to_string(&b_path).unwrap();
        let added = invoke(
            root,
            mcp,
            "add",
            "REQ-B",
            "followed_by",
            "01ARZ3NDEKTSV4RRFFQ69G5F00",
            None,
        );
        assert_eq!(added["edge"]["source"]["id"], "REQ-A");
        assert_eq!(added["action"], "added");
        assert_eq!(added["changed_occurrences"], 1);
        assert_eq!(fs::read_to_string(&a_path).unwrap(), a);
        assert!(
            fs::read_to_string(&b_path)
                .unwrap()
                .contains(":followed_by: REQ-A\n")
        );
        fs::write(
            &a_path,
            a.replace(
                "\r\n\r\n",
                "\r\n:follows: REQ-B\r\n:follows: 01ARZ3NDEKTSV4RRFFQ69G5F01\r\n\r\n",
            ),
        )
        .unwrap();
        let duplicate = invoke(root, mcp, "add", "REQ-A", "follows", "REQ-B", None);
        assert_eq!(duplicate["error"]["code"], "relation_exists");
        assert_eq!(duplicate["occurrence_count"], 3);
        let inspected = invoke(root, mcp, "get", "REQ-B", "followed_by", "REQ-A", None);
        let token = inspected["occurrences"][0]["reference"].as_str().unwrap();
        let snapshot = fs::read(&a_path).unwrap();
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "associated_with",
                "REQ-B",
                Some(token)
            )["error"]["code"],
            "occurrence_mismatch"
        );
        assert_eq!(fs::read(&a_path).unwrap(), snapshot);
        let removed = invoke(
            root,
            mcp,
            "remove",
            "REQ-A",
            "follows",
            "REQ-B",
            Some(token),
        );
        assert_eq!(removed["scope"], "occurrence");
        assert_eq!(removed["remaining_occurrences"], 2);
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "follows",
                "REQ-B",
                Some(token)
            )["error"]["code"],
            "stale_occurrence"
        );
        let removed = invoke(root, mcp, "remove", "REQ-A", "follows", "REQ-B", None);
        assert_eq!(removed["changed_occurrences"], 2);
        assert_eq!(removed["edge_exists"], false);
        assert_eq!(fs::read_to_string(&a_path).unwrap(), a);
        assert_eq!(fs::read_to_string(&b_path).unwrap(), b);
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "follows",
                "REQ-B",
                Some(token)
            )["error"]["code"],
            "stale_occurrence"
        );
        assert_eq!(
            invoke(root, mcp, "remove", "REQ-A", "follows", "REQ-B", None)["error"]["code"],
            "relation_not_found"
        );
        let symmetric = invoke(root, mcp, "add", "REQ-B", "associated_with", "REQ-A", None);
        assert_eq!(symmetric["edge"]["symmetric"], true);
        assert_eq!(
            invoke(root, mcp, "add", "REQ-A", "associated_with", "REQ-B", None)["error"]["code"],
            "relation_exists"
        );
        fs::write(
            &a_path,
            a.replace("\r\n\r\n", "\r\n:associated_with: REQ-B\r\n\r\n"),
        )
        .unwrap();
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "associated_with",
                "REQ-B",
                None
            )["changed_occurrences"],
            2
        );
        for name in ["follows", "associated_with"] {
            assert_eq!(
                invoke(root, mcp, "add", "REQ-A", name, "REQ-A", None)["edge_exists"],
                true
            );
            assert_eq!(
                invoke(root, mcp, "remove", "REQ-A", name, "REQ-A", None)["edge_exists"],
                false
            );
        }
        assert_eq!(fs::read_to_string(&a_path).unwrap(), a);
        assert_eq!(fs::read_to_string(&b_path).unwrap(), b);
        assert!(!root.join(".mara/transaction.json").exists());
        valid(root);
    }
}

// @mara checks REQ-RELATION-MUTATION
// @mara checks DES-RELATION-MUTATION
#[test]
fn whole_edge_removal_demotes_inline_assertions_and_preserves_other_bytes() {
    for mcp in [false, true] {
        let fixture = relation_fixture();
        let root = fixture.path();
        let a_path = root.join("a.mara.md");
        let b_path = root.join("b.mara.md");
        let a=fs::read_to_string(&a_path).unwrap().replace("Preserved prose.","Zażółć [[follows:REQ-B]] and [[REQ-B]].\r\n\r\n> Nested [[follows:REQ-B]].\r\n\r\n`[[follows:REQ-B]]` and \\[[follows:REQ-B]].").replace("\r\n\r\nZażółć","\r\n:follows: REQ-B\r\n:associated_with: REQ-B\r\n\r\nZażółć");
        let b = fs::read_to_string(&b_path).unwrap().replace(
            "Preserved prose.",
            "- Check [[followed_by:01ARZ3NDEKTSV4RRFFQ69G5F00]].",
        );
        fs::write(&a_path, &a).unwrap();
        fs::write(&b_path, &b).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&a_path, fs::Permissions::from_mode(0o640)).unwrap();
        }
        let removed = invoke(root, mcp, "remove", "REQ-A", "follows", "REQ-B", None);
        assert_eq!(removed["changed_occurrences"], 4);
        assert_eq!(removed["remaining_occurrences"], 0);
        let expected =
            a.replace(":follows: REQ-B\r\n", "")
                .replacen("[[follows:REQ-B]]", "[[REQ-B]]", 2);
        assert_eq!(fs::read_to_string(&a_path).unwrap(), expected);
        assert_eq!(
            fs::read_to_string(&b_path).unwrap(),
            b.replace(
                "[[followed_by:01ARZ3NDEKTSV4RRFFQ69G5F00]]",
                "[[01ARZ3NDEKTSV4RRFFQ69G5F00]]"
            )
        );
        assert_eq!(
            invoke(root, mcp, "get", "REQ-A", "associated_with", "REQ-B", None)["occurrence_count"],
            1
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&a_path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
        valid(root);
    }
}

// @mara checks REQ-RELATION-MUTATION
#[test]
fn external_addresses_keep_exact_spelling_and_demote_to_autolinks() {
    for mcp in [false, true] {
        let fixture = relation_fixture();
        let root = fixture.path();
        let path = root.join("a.mara.md");
        let target = "external:HTTPS://Example.invalid/A%2fb?x=Y#Z";
        let token = format!("[[tracked_by:{target}]]");
        let source = fs::read_to_string(&path).unwrap().replace(
            "Preserved prose.",
            &format!("Ticket {token}; again {token}."),
        );
        fs::write(&path, &source).unwrap();
        assert_eq!(
            invoke(root, mcp, "add", "REQ-A", "tracked_by", target, None)["error"]["code"],
            "relation_exists"
        );
        let inspected = invoke(root, mcp, "get", "REQ-A", "tracked_by", target, None);
        let selector = inspected["occurrences"][0]["reference"].as_str().unwrap();
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "tracked_by",
                target,
                Some(selector)
            )["remaining_occurrences"],
            1
        );
        let demoted = "<HTTPS://Example.invalid/A%2fb?x=Y#Z>";
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            source.replacen(&token, demoted, 1)
        );
        assert_eq!(
            invoke(root, mcp, "remove", "REQ-A", "tracked_by", target, None)["edge_exists"],
            false
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            source.replace(&token, demoted)
        );
        let other = "external:https://Example.invalid/A%2fb?x=Y#Z";
        assert_eq!(
            invoke(root, mcp, "add", "REQ-A", "tracked_by", target, None)["edge_exists"],
            true
        );
        assert_eq!(
            invoke(root, mcp, "add", "REQ-A", "tracked_by", other, None)["edge_exists"],
            true
        );
        assert!(fs::read_to_string(&path).unwrap().contains(&format!(
            ":tracked_by: {target}\r\n:tracked_by: {other}\r\n"
        )));
        valid(root);
    }
}

// @mara checks DES-RELATION-MUTATION
#[test]
fn heading_link_retargeting_is_rejected_without_writes() {
    for mcp in [false, true] {
        let fixture = relation_fixture();
        let root = fixture.path();
        let path = root.join("a.mara.md");
        let source = fs::read_to_string(&path)
            .unwrap()
            .replace("Preserved prose.", "# Checked [[follows:REQ-B]]");
        fs::write(&path, &source).unwrap();
        fs::write(
            root.join("links.mara.md"),
            "[check](a.mara.md#checked-followsreq-b)\n",
        )
        .unwrap();
        valid(root);
        let inspected = invoke(root, mcp, "get", "REQ-A", "follows", "REQ-B", None);
        let selector = inspected["occurrences"][0]["reference"].as_str().unwrap();
        let b = fs::read(root.join("b.mara.md")).unwrap();
        for occurrence in [None, Some(selector)] {
            let result = invoke(root, mcp, "remove", "REQ-A", "follows", "REQ-B", occurrence);
            assert!(
                result["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("would break or change destination"),
                "{result}"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
            assert_eq!(fs::read(root.join("b.mara.md")).unwrap(), b);
            assert!(!root.join(".mara/transaction.json").exists());
        }
    }
}

// @mara checks REQ-RELATION-MUTATION
// @mara checks DES-RELATION-MUTATION
#[test]
fn code_inverse_mutations_never_edit_comments_and_report_remaining_edge() {
    for mcp in [false, true] {
        let fixture = code_fixture("rust", "rs", "::");
        let root = fixture.path();
        let schema = root.join(".mara/schema.yaml");
        fs::write(&schema,fs::read_to_string(&schema).unwrap()+"\n  code_check:\n    description: Checks requirement\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: checked_by_code\n").unwrap();
        let item =
            ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n\nA.\n:::\n";
        let path = root.join("a.mara.md");
        fs::write(&path, item).unwrap();
        let code_path = root.join("code.rs");
        fs::write(&code_path, "fn run() {}\n").unwrap();
        let added = invoke(
            root,
            mcp,
            "add",
            "REQ-A",
            "checked_by_code",
            "code:code.rs::run",
            None,
        );
        assert_eq!(added["scope"], "item");
        assert_eq!(added["edge"]["source"]["kind"], "code");
        let code = "// @mara code_check REQ-A\nfn run() {}\n";
        fs::write(&code_path, code).unwrap();
        let authored = fs::read_to_string(&path)
            .unwrap()
            .replace("A.\n", "Also [[checked_by_code:code:code.rs::run]].\n");
        fs::write(&path, &authored).unwrap();
        let inspected = invoke(
            root,
            mcp,
            "get",
            "REQ-A",
            "checked_by_code",
            "code:code.rs::run",
            None,
        );
        assert_eq!(inspected["occurrence_count"], 3);
        let comment = inspected["occurrences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["kind"] == "code_comment")
            .unwrap()["reference"]
            .as_str()
            .unwrap();
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "checked_by_code",
                "code:code.rs::run",
                Some(comment)
            )["error"]["code"],
            "unsupported_mutation"
        );
        for operation in ["add", "remove"] {
            assert_eq!(
                invoke(
                    root,
                    mcp,
                    operation,
                    "code:code.rs::run",
                    "code_check",
                    "REQ-A",
                    None
                )["error"]["code"],
                "unsupported_mutation"
            );
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), authored);
        assert_eq!(
            invoke(
                root,
                mcp,
                "add",
                "REQ-A",
                "checked_by_code",
                "code:code.rs::run",
                None
            )["error"]["code"],
            "relation_exists"
        );
        let removed = invoke(
            root,
            mcp,
            "remove",
            "REQ-A",
            "checked_by_code",
            "code:code.rs::run",
            None,
        );
        assert_eq!(removed["changed_occurrences"], 2);
        assert_eq!(removed["remaining_occurrences"], 1);
        assert_eq!(removed["edge_exists"], true);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            authored
                .replace(":checked_by_code: code:code.rs::run\n", "")
                .replace("[[checked_by_code:code:code.rs::run]]", "code:code.rs::run")
        );
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "checked_by_code",
                "code:code.rs::run",
                None
            )["error"]["code"],
            "unsupported_mutation"
        );
        assert_eq!(fs::read_to_string(&code_path).unwrap(), code);
        fs::write(root.join("blob.bin"), [0xff, 0, 0xfe]).unwrap();
        assert_eq!(
            invoke(
                root,
                mcp,
                "add",
                "REQ-A",
                "checked_by_code",
                "code:blob.bin",
                None
            )["edge_exists"],
            true
        );
        assert_eq!(
            invoke(
                root,
                mcp,
                "remove",
                "REQ-A",
                "checked_by_code",
                "code:blob.bin",
                None
            )["edge_exists"],
            false
        );
        assert_eq!(fs::read(root.join("blob.bin")).unwrap(), [0xff, 0, 0xfe]);
        valid(root);
    }
}

// @mara checks REQ-RELATION-MUTATION
#[test]
fn invalid_endpoints_and_ambiguous_identities_preserve_documents() {
    for mcp in [false, true] {
        let fixture = relation_fixture();
        let root = fixture.path();
        let a = fs::read(root.join("a.mara.md")).unwrap();
        let b = fs::read(root.join("b.mara.md")).unwrap();
        for (source, relation, target) in [
            ("REQ-MISSING", "follows", "REQ-B"),
            ("REQ-A", "follows", "REQ-MISSING"),
            ("REQ-A", "unknown", "REQ-B"),
            ("REQ-A", "derives_from", "external:https://example.invalid"),
            ("REQ-A", "follows", "external:https://example.invalid"),
        ] {
            let result = invoke(root, mcp, "add", source, relation, target, None);
            assert!(result.get("error").is_some(), "{result}");
            assert_eq!(fs::read(root.join("a.mara.md")).unwrap(), a);
            assert_eq!(fs::read(root.join("b.mara.md")).unwrap(), b);
        }
        let b_text = String::from_utf8(b.clone()).unwrap();
        for duplicate in [
            b_text.replace("REQ-B", "REQ-A"),
            b_text.replace("01ARZ3NDEKTSV4RRFFQ69G5F01", "01ARZ3NDEKTSV4RRFFQ69G5F00"),
        ] {
            fs::write(root.join("b.mara.md"), &duplicate).unwrap();
            assert!(
                invoke(root, mcp, "add", "REQ-A", "follows", "REQ-A", None)
                    .get("error")
                    .is_some()
            );
            assert_eq!(fs::read(root.join("a.mara.md")).unwrap(), a);
            assert_eq!(
                fs::read_to_string(root.join("b.mara.md")).unwrap(),
                duplicate
            );
        }
        assert!(!root.join(".mara/transaction.json").exists());
    }
}

// @mara checks DES-RELATION-MUTATION
// @mara checks REQ-RECOVERABLE-MUTATION
#[test]
fn pending_and_active_writers_block_both_mutations_without_source_changes() {
    for mcp in [false, true] {
        let fixture = relation_fixture();
        let root = fixture.path();
        assert_eq!(
            invoke(root, mcp, "add", "REQ-A", "follows", "REQ-B", None)["edge_exists"],
            true
        );
        let before = fs::read(root.join("a.mara.md")).unwrap();
        fs::write(root.join(".mara/transaction.json"), "pending").unwrap();
        for operation in ["add", "remove"] {
            let result = invoke(root, mcp, operation, "REQ-A", "follows", "REQ-B", None);
            assert!(
                result["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("pending transaction")
            );
            assert_eq!(fs::read(root.join("a.mara.md")).unwrap(), before);
        }
        fs::remove_file(root.join(".mara/transaction.json")).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(".mara/mutation.lock"))
            .unwrap();
        lock.lock().unwrap();
        for operation in ["add", "remove"] {
            let result = invoke(root, mcp, operation, "REQ-A", "follows", "REQ-B", None);
            assert!(
                result["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("another Mara mutation")
            );
            assert_eq!(fs::read(root.join("a.mara.md")).unwrap(), before);
        }
        lock.unlock().unwrap();
    }
}

// @mara checks DES-RELATION-MUTATION
#[test]
fn removal_allows_incomplete_items_and_preserves_ineligible_alias_named_fields() {
    for mcp in [false, true] {
        let fixture = relation_fixture();
        let root = fixture.path();
        let schema_path = root.join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_path).unwrap();
        // An inverse alias is a custom string on a flavour excluded from both endpoints.
        let schema = schema.replacen(
            "    fields: {}",
            "    fields:\n      followed_by:\n        type: string",
            1,
        );
        fs::write(&schema_path, schema).unwrap();
        let extra = ":::mara scenario SCN-X\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02\n:title: Extra\n:followed_by: Ordinary text\n\nScenario.\n:::\n";
        fs::write(root.join("extra.mara.md"), extra).unwrap();
        assert_eq!(
            invoke(root, mcp, "add", "REQ-A", "follows", "REQ-B", None)["edge_exists"],
            true
        );
        let path = root.join("a.mara.md");
        fs::write(
            &path,
            fs::read_to_string(&path)
                .unwrap()
                .replace("Preserved prose.", ""),
        )
        .unwrap();
        let result = invoke(root, mcp, "remove", "REQ-A", "follows", "REQ-B", None);
        assert_eq!(result["edge_exists"], false, "{result}");
        assert_eq!(
            fs::read_to_string(root.join("extra.mara.md")).unwrap(),
            extra
        );
    }
}
