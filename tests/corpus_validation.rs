mod support;
use mara::{Diagnostic, DiagnosticCode};
use std::{fs, path::Path};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    fixture
}
fn validate(root: &Path) -> (mara::Corpus, Vec<Diagnostic>) {
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let (corpus, mut diagnostics) = mara::load_corpus_for_validation(&project, &schema).unwrap();
    diagnostics.extend(mara::validate_corpus(&corpus, &schema));
    (corpus, diagnostics)
}
fn code_adapter(root: &Path) {
    fs::create_dir(root.join(".mara/code")).unwrap();
    for extension in ["wasm", "scm"] {
        let name = format!("rust.{extension}");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(".mara/code")
                .join(&name),
            root.join(".mara/code").join(name),
        )
        .unwrap();
    }
    let path = root.join(".mara/project.toml");
    let config =
        fs::read_to_string(&path)
            .unwrap()
            .replacen("format_version = 1", "format_version = 3", 1);
    fs::write(path, config + "\n[[code.languages]]\nname = \"rust\"\nextensions = [\"rs\"]\ngrammar = \".mara/code/rust.wasm\"\nquery = \".mara/code/rust.scm\"\nseparator = \"::\"\n").unwrap();
    let path = root.join(".mara/schema.yaml");
    let schema = fs::read_to_string(&path).unwrap();
    fs::write(path, schema + "\n  checks:\n    description: Code check\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: checked_by\n").unwrap();
}
fn item(id: &str, mid: &str, metadata: &str, body: &str) -> String {
    format!(":::mara requirement {id}\n:mid: {mid}\n:title: Item\n{metadata}\n{body}\n:::\n")
}
const MID: &str = "01ARZ3NDEKTSV4RRFFQ69G5F00";

// @mara implements VER-CORPUS-CONFORMANCE
// @mara checks DES-CORPUS-CONFORMANCE
#[test]
fn incomplete_document_discovery_does_not_invent_missing_code_marker_targets() {
    let fixture = fixture();
    let root = fixture.path();
    code_adapter(root);
    let source = "// @mara checks REQ-TARGET\nfn check() {}\n";
    fs::write(root.join("check.rs"), source).unwrap();
    fs::write(root.join("target.mara.md"), [0xff]).unwrap();
    let (corpus, diagnostics) = validate(root);
    assert!(!corpus.is_complete());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == DiagnosticCode::SourceInvalid)
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code() == DiagnosticCode::ReferenceUnresolved),
        "{diagnostics:?}"
    );
    fs::write(root.join("target.mara.md"), "Ordinary text.\n").unwrap();
    let (corpus, diagnostics) = validate(root);
    assert!(corpus.is_complete());
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::ReferenceUnresolved);
    fs::write(
        root.join("target.mara.md"),
        item("REQ-TARGET", MID, "", "Body."),
    )
    .unwrap();
    assert!(validate(root).1.is_empty());
    assert_eq!(fs::read_to_string(root.join("check.rs")).unwrap(), source);
}

