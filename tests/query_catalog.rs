//! Integrated witnesses from VER-REUSABLE-QUERY-ENGINE. Fixtures never enter the corpus.
#![cfg(any(target_os = "linux", target_os = "macos"))]
mod support;
use mara::{ClientOptions, ExecutionMode, NativeClient, SourceEngine, SourceSnapshot};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use support::*;

fn snapshot() -> SourceSnapshot {
    serde_json::from_str(include_str!("fixtures/query-catalog/snapshot.json")).unwrap()
}
fn content<'a>(s: &'a mut SourceSnapshot, path: &str) -> &'a mut String {
    &mut s.files.iter_mut().find(|f| f.path == path).unwrap().content
}
fn definitions(s: &mut SourceSnapshot, f: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_str(content(s, "queries.json")).unwrap();
    f(&mut v);
    *content(s, "queries.json") = serde_json::to_string_pretty(&v).unwrap();
}
fn query(id: &str, roots: &[&str]) -> Value {
    json!({"definition_files":["queries.json"],"definition":id,"selection":{"ids":roots},"parameters":[],"max_rows":1000,"limit":100,"cursor":null,"fact_snapshots":[],"mappings":[]})
}
fn text(v: &str) -> Value {
    json!({"type":"text","value":v})
}
fn integer(v: i64) -> Value {
    json!({"type":"integer","value":v})
}
fn param(name: &str, value: Value) -> Value {
    json!({"name":name,"value":value})
}
fn supports(roots: &[&str]) -> Value {
    let mut r = query("urn:fixture:query:supports", roots);
    r["parameters"] = json!([param("state", text(""))]);
    r
}
fn gate(id: &str, roots: &[&str], context: Value, facts: Value) -> Value {
    json!({"definition_files":["queries.json"],"gate_file":"gates.yaml","shape_files":["catalog.ttl"],"gate":id,"scope":{"ids":roots},"context":context,"fact_snapshots":facts,"mappings":[],"max_rows":1000})
}
fn key(v: &str, candidate: &str, context: &str) -> Value {
    json!({"verification":{"type":"item","value":v},"candidate":text(candidate),"context":text(context)})
}
fn facts(candidate: &str, context: &str, states: &[(&str, &str)], covered: &[&str]) -> Value {
    json!({"input":"reported","identity":"unchanged-label","coverage":covered.iter().map(|v|key(v,candidate,context)).collect::<Vec<_>>(),"records":states.iter().map(|(v,state)|json!({"keys":key(v,candidate,context),"fields":{"state":text(state)}})).collect::<Vec<_>>()})
}
fn candidate(f: Value) -> Value {
    gate(
        "candidate",
        &["CAND-C"],
        json!([
            param("candidate", text("CAND-C")),
            param("context", text("release"))
        ]),
        f,
    )
}
fn risk(likelihood: i64) -> Value {
    let mut r = query("urn:fixture:catalog:risk-score", &["CAND-C"]);
    r["parameters"] = json!([param("threshold", integer(6))]);
    r["mappings"] = json!([
        {"mapping":"likelihood","entries":[{"keys":{"likelihood_key":integer(2)},"value":integer(likelihood)}]},
        {"mapping":"consequence","entries":[{"keys":{"consequence_key":integer(3)},"value":integer(3)}]}
    ]);
    r
}
fn enabled(mut s: SourceSnapshot, severity: &str) -> SourceSnapshot {
    content(&mut s,".mara/project.toml").push_str("\n[query_policy]\nformat_version = 1\nfiles = [\"invariants.yaml\"]\ndefinition_files = [\"queries.json\"]\nshape_files = [\"catalog.ttl\"]\nmax_rows = 1000\n");
    *content(&mut s, "invariants.yaml") = content(&mut s, "invariants.yaml")
        .replace("severity: error", &format!("severity: {severity}"));
    s
}
// Hosts supply the same effective inventory. Unused source files are not implicit imports.
fn inputs(mut s: SourceSnapshot, operation: &str, params: &Value) -> SourceSnapshot {
    let policy = s
        .files
        .iter()
        .find(|f| f.path == ".mara/project.toml")
        .unwrap()
        .content
        .contains("[query_policy]");
    s.files.retain(|f| {
        f.path.ends_with(".mara.md")
            || f.path.starts_with(".mara/")
            || match operation {
                "query_execute" | "query_view" => f.path == "queries.json",
                "query_gate" => {
                    ["queries.json", "catalog.ttl", "gates.yaml"].contains(&f.path.as_str())
                }
                "query_report" => [
                    "queries.json",
                    "catalog.ttl",
                    "gates.yaml",
                    "invariants.yaml",
                ]
                .contains(&f.path.as_str()),
                _ => {
                    policy
                        && ["queries.json", "catalog.ttl", "invariants.yaml"]
                            .contains(&f.path.as_str())
                }
            }
    });
    if operation == "query_report" {
        s.files.retain(|f| {
            if params["subject"]["kind"] == "gate" {
                f.path != "invariants.yaml"
            } else {
                f.path != "gates.yaml"
            }
        });
    }
    s
}
fn normalize(mut v: Value) -> Value {
    // The physical project path is host context, not factual identity.
    if let Some(v) = v.get_mut("result").and_then(Value::as_object_mut) {
        v.remove("project");
    }
    v
}
fn portable(s: SourceSnapshot, op: &str, p: Value) -> Value {
    let e = SourceEngine::new(s).unwrap();
    macro_rules! result { ($r:expr) => { match $r { Ok(v)=>json!({"result":v}),Err(e)=>json!({"error":e}) } }; }
    normalize(match op {
        "query_execute" => result!(e.query_execute(serde_json::from_value(p).unwrap())),
        "query_view" => result!(e.query_view(serde_json::from_value(p).unwrap())),
        "query_gate" => result!(
            e.query_gate(serde_json::from_value(p).unwrap())
                .map_err(|e| json!({"code":e.code,"message":e.message}))
        ),
        "query_report" => {
            result!(
                e.query_report(
                    serde_json::from_value(p["subject"].clone()).unwrap(),
                    p["render"].as_str().map(str::to_owned)
                )
                .map_err(|e| json!({"code":e.code,"message":e.message}))
            )
        }
        "item_validate" => result!(e.item_validate(
            p["id"].as_str().unwrap(),
            &serde_json::from_value(p.clone()).unwrap(),
            None
        )),
        "project_validate" => {
            result!(e.project_validate(&[], &serde_json::from_value(p.clone()).unwrap(), None))
        }
        _ => panic!("unknown fixture operation"),
    })
}
fn native(c: &NativeClient, op: &str, p: Value) -> Value {
    macro_rules! result { ($r:expr) => { match $r { Ok(v)=>json!({"result":v}),Err(e)=>json!({"error":e.envelope()["error"]}) } }; }
    normalize(match op {
        "query_execute" => result!(c.query_execute(serde_json::from_value(p).unwrap())),
        "query_view" => result!(c.query_view(serde_json::from_value(p).unwrap())),
        "query_gate" => result!(c.query_gate(serde_json::from_value(p).unwrap())),
        "query_report" => {
            result!(c.query_report(
                serde_json::from_value(p["subject"].clone()).unwrap(),
                p["render"].as_str().map(str::to_owned)
            ))
        }
        "item_validate" => result!(c.item_validate_with_options(
            p["id"].as_str().unwrap(),
            &serde_json::from_value(p.clone()).unwrap()
        )),
        "project_validate" => result!(
            c.project_validate_with_options(&[], &serde_json::from_value(p.clone()).unwrap())
        ),
        _ => panic!("unknown fixture operation"),
    })
}
struct Host {
    directory: tempfile::TempDir,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<std::io::Result<()>>>,
}
impl Host {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("qc-")
            .tempdir_in(std::env::var_os("MARA_TEST_RUNTIME_DIR").unwrap())
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let server =
            mara::ipc::IpcServer::bind_in(directory.path(), mara::NativeSessions::default())
                .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        Self {
            directory,
            stop,
            worker: Some(thread::spawn(move || server.serve_until(&stopping))),
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap().unwrap();
    }
}
fn cli_args(root: &Path, op: &str, p: &Value) -> Vec<String> {
    let mut a = vec!["--format".into(), "json".into()];
    let input = match op {
        "query_view" => &p["request"],
        "query_report" => &p["subject"],
        _ => p,
    };
    let mut input_flags: Vec<String> = vec![];
    for (key, flag) in [
        ("fact_snapshots", "--fact-file"),
        ("mappings", "--mapping-file"),
    ] {
        if let Some(values) = input[key].as_array() {
            for (i, v) in values.iter().enumerate() {
                let f = format!("{key}-{i}.json");
                fs::write(root.join(&f), serde_json::to_vec(v).unwrap()).unwrap();
                input_flags.extend([flag.into(), f]);
            }
        }
    }
    let request = match op {
        "query_execute" | "query_view" => {
            a.extend([
                "query".into(),
                if op == "query_view" {
                    "view"
                } else {
                    "execute"
                }
                .into(),
            ]);
            if op == "query_view" { &p["request"] } else { p }
        }
        "query_gate" | "query_report"
            if p.get("subject").is_none() || p["subject"]["kind"] == "gate" =>
        {
            a.push("query".into());
            if op == "query_report" {
                a.extend(["report".into(), "gate".into()]);
                &p["subject"]
            } else {
                a.push("gate".into());
                p
            }
        }
        "query_report" => {
            a.extend([
                "query".into(),
                "report".into(),
                "rules".into(),
                "--id".into(),
                p["subject"]["target"]["id"].as_str().unwrap().into(),
            ]);
            a.extend(input_flags);
            return a;
        }
        "item_validate" => {
            a.extend([
                "item".into(),
                "validate".into(),
                p["id"].as_str().unwrap().into(),
                "--limit".into(),
                "100".into(),
            ]);
            a.extend(input_flags);
            return a;
        }
        "project_validate" => {
            a.extend([
                "project".into(),
                "validate".into(),
                "--limit".into(),
                "100".into(),
            ]);
            a.extend(input_flags);
            return a;
        }
        _ => panic!("operation"),
    };
    let is_gate = request.get("gate").is_some();
    a.push(
        request[if is_gate { "gate" } else { "definition" }]
            .as_str()
            .unwrap()
            .into(),
    );
    for f in request["definition_files"].as_array().unwrap() {
        a.extend(["--definition-file".into(), f.as_str().unwrap().into()]);
    }
    if is_gate {
        a.extend([
            "--gate-file".into(),
            request["gate_file"].as_str().unwrap().into(),
        ]);
        for f in request["shape_files"].as_array().unwrap() {
            a.extend(["--shape-file".into(), f.as_str().unwrap().into()]);
        }
    }
    let sel = &request[if is_gate { "scope" } else { "selection" }];
    if sel["all"] == true {
        a.push("--all".into());
    }
    for (key, flag) in [
        ("ids", "--id"),
        ("flavours", "--flavour"),
        ("paths", "--path"),
    ] {
        if let Some(values) = sel[key].as_array() {
            for v in values {
                a.extend([flag.into(), v.as_str().unwrap().into()]);
            }
        }
    }
    if let Some(values) = sel["fields"].as_array() {
        for v in values {
            a.extend([
                "--field".into(),
                format!(
                    "{}={}",
                    v["key"].as_str().unwrap(),
                    v["value"].as_str().unwrap()
                ),
            ]);
        }
    }
    for p in request[if is_gate { "context" } else { "parameters" }]
        .as_array()
        .unwrap()
    {
        a.extend([
            "--param".into(),
            format!("{}={}", p["name"].as_str().unwrap(), p["value"]),
        ]);
    }
    // Input files belong to this isolated fixture, not the source snapshot inventory.
    a.extend(input_flags);
    a.extend(["--max-rows".into(), request["max_rows"].to_string()]);
    if !is_gate {
        a.extend(["--limit".into(), request["limit"].to_string()]);
        if let Some(cursor) = request["cursor"].as_str() {
            a.extend(["--cursor".into(), cursor.into()]);
        }
    }
    a
}
#[derive(Default)]
struct Catalog {
    cases: Vec<Value>,
}
impl Catalog {
    fn run(
        &mut self,
        host: &Host,
        witness: u8,
        name: &str,
        s: SourceSnapshot,
        op: &str,
        p: Value,
    ) -> Value {
        let s = inputs(s, op, &p);
        let expected = portable(s.clone(), op, p.clone());
        let root = fixture();
        for file in &s.files {
            let path = root.path().join(&file.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, &file.content).unwrap();
        }
        for mode in [
            ExecutionMode::Direct,
            ExecutionMode::Auto,
            ExecutionMode::DaemonRequired,
        ] {
            let c = NativeClient::new(
                None,
                root.path().into(),
                ClientOptions {
                    mode: Some(mode),
                    daemon_directory: Some(host.directory.path().into()),
                },
            )
            .unwrap();
            assert_eq!(native(&c, op, p.clone()), expected, "{name}: {mode:?}");
        }
        for mode in ["direct", "daemon-required"] {
            let args = cli_args(root.path(), op, &p);
            let mut routed = vec![
                "--execution-mode".to_owned(),
                mode.into(),
                "--daemon-directory".into(),
                host.directory.path().to_string_lossy().into_owned(),
            ];
            routed.extend(args);
            let out = mara(
                root.path(),
                &routed.iter().map(String::as_str).collect::<Vec<_>>(),
            );
            let v: Value = serde_json::from_slice(&out.stdout)
                .unwrap_or_else(|_| panic!("{name}: {} {}", stdout(&out), stderr(&out)));
            let actual = if v.get("error").is_some() {
                json!({"error":v["error"]})
            } else {
                json!({"result":v})
            };
            let mut json_expected = expected.clone();
            // CLI JSON deliberately excludes presentation; human output is the same renderer.
            if let Some(markdown) = json_expected
                .get_mut("result")
                .and_then(Value::as_object_mut)
                .and_then(|r| r.remove("markdown"))
            {
                let format = routed.iter().position(|a| a == "--format").unwrap() + 1;
                routed[format] = "human".into();
                let human = mara(
                    root.path(),
                    &routed.iter().map(String::as_str).collect::<Vec<_>>(),
                );
                assert_eq!(
                    stdout(&human).trim_end(),
                    markdown.as_str().unwrap().trim_end(),
                    "{name}: CLI human {mode}"
                );
            }
            let successful = expected.get("error").is_none()
                && if expected["result"].get("valid").is_some() {
                    expected["result"]["valid"] == true
                } else {
                    expected["result"]["evaluation_complete"] != false
                };
            assert_eq!(
                out.status.success(),
                successful,
                "{name}: CLI status: {}",
                stderr(&out)
            );
            assert_eq!(normalize(actual), json_expected, "{name}: CLI {mode}");
            let responses = mcp_exchange_with_arguments(
                root.path(),
                &[
                    "--execution-mode",
                    mode,
                    "--daemon-directory",
                    host.directory.path().to_str().unwrap(),
                    "mcp",
                ],
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(
                        2,
                        op,
                        if op == "query_view" {
                            let mut flattened = p["request"].clone();
                            flattened["render"] = p["render"].clone();
                            flattened
                        } else {
                            p.clone()
                        },
                    ),
                ],
            );
            let r = &mcp_response(&responses, 2)["result"];
            let actual = if r["isError"] == true {
                json!({"error":r["structuredContent"]["error"]})
            } else {
                json!({"result":r["structuredContent"]})
            };
            assert_eq!(normalize(actual), expected, "{name}: MCP {mode}");
        }
        self.cases.push(json!({"witness":witness,"name":name,"snapshot":s,"operation":op,"params":p,"outcome":expected}));
        expected
    }
}
fn rows(v: &Value) -> Vec<&Value> {
    v["result"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r.get("binding"))
        .collect()
}
fn values(v: &Value, column: &str) -> Vec<Value> {
    rows(v)
        .iter()
        .map(|b| {
            let mut value = b["values"][column].clone();
            if let Some(object) = value.as_object_mut() {
                if let Some(kind) = object.remove("kind") {
                    object.insert("type".into(), kind);
                }
                if object.get("language") == Some(&Value::Null) {
                    object.remove("language");
                }
            }
            value
        })
        .collect()
}
fn count(v: &Value, n: usize) {
    assert_eq!(v["result"]["summary"]["binding_count"], n, "{v:#}");
}
fn state(v: &Value, expected: &str) {
    assert_eq!(v["result"]["state"], expected, "{v:#}");
}