fn invalid_identities(root: &Path) -> Vec<Diagnostic> {
    let source = fs::read(root.join("invalid.mara.md")).unwrap();
    let diagnostics = validate(root).1;
    assert!(!diagnostics.is_empty());
    assert!(
        diagnostics
            .iter()
            .all(|d| d.code() == DiagnosticCode::IdentityInvalid),
        "{diagnostics:?}"
    );
    assert_eq!(fs::read(root.join("invalid.mara.md")).unwrap(), source);
    diagnostics
}
// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn corpus_validation_treats_mid_as_structural_metadata() {
    let fixture = fixture();
    fs::write(
        fixture.path().join("valid.mara.md"),
        r#":::mara requirement REQ-VALID
:mid: 01JQZ4W7G5H8K2M3N6P9R0STVX
:title: Valid

A complete requirement.
:::
"#,
    )
    .unwrap();

    assert!(validate(fixture.path()).1.is_empty());
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn corpus_validation_reports_missing_malformed_duplicate_and_misplaced_mids() {
    let fixture = fixture();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-MISSING
:title: Missing MID

Body.
:::

:::mara requirement REQ-MALFORMED
:mid: not-a-mid
:title: Malformed MID

Body.
:::

:::mara requirement REQ-OVERFLOW
:mid: ZZZZZZZZZZZZZZZZZZZZZZZZZZ
:title: Overflow MID

Body.
:::

:::mara requirement REQ-DUPLICATE-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate one

Body.
:::

:::mara requirement REQ-DUPLICATE-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate two

Body.
:::

:::mara requirement REQ-MISPLACED
:title: Misplaced MID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01

Body.
:::
"#,
    )
    .unwrap();

    let diagnostics = invalid_identities(fixture.path());
    let lines: Vec<_> = diagnostics
        .iter()
        .map(|d| d.source().span().start_line())
        .collect();
    assert_eq!(lines, [1, 8, 15, 22, 29, 37]);
    assert!(
        diagnostics
            .iter()
            .all(|d| d.source().path() == Path::new("invalid.mara.md"))
    );
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn corpus_validation_rejects_non_bijective_item_identities() {
    let fixture = fixture();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Shared ID one

Body.
:::

:::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Shared ID two

Body.
:::

:::mara requirement REQ-SHARED-MID-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID one

Body.
:::

:::mara requirement REQ-SHARED-MID-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID two

Body.
:::
"#,
    )
    .unwrap();

    let diagnostics = invalid_identities(fixture.path());
    let lines: Vec<_> = diagnostics
        .iter()
        .map(|d| d.source().span().start_line())
        .collect();
    assert_eq!(lines, [1, 8, 16, 23]);
    assert!(
        diagnostics
            .iter()
            .all(|d| d.source().path() == Path::new("invalid.mara.md"))
    );
    for reference in ["REQ-SHARED-ID", "01ARZ3NDEKTSV4RRFFQ69G5F02"] {
        let get = support::mara(fixture.path(), &["--format", "json", "get", reference]);
        assert!(!get.status.success(), "{}", support::stdout(&get));
    }
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn corpus_validation_reports_the_duplicated_secondary_mid_entry() {
    let fixture = fixture();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-FIRST
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: First

Body.
:::

:::mara requirement REQ-SECOND
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Second

Body.
:::
"#,
    )
    .unwrap();

    let diagnostics = invalid_identities(fixture.path());
    let mut lines: Vec<_> = diagnostics
        .iter()
        .map(|d| d.source().span().start_line())
        .collect();
    lines.sort_unstable();
    assert_eq!(lines, [3, 3, 10]);
}

// @mara checks REQ-PROJECT-VALIDATION
#[test]
fn independent_flavour_prefix_metadata_relation_and_mention_errors_accumulate() {
    let fixture = fixture();
    let source = ":::mara requirement WRONG-ID\n:title: Invalid\n:unknown: value\n:derives_from: MISSING-RELATION\n\nMentions [[MISSING-MENTION]].\n:::\n\n:::mara mystery MYS-UNKNOWN\n:title: Unknown\n\nBody.\n:::\n";
    fs::write(fixture.path().join("invalid.mara.md"), source).unwrap();
    let (_, diagnostics) = validate(fixture.path());
    assert_eq!(diagnostics.len(), 7, "{diagnostics:?}");
    for code in [
        DiagnosticCode::IdentityInvalid,
        DiagnosticCode::FieldInvalid,
        DiagnosticCode::ReferenceUnresolved,
        DiagnosticCode::SourceInvalid,
    ] {
        assert!(diagnostics.iter().any(|d| d.code() == code));
    }
    let positions: Vec<_> = diagnostics
        .iter()
        .map(|d| {
            (
                d.source().path(),
                d.source().span().start_line(),
                d.message(),
            )
        })
        .collect();
    let mut ordered = positions.clone();
    ordered.sort();
    assert_eq!(positions, ordered);
    assert_eq!(
        fs::read_to_string(fixture.path().join("invalid.mara.md")).unwrap(),
        source
    );
}

// @mara checks DES-CORPUS-CONFORMANCE
#[test]
fn recovered_titles_metadata_and_missing_closers_preserve_independent_checks() {
    let fixture = fixture();
    let root = fixture.path();
    fs::write(
        root.join("first.mara.md"),
        item("REQ-DUPLICATE", MID, "", "Body."),
    )
    .unwrap();
    for (tail, expected) in [
        (
            ":title: Second\n:title: Again\n:unknown: value\n:derives_from: MISSING-TARGET\n\n:::\n",
            vec![
                "exactly one non-empty title",
                "duplicate item ID",
                "required body is empty",
                "unknown metadata",
                "references missing item",
            ],
        ),
        (
            ":title: Partial\n:unknown: value\n:derives_from: MISSING-TARGET\n:malformed\n\nBody.\n:::\n",
            vec![
                "invalid metadata",
                "duplicate item ID",
                "unknown metadata",
                "references missing item",
            ],
        ),
        (
            ":title: Partial\n\nBody without a close.\n",
            vec!["missing its closing delimiter", "duplicate item ID"],
        ),
    ] {
        let source = format!(":::mara requirement REQ-DUPLICATE\n{tail}");
        fs::write(root.join("second.mara.md"), &source).unwrap();
        let (_, diagnostics) = validate(root);
        for message in expected {
            assert!(
                diagnostics.iter().any(|d| d.message().contains(message)),
                "{message}: {diagnostics:?}"
            );
        }
        assert_eq!(
            fs::read_to_string(root.join("second.mara.md")).unwrap(),
            source
        );
    }
}