// @mara checks REQ-QUERY-ENGINE-SURFACES
// @mara checks DES-QUERY-NAMED-ADAPTER
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
// @mara checks DES-QUERY-CONSUMER-CONTRACT
#[test]
fn twelve_discriminating_witnesses_on_real_native_paths() {
    let host = Host::new();
    let mut c = Catalog::default();
    let s = snapshot();
    // 1: one definition, four independent consumers, both linked and empty roots.
    let raw = c.run(
        &host,
        1,
        "four-consumers-facts",
        s.clone(),
        "query_execute",
        supports(&["SUB-LINKED", "SUB-EMPTY"]),
    );
    count(&raw, 3);
    let mut lifecycle = values(&raw, "lifecycle");
    lifecycle.sort_by_key(Value::to_string);
    assert_eq!(
        lifecycle,
        vec![
            json!({"type":"text","value":"accepted"}),
            json!({"type":"text","value":"draft"}),
            json!({"type":"text","value":"retired"})
        ]
    );
    let view = c.run(
        &host,
        1,
        "four-consumers-view",
        s.clone(),
        "query_view",
        json!({"request":supports(&["SUB-LINKED","SUB-EMPTY"]),"render":null}),
    );
    assert_eq!(
        view["result"]["input_identity"],
        raw["result"]["input_identity"]
    );
    assert_eq!(view["result"]["roots"].as_array().unwrap().len(), 2);
    for (root, valid) in [("SUB-LINKED", true), ("SUB-EMPTY", false)] {
        let v = c.run(
            &host,
            1,
            root,
            enabled(s.clone(), "error"),
            "item_validate",
            json!({"id":root,"limit":100}),
        );
        assert_eq!(v["result"]["valid"], valid);
        assert_eq!(v["result"]["evaluation_complete"], true);
    }
    let valid = c.run(
        &host,
        1,
        "disabled-invariant",
        s.clone(),
        "item_validate",
        json!({"id":"SUB-EMPTY","limit":100}),
    );
    assert_eq!(valid["result"]["valid"], true);
    let g = gate(
        "coverage",
        &["SUB-LINKED", "SUB-EMPTY"],
        json!([param("state", text(""))]),
        json!([]),
    );
    let v = c.run(&host, 1, "request-gate", s.clone(), "query_gate", g.clone());
    state(&v, "failed");
    let mut subject = g.clone();
    subject["kind"] = json!("gate");
    let report = c.run(
        &host,
        1,
        "gate-report",
        s.clone(),
        "query_report",
        json!({"subject":subject,"render":null}),
    );
    state(&report, "failed");
    // 2: strict resolution, declared arguments, actual returned types, aliases, empty selection.
    let mut r = supports(&["SUB-LINKED", "01ARZ3NDEKTSV4RRFFQ69G5F00"]);
    let v = c.run(
        &host,
        2,
        "id-mid-dedup",
        s.clone(),
        "query_execute",
        r.clone(),
    );
    assert_eq!(v["result"]["summary"]["selected_roots"], 1);
    let mut incompatible = s.clone();
    definitions(&mut incompatible, |v| {
        v["definitions"][0]["columns"]["amount"]["type"] = json!("boolean")
    });
    let v = c.run(
        &host,
        2,
        "no-match-no-static-inference",
        incompatible,
        "query_execute",
        supports(&["SUB-EMPTY"]),
    );
    count(&v, 0);
    for name in [
        "missing-param",
        "wrong-param",
        "undeclared-param",
        "unknown-id",
        "implicit-all",
        "unused-param",
        "duplicate-param",
    ] {
        r = supports(&["SUB-LINKED"]);
        match name {
            "missing-param" => r["parameters"] = json!([]),
            "wrong-param" => r["parameters"] = json!([param("state", integer(1))]),
            "undeclared-param" | "unused-param" => {
                r["parameters"] = json!([param("other", text(""))])
            }
            "unknown-id" => r["selection"] = json!({"ids":["SUB-NO"]}),
            "implicit-all" => r["selection"] = json!({}),
            _ => r["parameters"] = json!([param("state", text("")), param("state", text(""))]),
        }
        let v = c.run(&host, 2, name, s.clone(), "query_execute", r);
        assert_eq!(v["error"]["code"], "invalid_argument");
    }
    r = supports(&[]);
    r["selection"] = json!({"fields":[{"key":"status","value":"no-match"}]});
    let v = c.run(
        &host,
        2,
        "complete-empty-filter",
        s.clone(),
        "query_execute",
        r,
    );
    count(&v, 0);
    assert_eq!(v["result"]["evaluation_complete"], true);
    for name in [
        "duplicate-definition",
        "unknown-vocabulary",
        "unresolved-dependency",
        "recursive-dependency",
        "returned-wrong-type",
    ] {
        let mut changed = s.clone();
        definitions(&mut changed, |v| {
            let a = v["definitions"].as_array_mut().unwrap();
            match name {
                "duplicate-definition" => a.push(a[0].clone()),
                "unknown-vocabulary" => a[0]["root_flavours"] = json!(["unknown"]),
                "returned-wrong-type" => a[0]["columns"]["amount"]["type"] = json!("boolean"),
                _ => {
                    a[0]["dependencies"] = json!([{"name":"bad","definition":if name=="recursive-dependency"{"urn:fixture:query:supports"}else{"urn:missing"},"root":"root","projections":{}}])
                }
            }
        });
        let v = c.run(
            &host,
            2,
            name,
            changed,
            "query_execute",
            supports(&["SUB-LINKED"]),
        );
        assert_eq!(v["error"]["code"], "invalid_definition");
    }
    // 3: runtime primitive distinctions, explicit normalization, lifecycle context.
    let v = c.run(
        &host,
        3,
        "typed-absence",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:typed", &["SUP-A"]),
    );
    count(&v, 1);
    assert_eq!(values(&v, "empty"), vec![text("")]);
    assert_eq!(values(&v, "absent"), vec![Value::Null]);
    assert_eq!(
        values(&v, "text_int"),
        vec![json!({"type":"boolean","value":false})]
    );
    assert_eq!(
        values(&v, "int_bool"),
        vec![json!({"type":"boolean","value":false})]
    );
    let v = c.run(
        &host,
        3,
        "default-exact-collision",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:exact-collision", &["SUP-A", "SUP-C"]),
    );
    count(&v, 0);
    let v = c.run(
        &host,
        3,
        "explicit-nfc-casefold",
        s.clone(),
        "query_execute",
        query(
            "urn:fixture:query:title-alias-collisions",
            &["SUP-A", "SUP-C"],
        ),
    );
    count(&v, 2);
    let v = c.run(
        &host,
        3,
        "decomposition-supersession",
        s.clone(),
        "query_execute",
        query(
            "urn:fixture:query:decomposition-or-supersession",
            &["SUB-LINKED"],
        ),
    );
    count(&v, 3);
    // 4: distinct tuple counts vs distinct authored occurrences; standard joins and paging.
    let mut occurrence_inputs = s.clone();
    *content(&mut occurrence_inputs, "fixture.mara.md") = content(
        &mut occurrence_inputs,
        "fixture.mara.md",
    )
    .replacen(":supports: SUB-LINKED\n", "", 1);
    let v = c.run(
        &host,
        4,
        "occurrences",
        occurrence_inputs.clone(),
        "query_execute",
        query("urn:fixture:catalog:occurrences", &["SUB-LINKED"]),
    );
    count(&v, 2);
    let endpoint = c.run(
        &host,
        4,
        "endpoint-versus-two-occurrences",
        occurrence_inputs.clone(),
        "query_execute",
        supports(&["SUB-LINKED"]),
    );
    let matches = rows(&endpoint)
        .into_iter()
        .filter(|b| b["values"]["support"]["endpoint"]["id"] == "SUP-A")
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0]["support"][0]["occurrences"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        rows(&v)[0]["support"][0]["occurrences"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let v = c.run(
        &host,
        4,
        "occurrence-count",
        occurrence_inputs.clone(),
        "query_execute",
        query("urn:fixture:catalog:occurrence-count", &["SUB-LINKED"]),
    );
    assert_eq!(values(&v, "count"), vec![integer(2)]);
    let v = c.run(
        &host,
        4,
        "left-absence",
        s.clone(),
        "query_execute",
        query("urn:fixture:query:left-join-absence", &["SUB-EMPTY"]),
    );
    count(&v, 1);
    assert_eq!(values(&v, "support"), vec![Value::Null]);
    let v = c.run(
        &host,
        4,
        "inner-absence",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:inner-empty", &["SUB-EMPTY"]),
    );
    count(&v, 0);
    let v = c.run(
        &host,
        4,
        "distinct-alternative-endpoint",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:alternative-endpoints", &["SUB-LINKED"]),
    );
    count(&v, 1);
    assert_eq!(rows(&v)[0]["support"].as_array().unwrap().len(), 3);
    // 5: direct or accepted-design support. Removal must not invent a mandatory design.
    let r = query("urn:fixture:catalog:alternative", &["SUB-LINKED"]);
    let v = c.run(
        &host,
        5,
        "implementation-alternatives",
        s.clone(),
        "query_execute",
        r.clone(),
    );
    count(&v, 2);
    let mut changed = s.clone();
    *content(&mut changed, "fixture.mara.md") = content(&mut changed, "fixture.mara.md").replace(
        ":status: accepted\n:satisfies: SUB-LINKED",
        ":status: draft\n:satisfies: SUB-LINKED",
    );
    let v = c.run(
        &host,
        5,
        "unaccepted-design",
        changed,
        "query_execute",
        r.clone(),
    );
    count(&v, 1);
    assert_eq!(values(&v, "route"), vec![text("direct")]);
    let mut changed = s.clone();
    let docs = content(&mut changed, "fixture.mara.md");
    let start = docs.find(":::mara subject SUB-DESIGN").unwrap();
    let end = docs[start..].find("\n:::\n").unwrap() + start + 5;
    docs.replace_range(start..end, "");
    *docs = docs.replace(":implements: SUB-DESIGN\n", "");
    let v = c.run(
        &host,
        5,
        "removed-design-direct-survives",
        changed,
        "query_execute",
        r,
    );
    count(&v, 1);
    // 6: finite cycle/diamond ancestry and context, no root/workload expansion.
    let v = c.run(
        &host,
        6,
        "ancestry-context",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:ancestry-context", &["SUB-LINKED"]),
    );
    count(&v, 3);
    assert_eq!(v["result"]["summary"]["selected_roots"], 1);
    let ancestors = values(&v, "ancestor");
    assert!(
        ancestors
            .iter()
            .all(|v| !["SUB-LINKED", "SUB-S"].contains(&v["endpoint"]["id"].as_str().unwrap()))
    );
    let mut ids = ancestors
        .iter()
        .map(|v| v["endpoint"]["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(ids, ["SUB-A", "SUB-B", "SUB-G"]);
    let mut kinds = values(&v, "context_kind");
    kinds.sort_by_key(Value::to_string);
    assert_eq!(kinds, vec![text("goal"), text("need"), text("risk")]);
    let mut ids = values(&v, "context")
        .iter()
        .map(|v| v["endpoint"]["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(ids, ["CTX-GOAL", "CTX-NEED", "CTX-RISK"]);
    let v = c.run(
        &host,
        6,
        "explicit-self",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:include-self", &["SUB-LINKED"]),
    );
    count(&v, 4);
    let v = c.run(
        &host,
        6,
        "intentional-two-hop",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:two-hop", &["SUB-LINKED"]),
    );
    count(&v, 3);
    assert_eq!(v["result"]["evaluation_complete"], true);
    let mut r = query("urn:fixture:catalog:ancestry-context", &["SUB-LINKED"]);
    r["max_rows"] = json!(1);
    let v = c.run(&host, 6, "row-limit", s.clone(), "query_execute", r);
    // The backend must retain the same prefix on native and actual worker/WASM,
    // not just report matching incomplete metadata.
    assert_eq!(values(&v, "ancestor")[0]["endpoint"]["id"], "SUB-A");
    assert_eq!(v["result"]["evaluation_complete"], false);
    assert!(v["result"]["summary"]["binding_count"].is_null());
    let mut ancestry_gate = s.clone();
    content(&mut ancestry_gate,"gates.yaml").push_str("\n  - id: ancestry\n    empty_roots: failed\n    empty_required: failed\n    conditions:\n      - id: context\n        definition: urn:fixture:catalog:ancestry-context\n        shape: urn:fixture:shape:present\n        shape_file: catalog.ttl\n        severity: error\n");
    let mut g = gate("ancestry", &["SUB-LINKED"], json!([]), json!([]));
    g["max_rows"] = json!(1);
    let v = c.run(
        &host,
        6,
        "row-limit-cannot-pass-gate",
        ancestry_gate,
        "query_gate",
        g,
    );
    state(&v, "unavailable");
    assert_eq!(v["result"]["evaluation_complete"], false);
    let mut changed = s.clone();
    let mut schema: Value =
        serde_json::from_str(content(&mut changed, ".mara/schema.yaml")).unwrap();
    schema["relations"]["parent"]["acyclic"] = json!({"severity":"error"});
    *content(&mut changed, ".mara/schema.yaml") = schema.to_string();
    let v = c.run(
        &host,
        6,
        "independent-cycle-policy",
        changed,
        "project_validate",
        json!({"limit":100}),
    );
    assert_eq!(v["result"]["valid"], false);
    assert!(
        v["result"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["code"] == "relation_cycle")
    );
    // 7: accepted V1/V2, nonempty and passing, exact candidate coverage.
    for (name, f, expected) in [
        (
            "candidate-passes",
            json!([facts(
                "CAND-C",
                "release",
                &[("VER-V1", "pass"), ("VER-V2", "pass")],
                &["VER-V1", "VER-V2"]
            )]),
            "passed",
        ),
        (
            "candidate-covered-missing",
            json!([facts(
                "CAND-C",
                "release",
                &[("VER-V1", "pass")],
                &["VER-V1", "VER-V2"]
            )]),
            "failed",
        ),
        (
            "candidate-known-failure",
            json!([facts(
                "CAND-C",
                "release",
                &[("VER-V1", "pass"), ("VER-V2", "fail")],
                &["VER-V1", "VER-V2"]
            )]),
            "failed",
        ),
        (
            "candidate-partial",
            json!([facts(
                "CAND-C",
                "release",
                &[("VER-V1", "pass")],
                &["VER-V1"]
            )]),
            "unavailable",
        ),
        (
            "candidate-other",
            json!([facts(
                "D",
                "release",
                &[("VER-V1", "pass"), ("VER-V2", "pass")],
                &["VER-V1", "VER-V2"]
            )]),
            "unavailable",
        ),
        ("candidate-no-input", json!([]), "unavailable"),
    ] {
        let v = c.run(&host, 7, name, s.clone(), "query_gate", candidate(f));
        state(&v, expected);
    }
    let mut g = candidate(json!([facts("CAND-EMPTY", "release", &[], &[])]));
    g["scope"] = json!({"ids":["CAND-EMPTY"]});
    g["context"][0]["value"] = text("CAND-EMPTY");
    let v = c.run(
        &host,
        7,
        "candidate-empty-required",
        s.clone(),
        "query_gate",
        g,
    );
    state(&v, "failed");
    let mut changed = s.clone();
    *content(&mut changed, "gates.yaml") = content(&mut changed, "gates.yaml")
        .replace("urn:fixture:facts:verifications", "urn:missing");
    let v = c.run(
        &host,
        7,
        "missing-named-definition",
        changed,
        "query_gate",
        candidate(json!([])),
    );
    assert_eq!(v["error"]["code"], "invalid_definition");
    // 8: five domain states and a second context. Precedence is replaceable data.
    let members = ["VER-X1", "VER-X2", "VER-X3", "VER-X4", "VER-X5"];
    let mut f = facts(
        "CAND-E",
        "release",
        &[
            ("VER-X1", "pass"),
            ("VER-X2", "fail"),
            ("VER-X3", "inconclusive"),
            ("VER-X4", "error"),
            ("VER-X5", "pending"),
        ],
        &members,
    );
    let other = facts(
        "CAND-E",
        "nightly",
        &[
            ("VER-X1", "error"),
            ("VER-X2", "pass"),
            ("VER-X3", "pending"),
            ("VER-X4", "inconclusive"),
            ("VER-X5", "fail"),
        ],
        &members,
    );
    for k in ["coverage", "records"] {
        f[k].as_array_mut()
            .unwrap()
            .extend(other[k].as_array().unwrap().clone());
    }
    let mut r = query("urn:fixture:facts:validation", &["CAND-E"]);
    r["parameters"] = json!([
        param("candidate", text("CAND-E")),
        param("context", text("release"))
    ]);
    r["fact_snapshots"] = json!([f]);
    let v = c.run(
        &host,
        8,
        "selected-context-aggregate",
        s.clone(),
        "query_execute",
        r.clone(),
    );
    assert_eq!(values(&v, "state"), vec![text("error")]);
    assert_eq!(values(&v, "members"), vec![integer(5)]);
    let mut changed = s.clone();
    definitions(&mut changed, |v| {
        let d = v["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|d| d["id"] == "urn:fixture:facts:validation")
            .unwrap();
        d["aggregate"]["precedence"] = json!([
            text("pass"),
            text("pending"),
            text("inconclusive"),
            text("fail"),
            text("error"),
            text("missing")
        ]);
    });
    let v = c.run(
        &host,
        8,
        "replaced-precedence",
        changed,
        "query_execute",
        r.clone(),
    );
    assert_eq!(values(&v, "state"), vec![text("pass")]);
    for (name, covered, ctx, complete) in [
        ("context-covered-missing", members.to_vec(), "release", true),
        ("context-partial", vec!["VER-X1"], "release", false),
        ("context-other", members.to_vec(), "nightly", false),
    ] {
        let mut r = r.clone();
        r["fact_snapshots"] = json!([facts("CAND-E", ctx, &[], &covered)]);
        let v = c.run(&host, 8, name, s.clone(), "query_execute", r);
        assert_eq!(v["result"]["evaluation_complete"], complete);
        if complete {
            assert_eq!(values(&v, "state"), vec![text("missing")]);
        }
    }
    // 9: the definition computes the product/threshold; mappings only supply factors.
    for (name, l, score, above) in [("risk-six", 2, 6, true), ("risk-three", 1, 3, false)] {
        let v = c.run(&host, 9, name, s.clone(), "query_execute", risk(l));
        assert_eq!(values(&v, "score"), vec![integer(score)]);
        assert_eq!(
            values(&v, "above"),
            vec![json!({"type":"boolean","value":above})]
        );
    }
    let mut r = risk(2);
    r["mappings"][0]["entries"] = json!([]);
    let v = c.run(
        &host,
        9,
        "risk-unmapped",
        s.clone(),
        "query_execute",
        r.clone(),
    );
    assert_eq!(v["result"]["evaluation_complete"], false);
    r["mappings"][0]["fallback"] = integer(1);
    let v = c.run(
        &host,
        9,
        "risk-explicit-fallback",
        s.clone(),
        "query_execute",
        r,
    );
    assert_eq!(values(&v, "score"), vec![integer(3)]);
    let mut r = risk(2);
    let mut duplicate = r["mappings"][0]["entries"][0].clone();
    duplicate["value"] = integer(1);
    r["mappings"][0]["entries"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let v = c.run(
        &host,
        9,
        "risk-conflicting-key",
        s.clone(),
        "query_execute",
        r,
    );
    assert_eq!(v["error"]["code"], "invalid_argument");
    let mut changed = s.clone();
    content(&mut changed, "facts.mara.md").push_str("\n:::mara implementation IMP-RISK\n:mid: 01JQCAT0000000000000000030\n:title: Risk implementation\n:status: accepted\n:implements: CAND-C\n\nImplementation without a reassessment.\n:::\n");
    let v = c.run(
        &host,
        9,
        "risk-no-implicit-reassessment",
        changed,
        "query_execute",
        risk(2),
    );
    assert_eq!(values(&v, "score"), vec![integer(6)]);
    // The derived fact, not a frontend calculation, also drives each owning consumer.
    let mut risk_sources = enabled(s.clone(), "error");
    *content(&mut risk_sources,"invariants.yaml")=
        "format_version: 1\ninvariants:\n  - id: mapped-risk\n    definition: urn:fixture:catalog:risk-score\n    shape: urn:fixture:shape:above-threshold\n    flavours: [candidate]\n    paths: [facts.mara.md]\n    severity: error\n    parameters:\n      - name: threshold\n        value: {type: integer, value: 6}\n".into();
    content(&mut risk_sources,"catalog.ttl").push_str("\n<urn:fixture:shape:above-threshold> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1; sh:node [ sh:property [ sh:path <urn:mara:query:column:above>; sh:hasValue true ] ] ].\n");
    content(&mut risk_sources,"gates.yaml").push_str("\n  - id: risk\n    empty_roots: failed\n    empty_required: failed\n    conditions:\n      - id: threshold\n        definition: urn:fixture:catalog:risk-score\n        shape: urn:fixture:shape:above-threshold\n        shape_file: catalog.ttl\n        severity: error\n        context_parameters: [threshold]\n");
    for (factor, expected) in [(2, "passed"), (1, "failed")] {
        let r = risk(factor);
        let v = c.run(
            &host,
            9,
            &format!("risk-view-{factor}"),
            risk_sources.clone(),
            "query_view",
            json!({"request":r.clone(),"render":"markdown"}),
        );
        assert_eq!(
            v["result"]["roots"][0]["rows"][0]["values"]["score"]["value"],
            factor * 3
        );
        let v = c.run(
            &host,
            9,
            &format!("risk-invariant-{factor}"),
            risk_sources.clone(),
            "item_validate",
            json!({"id":"CAND-C","limit":100,"mappings":r["mappings"]}),
        );
        assert_eq!(
            v["result"]["query_policies"]["checks"][0]["state"],
            expected
        );
        assert_eq!(v["result"]["valid"], expected == "passed");
        let mut g = gate("risk", &["CAND-C"], r["parameters"].clone(), json!([]));
        g["mappings"] = r["mappings"].clone();
        let v = c.run(
            &host,
            9,
            &format!("risk-gate-{factor}"),
            risk_sources.clone(),
            "query_gate",
            g.clone(),
        );
        state(&v, expected);
        assert_eq!(
            v["result"]["roots"][0]["conditions"][0]["bindings"][0]["values"]["score"]["value"],
            factor * 3
        );
        g["kind"] = json!("gate");
        let v = c.run(
            &host,
            9,
            &format!("risk-report-{factor}"),
            risk_sources.clone(),
            "query_report",
            json!({"subject":g,"render":"markdown"}),
        );
        state(&v, expected);
        assert!(v["result"]["markdown"].as_str().unwrap().contains("score"));
    }
    // 10: empty mathematical facts and authored empty gate cases, severity and applicability.
    let v = c.run(
        &host,
        10,
        "empty-all-any-none",
        s.clone(),
        "query_execute",
        query("urn:fixture:catalog:empty-logic", &["SUB-EMPTY"]),
    );
    assert_eq!(
        values(&v, "all"),
        vec![json!({"type":"boolean","value":true})]
    );
    assert_eq!(
        values(&v, "any"),
        vec![json!({"type":"boolean","value":false})]
    );
    assert_eq!(
        values(&v, "none"),
        vec![json!({"type":"boolean","value":true})]
    );
    for outcome in ["passed", "failed", "not_applicable"] {
        for empty in ["roots", "required"] {
            let mut changed = s.clone();
            *content(&mut changed, "gates.yaml") = format!(
                "format_version: 1\ngates:\n  - id: empty\n    empty_roots: {outcome}\n    empty_required: {outcome}\n    conditions: []\n"
            );
            let mut g = gate("empty", &["SUB-EMPTY"], json!([]), json!([]));
            if empty == "roots" {
                g["scope"] = json!({"fields":[{"key":"status","value":"no-match"}]});
            }
            let v = c.run(
                &host,
                10,
                &format!("empty-{empty}-{outcome}"),
                changed,
                "query_gate",
                g,
            );
            state(&v, outcome);
        }
    }
    let mut changed = s.clone();
    *content(&mut changed, "gates.yaml") =
        "format_version: 1\ngates:\n  - id: empty\n    conditions: []\n".into();
    let v = c.run(
        &host,
        10,
        "unspecified-empty-invalid",
        changed,
        "query_gate",
        gate("empty", &["SUB-EMPTY"], json!([]), json!([])),
    );
    assert_eq!(v["error"]["code"], "invalid_definition");
    let v = c.run(
        &host,
        10,
        "warning-invariant",
        enabled(s.clone(), "warning"),
        "item_validate",
        json!({"id":"SUB-EMPTY","limit":100}),
    );
    assert_eq!(v["result"]["valid"], true);
    assert_eq!(
        v["result"]["query_policies"]["checks"][0]["state"],
        "failed"
    );
    let mut changed = s.clone();
    *content(&mut changed, "gates.yaml") = content(&mut changed, "gates.yaml").replace(
        "        context_parameters: [state]",
        "        context_parameters: [state]\n        applies_when: bindings_present",
    );
    let v = c.run(
        &host,
        10,
        "inactive-branch",
        changed,
        "query_gate",
        gate(
            "coverage",
            &["SUB-EMPTY"],
            json!([param("state", text(""))]),
            json!([]),
        ),
    );
    assert_eq!(
        v["result"]["roots"][0]["conditions"][0]["state"],
        "not_applicable"
    );
    let v = c.run(
        &host,
        10,
        "unavailable-applicability",
        s.clone(),
        "query_gate",
        candidate(json!([])),
    );
    state(&v, "unavailable");
    // Known failed and unknown required checks under the same candidate root.
    let mut changed = s.clone();
    definitions(&mut changed, |v| {
        let d = v["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|d| d["id"] == "urn:fixture:catalog:inner-empty")
            .unwrap();
        d["root_flavours"] = json!(["candidate"]);
    });
    let g = gate(
        "mixed",
        &["CAND-C"],
        json!([
            param("candidate", text("CAND-C")),
            param("context", text("release"))
        ]),
        json!([]),
    );
    let v = c.run(
        &host,
        10,
        "known-failure-beside-unavailable",
        changed,
        "query_gate",
        g,
    );
    state(&v, "unavailable");
    assert_eq!(v["result"]["roots"][0]["conditions"][0]["state"], "failed");
    assert_eq!(
        v["result"]["roots"][0]["conditions"][1]["state"],
        "unavailable"
    );
    // 11: page completion is not logical completeness; every relevant input invalidates reuse.
    let full = raw["result"]["records"].as_array().unwrap();
    let mut r = supports(&["SUB-LINKED", "SUB-EMPTY"]);
    r["limit"] = json!(1);
    let mut collected = vec![];
    let mut cursor = None;
    loop {
        let v = c.run(
            &host,
            11,
            &format!("frozen-page-{}", collected.len()),
            s.clone(),
            "query_execute",
            r.clone(),
        );
        assert_eq!(v["result"]["summary"]["binding_count"], 3);
        assert_eq!(v["result"]["evaluation_complete"], true);
        collected.extend(v["result"]["records"].as_array().unwrap().clone());
        if v["result"]["has_more"] != true {
            break;
        }
        if cursor.is_none() {
            cursor = Some(v["result"]["next_cursor"].clone());
        }
        r["cursor"] = v["result"]["next_cursor"].clone();
    }
    assert_eq!(&collected, full);
    for name in [
        "source-content",
        "source-inventory",
        "schema-content",
        "referenced-definition",
        "parameters",
        "roots",
    ] {
        let mut changed = s.clone();
        let mut r = supports(&["SUB-LINKED", "SUB-EMPTY"]);
        r["limit"] = json!(1);
        r["cursor"] = cursor.clone().unwrap();
        match name {
            "source-content" => content(&mut changed, "fixture.mara.md").push(' '),
            "source-inventory" => {
                changed.files.push(mara::SourceFile {
                    path: "new.mara.md".into(),
                    content: "Narrative.".into(),
                });
                changed.selection.document_paths.push("new.mara.md".into());
            }
            "schema-content" => content(&mut changed, ".mara/schema.yaml").push(' '),
            "referenced-definition" => content(&mut changed, "queries.json").push(' '),
            "parameters" => r["parameters"][0]["value"] = text("accepted"),
            _ => r["selection"] = json!({"ids":["SUB-LINKED"]}),
        }
        let v = c.run(&host, 11, name, changed, "query_execute", r);
        assert_eq!(v["error"]["code"], "invalid_argument");
    }
    let mut r = query("urn:fixture:facts:verifications", &["CAND-C"]);
    r["parameters"] = json!([
        param("candidate", text("CAND-C")),
        param("context", text("release"))
    ]);
    r["fact_snapshots"] = json!([facts(
        "CAND-C",
        "release",
        &[("VER-V1", "pass"), ("VER-V2", "pass")],
        &["VER-V1", "VER-V2"]
    )]);
    r["limit"] = json!(1);
    let v = c.run(
        &host,
        11,
        "fact-provenance-page",
        s.clone(),
        "query_execute",
        r.clone(),
    );
    let cursor = v["result"]["next_cursor"].clone();
    for name in [
        "fact-content-same-label",
        "coverage-same-label",
        "candidate-identity",
        "context-identity",
    ] {
        let mut r = r.clone();
        r["cursor"] = cursor.clone();
        match name {
            "fact-content-same-label" => {
                r["fact_snapshots"][0]["records"][0]["fields"]["state"] = text("fail")
            }
            "coverage-same-label" => {
                r["fact_snapshots"][0]["coverage"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            "candidate-identity" => r["parameters"][0]["value"] = text("D"),
            _ => r["parameters"][1]["value"] = text("nightly"),
        };
        let v = c.run(&host, 11, name, s.clone(), "query_execute", r);
        assert_eq!(v["error"]["code"], "invalid_argument");
    }
    let mut mapping_request = risk(2);
    mapping_request["limit"] = json!(1);
    let v = c.run(
        &host,
        11,
        "mapping-frozen-page",
        s.clone(),
        "query_execute",
        mapping_request.clone(),
    );
    mapping_request["cursor"] = v["result"]["next_cursor"].clone();
    mapping_request["mappings"][0]["entries"][0]["value"] = integer(1);
    let v = c.run(
        &host,
        11,
        "mapping-content-stale",
        s.clone(),
        "query_execute",
        mapping_request,
    );
    assert_eq!(v["error"]["code"], "invalid_argument");
    let mut covered = query("urn:fixture:facts:verifications", &["CAND-C"]);
    covered["parameters"] = json!([
        param("candidate", text("CAND-C")),
        param("context", text("release"))
    ]);
    covered["fact_snapshots"] = json!([facts(
        "CAND-C",
        "release",
        &[("VER-V1", "pass")],
        &["VER-V1", "VER-V2"]
    )]);
    let v = c.run(
        &host,
        11,
        "covered-record-and-absence-provenance",
        s.clone(),
        "query_execute",
        covered,
    );
    count(&v, 2);
    assert!(rows(&v).iter().any(|b| b["facts"][0]["record"] == 0));
    assert!(
        rows(&v).iter().any(
            |b| b["facts"][0]["record"].is_null() && b["values"]["state"]["value"] == "missing"
        )
    );
    let mut g = candidate(json!([]));
    g["scope"] = json!({"ids":["SUB-EMPTY"]});
    g["context"] = json!([]);
    let v = c.run(
        &host,
        10,
        "inactive-branch-needs-no-external-context",
        s.clone(),
        "query_gate",
        g,
    );
    assert_eq!(
        v["result"]["roots"][0]["conditions"][0]["state"],
        "not_applicable"
    );
    // 12: native-only input is unavailable on every document-only semantic path.
    let mut changed = s.clone();
    definitions(&mut changed, |v| {
        v["definitions"][0]["required_fact_sources"] = json!(["documents", "code"])
    });
    let v = c.run(
        &host,
        12,
        "required-code-unavailable",
        changed,
        "query_execute",
        supports(&["SUB-LINKED"]),
    );
    assert_eq!(v["result"]["evaluation_complete"], false);
    if let Some(path) = std::env::var_os("MARA_QUERY_CATALOG_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&c.cases).unwrap()).unwrap();
    }
    assert!(c.cases.len() > 60);
}