// @mara checks DES-CORPUS-CONFORMANCE
#[test]
fn malformed_or_unreadable_documents_suppress_unproven_absence_but_retain_ambiguity() {
    let fixture = fixture();
    let root = fixture.path();
    let source = item(
        "REQ-SOURCE",
        MID,
        ":depends_on: REQ-TARGET\n",
        "[[REQ-TARGET]]",
    );
    fs::write(root.join("source.mara.md"), source).unwrap();
    for broken in [
        b":::mara requirement REQ-TARGET trailing\n:title: Target\n\nBody.\n:::\n".as_slice(),
        &[0xff],
    ] {
        fs::write(root.join("target.mara.md"), broken).unwrap();
        let (corpus, diagnostics) = validate(root);
        assert!(!corpus.is_complete());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code(), DiagnosticCode::SourceInvalid);
        assert_eq!(fs::read(root.join("target.mara.md")).unwrap(), broken);
    }
    fs::write(
        root.join("duplicates.mara.md"),
        item("REQ-TARGET", "01ARZ3NDEKTSV4RRFFQ69G5F01", "", "Body.")
            + &item("REQ-TARGET", "01ARZ3NDEKTSV4RRFFQ69G5F02", "", "Body."),
    )
    .unwrap();
    let (_, diagnostics) = validate(root);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|d| d.code() == DiagnosticCode::ReferenceUnresolved)
            .count(),
        2,
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .filter(|d| d.code() == DiagnosticCode::ReferenceUnresolved)
            .all(|d| d.message().contains("ambiguous"))
    );
}

fn fields_schema(root: &Path) {
    let path = root.join(".mara/schema.yaml");
    let schema = fs::read_to_string(&path).unwrap().replace("    id_prefix: REQ-\n    body: required\n    fields: {}", "    id_prefix: REQ-\n    body: required\n    fields:\n      count: {type: integer, required: true}\n      score: {type: number}\n      active: {type: boolean}\n      state: {type: enum, values: [draft, accepted]}\n      tag: {type: string, repeatable: true}");
    fs::write(path, schema).unwrap();
}

// @mara checks REQ-PROJECT-VALIDATION
#[test]
fn field_types_required_values_repetition_and_body_checks_remain_independent() {
    let fixture = fixture();
    let root = fixture.path();
    fields_schema(root);
    let valid = ":count: -42\n:score: 1.25\n:active: false\n:state: draft\n:tag: first\n:tag: \n";
    fs::write(
        root.join("fields.mara.md"),
        item("REQ-FIELDS", MID, valid, "Body."),
    )
    .unwrap();
    assert!(validate(root).1.is_empty());
    for (metadata, body, errors) in [
        (":score: 3\n", "Body.", 1),
        (":count: 1\n:count: 2\n", "Body.", 1),
        (
            ":count: 1.2\n:score: NaN\n:active: yes\n:state: unknown\n",
            "",
            5,
        ),
        (":count: 9223372036854775808\n:score: inf\n", "Body.", 2),
    ] {
        let source = item("REQ-FIELDS", MID, metadata, body);
        fs::write(root.join("fields.mara.md"), &source).unwrap();
        let (_, diagnostics) = validate(root);
        assert_eq!(diagnostics.len(), errors, "{diagnostics:?}");
        assert!(
            diagnostics
                .iter()
                .all(|d| d.code() == DiagnosticCode::FieldInvalid)
        );
        assert_eq!(
            fs::read_to_string(root.join("fields.mara.md")).unwrap(),
            source
        );
    }
    fs::write(root.join("fields.mara.md"), format!(":::mara requirement REQ-FIELDS\n:mid: {MID}\n:title: Partial\n:malformed\n\nBody.\n:::\n")).unwrap();
    let (_, diagnostics) = validate(root);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::SourceInvalid);
}

// @mara checks DES-CORPUS-CONFORMANCE
#[test]
fn recovered_schema_properties_suppress_only_dependent_checks() {
    let fixture = fixture();
    let root = fixture.path();
    fields_schema(root);
    let schema_path = root.join(".mara/schema.yaml");
    let original = fs::read_to_string(&schema_path).unwrap();
    fs::write(
        root.join("invalid.mara.md"),
        item("WRONG-ID", MID, ":count: nope\n", ""),
    )
    .unwrap();
    for (before, after, expected) in [
        (
            "id_prefix: REQ-",
            "id_prefix: REQ--",
            vec![DiagnosticCode::FieldInvalid, DiagnosticCode::FieldInvalid],
        ),
        (
            "body: required",
            "body: invalid",
            vec![
                DiagnosticCode::IdentityInvalid,
                DiagnosticCode::FieldInvalid,
            ],
        ),
        (
            "type: integer",
            "type: nonsense",
            vec![
                DiagnosticCode::IdentityInvalid,
                DiagnosticCode::FieldInvalid,
            ],
        ),
    ] {
        fs::write(&schema_path, original.replace(before, after)).unwrap();
        let project = mara::resolve_project(Some(root), root).unwrap();
        let (schema, errors) = mara::load_schema_for_validation(&project).unwrap();
        assert!(!errors.is_empty());
        let (corpus, diagnostics) = mara::load_corpus_for_validation(&project, &schema).unwrap();
        assert!(diagnostics.is_empty());
        let diagnostics = mara::validate_corpus(&corpus, &schema);
        let mut codes: Vec<_> = diagnostics
            .iter()
            .map(|d| format!("{:?}", d.code()))
            .collect();
        let mut expected: Vec<_> = expected.into_iter().map(|c| format!("{c:?}")).collect();
        codes.sort();
        expected.sort();
        assert_eq!(codes, expected, "{diagnostics:?}");
    }
}

// @mara checks DES-CORPUS-CONFORMANCE
#[test]
fn syntax_only_recovery_avoids_broken_adapters_and_retains_identity_checks() {
    let fixture = fixture();
    let root = fixture.path();
    code_adapter(root);
    fs::write(root.join(".mara/code/rust.wasm"), "broken").unwrap();
    fs::write(
        root.join("source.mara.md"),
        ":::mara requirement REQ-A\n:title: A\n\n[[REQ-MISSING]]\n:::\n",
    )
    .unwrap();
    let project = mara::resolve_project(Some(root), root).unwrap();
    let (corpus, diagnostics) = mara::load_corpus_syntax_for_validation(&project).unwrap();
    assert!(corpus.is_complete());
    assert!(diagnostics.is_empty());
    let diagnostics = mara::validate_corpus_independent(&corpus);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::IdentityInvalid);
    assert_eq!(diagnostics[1].code(), DiagnosticCode::ReferenceUnresolved);
    assert!(!validate(root).1.is_empty());
}

// @mara checks REQ-PROJECT-VALIDATION
#[test]
fn item_inverse_inline_external_and_same_flavour_relations_validate_endpoints() {
    let fixture = fixture();
    let root = fixture.path();
    let path = root.join(".mara/schema.yaml");
    let schema = fs::read_to_string(&path).unwrap();
    fs::write(path, schema + "\n  follows:\n    description: Directed link\n    source: [requirement]\n    target: [scenario]\n    inverse: followed_by\n  similar:\n    description: Same kind\n    source: [requirement, scenario]\n    target: [requirement, scenario]\n    same_flavour: true\n  tracked_by:\n    description: Ticket\n    source: [requirement]\n    target: []\n    external: true\n").unwrap();
    let target_mid = "01ARZ3NDEKTSV4RRFFQ69G5F01";
    let target = format!(
        ":::mara scenario SCN-TARGET\n:mid: {target_mid}\n:title: Target\n:followed_by: REQ-SOURCE\n\nTarget.\n:::\n"
    );
    fs::write(root.join("target.mara.md"), target).unwrap();
    for target in ["SCN-TARGET", target_mid] {
        let metadata =
            format!(":follows: {target}\n:tracked_by: external:https://example.test/issue?q=x\n");
        fs::write(
            root.join("source.mara.md"),
            item(
                "REQ-SOURCE",
                MID,
                &metadata,
                &format!("[[follows:{target}]] [[tracked_by:external:https://example.test/other]]"),
            ),
        )
        .unwrap();
        assert!(validate(root).1.is_empty(), "{target}");
    }
    for (metadata, body) in [
        (":follows: REQ-SOURCE\n", "Body."),
        (":followed_by: SCN-TARGET\n", "Body."),
        (":similar: SCN-TARGET\n", "Body."),
        (":tracked_by: external:ftp://example.test/\n", "Body."),
        (":follows: external:https://example.test/\n", "Body."),
        ("", "[[followed_by:SCN-TARGET]]"),
        ("", "[[unknown:SCN-TARGET]]"),
    ] {
        let source = item("REQ-SOURCE", MID, metadata, body);
        fs::write(root.join("source.mara.md"), &source).unwrap();
        let (_, diagnostics) = validate(root);
        assert!(!diagnostics.is_empty());
        assert!(
            diagnostics
                .iter()
                .all(|d| d.code() == DiagnosticCode::RelationInvalid),
            "{diagnostics:?}"
        );
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            source
        );
    }
}

// @mara checks DES-CORPUS-CONFORMANCE
#[test]
fn code_endpoints_and_marker_permissions_have_stable_codes_and_target_association() {
    let fixture = fixture();
    let root = fixture.path();
    code_adapter(root);
    let source = "// @mara checks REQ-A\nfn run() {}\n";
    fs::write(root.join("source.rs"), source).unwrap();
    fs::write(root.join("binary"), [0xff]).unwrap();
    for endpoint in ["code:source.rs", "code:source.rs::run", "code:binary"] {
        fs::write(
            root.join("a.mara.md"),
            item("REQ-A", MID, &format!(":checked_by: {endpoint}\n"), "Body."),
        )
        .unwrap();
        assert!(validate(root).1.is_empty(), "{endpoint}");
    }
    for (endpoint, code) in [
        ("code:missing.rs", DiagnosticCode::CodeMissing),
        ("code:source.rs::missing", DiagnosticCode::CodeMissing),
        ("code:../outside.rs", DiagnosticCode::CodeUnsupported),
    ] {
        fs::write(
            root.join("a.mara.md"),
            item("REQ-A", MID, &format!(":checked_by: {endpoint}\n"), "Body."),
        )
        .unwrap();
        let (_, diagnostics) = validate(root);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code(), code);
    }
    fs::write(root.join("a.mara.md"), item("REQ-A", MID, "", "Body.")).unwrap();
    for relation in ["unknown", "checked_by", "derives_from"] {
        let code = format!("// @mara {relation} {MID}\nfn run() {{}}\n");
        fs::write(root.join("source.rs"), &code).unwrap();
        let (_, diagnostics) = validate(root);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code(), DiagnosticCode::RelationInvalid);
        assert!(diagnostic.applies_to_item("REQ-A"));
        assert!(diagnostic.applies_to_item(MID));
        assert_eq!(diagnostic.source().path(), Path::new("source.rs"));
        assert_eq!(fs::read_to_string(root.join("source.rs")).unwrap(), code);
    }
    fs::write(root.join("source.rs"), "fn run() {}\nfn run() {}\n").unwrap();
    fs::write(
        root.join("a.mara.md"),
        item("REQ-A", MID, ":checked_by: code:source.rs::run\n", "Body."),
    )
    .unwrap();
    let (_, diagnostics) = validate(root);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::CodeAmbiguous);
}

// @mara checks DES-CORPUS-CONFORMANCE
#[test]
fn recovered_code_problems_preserve_item_associations_and_incomplete_source() {
    let fixture = fixture();
    let root = fixture.path();
    code_adapter(root);
    fs::write(root.join("a.mara.md"), item("REQ-A", MID, "", "Body.")).unwrap();
    fs::write(root.join("bad.rs"), [0xff]).unwrap();
    fs::write(
        root.join("source.rs"),
        format!("// @mara checks {MID} extra\nfn run() {{}}\n"),
    )
    .unwrap();
    let (corpus, diagnostics) = validate(root);
    assert!(!corpus.is_complete());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == DiagnosticCode::SourceInvalid)
    );
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code() == DiagnosticCode::CodeUnsupported)
        .expect("invalid marker with identifiable target");
    assert!(diagnostic.applies_to_item("REQ-A"));
    assert!(diagnostic.applies_to_item(MID));
    assert_eq!(fs::read(root.join("bad.rs")).unwrap(), [0xff]);
}
