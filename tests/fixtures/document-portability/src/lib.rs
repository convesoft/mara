//! Dependency gate only, not an extracted Mara API or browser product.
//! engine.rs and bindings.rs are copied unchanged from the tested checkout.
#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};

use rudof_iri::IriS;
use rudof_rdf::{
    rdf_core::{RDFFormat, term::Object},
    rdf_impl::{OxigraphInMemory, ReaderMode},
};
use serde_json::{Value, json};
use shacl::{
    ir::IRSchema,
    validator::{engine::Validate, nodes::FocusNodes, report::ValidationOutcome},
};

const SH: &str = "http://www.w3.org/ns/shacl#";
const FIELD: &str = "urn:mara:field:";
const REL: &str = "urn:mara:relation:";
const FLAVOUR: &str = "urn:mara:flavour:";
const RULE: &str = "urn:mara:rule:";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
#[allow(dead_code)]
mod bindings;
mod engine;

#[cfg(target_family = "wasm")]
use wasm_bindgen_test::*;
#[cfg(target_family = "wasm")]
wasm_bindgen_test_configure!(run_in_browser);

fn schema() -> Value {
    json!({"flavours": {
        "requirement": {"fields": {"status": {}, "title": {}, "integer": {}, "double": {}, "boolean": {}, "revision": {}}},
        "verification": {"fields": {"status": {}, "revision": {}}}
    }, "relations": {"verifies": {}}})
}

fn shapes(yaml: &str) -> IRSchema {
    // Named SHACL mappings use the same schema-generated JSON-LD context as Mara.
    // Definition validation/source recovery remain covered by the native suites.
    let graph: Value = serde_saphyr::from_str(yaml).unwrap();
    IRSchema::from_str(
        &json!({"@context": bindings::context(&schema()), "@graph": graph}).to_string(),
        &RDFFormat::JsonLd,
        None,
        &ReaderMode::Strict,
    )
    .unwrap()
}

fn data() -> OxigraphInMemory {
    // Same expanded JSON-LD typed data representation as rules/evaluate.rs.
    OxigraphInMemory::from_str(
        &json!({"@graph": [
            {"@id": "urn:test:req", "@type": format!("{FLAVOUR}requirement"),
             format!("{FIELD}status"): "accepted", format!("{FIELD}title"): "Unicode café 🦀",
             format!("{FIELD}integer"): 7,
             format!("{FIELD}double"): {"@value": 1.5, "@type": format!("{XSD}double")},
             format!("{FIELD}boolean"): true, format!("{FIELD}revision"): "abc123"},
            {"@id": "urn:test:ver", "@type": format!("{FLAVOUR}verification"),
             format!("{FIELD}status"): "accepted", format!("{FIELD}revision"): "abc123",
             format!("{REL}verifies"): {"@id": "urn:test:req"}},
            {"@id": "urn:test:draft", "@type": format!("{FLAVOUR}verification"),
             format!("{FIELD}status"): "draft", format!("{REL}verifies"): {"@id": "urn:test:req"}}
        ]})
        .to_string(),
        &RDFFormat::JsonLd,
        None,
        &ReaderMode::Strict,
    )
    .unwrap()
}

fn run(ir: &IRSchema, id: &str, focus: &str) -> (ValidationOutcome, engine::RuleEngine) {
    let shape = ir
        .get_shape(&Object::iri(IriS::new(&format!("{RULE}{id}")).unwrap()))
        .unwrap();
    let mut engine = engine::RuleEngine::new();
    let outcome = shape
        .validate(
            &data(),
            &mut engine,
            Some(&FocusNodes::single(
                Object::iri(IriS::new(focus).unwrap()).into(),
            )),
            None,
            ir,
        )
        .unwrap();
    (outcome, engine)
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), test)]
fn nested_qualified_literals_and_diagnostics() {
    let ir = shapes(
        r#"
- {id: rule:root, type: NodeShape, class: requirement, property: [rule:status, rule:title, rule:int, rule:double, rule:bool, rule:coverage], and: [rule:logic]}
- {id: rule:status, type: PropertyShape, path: status, minCount: 1, maxCount: 1, hasValue: accepted, in: [draft, accepted], datatype: string}
- {id: rule:title, type: PropertyShape, path: title, pattern: '^Unicode café', node: rule:text}
- {id: rule:text, type: NodeShape, datatype: string}
- {id: rule:int, type: PropertyShape, path: integer, datatype: integer, hasValue: {value: 7, datatype: integer}}
- {id: rule:double, type: PropertyShape, path: double, datatype: double, hasValue: {value: 1.5, datatype: double}}
- {id: rule:bool, type: PropertyShape, path: boolean, datatype: boolean, hasValue: {value: true, datatype: boolean}}
- {id: rule:logic, type: NodeShape, or: [rule:accepted, rule:retired], not: rule:retired}
- {id: rule:accepted, type: NodeShape, property: [rule:status]}
- {id: rule:retired, type: NodeShape, property: [rule:retired_status]}
- {id: rule:retired_status, type: PropertyShape, path: status, hasValue: retired}
- {id: rule:coverage, type: PropertyShape, path: {inversePath: verifies}, qualifiedValueShape: rule:qualified, qualifiedMinCount: 1, qualifiedMaxCount: 1}
- {id: rule:qualified, type: NodeShape, class: verification, property: [rule:status, rule:hop]}
- {id: rule:hop, type: PropertyShape, path: verifies, node: rule:second_hop}
- {id: rule:second_hop, type: NodeShape, class: requirement, property: [rule:status]}
- {id: rule:fail, type: PropertyShape, path: {inversePath: verifies}, qualifiedValueShape: rule:qualified, qualifiedMinCount: 2}
- {id: rule:every, type: PropertyShape, path: {inversePath: verifies}, node: rule:qualified}
"#,
    );
    let (pass, engine) = run(&ir, "root", "urn:test:req");
    assert!(pass.conforms());
    assert!(!engine.failed.get());
    assert_eq!(
        engine.counts[&(
            format!("{RULE}coverage"),
            "urn:test:req".into(),
            format!("{SH}QualifiedMinCountConstraintComponent")
        )],
        (2, Some(1))
    );
    for (shape, component) in [("fail", "QualifiedMinCount"), ("every", "Node")] {
        let (fail, engine) = run(&ir, shape, "urn:test:req");
        assert!(!fail.conforms());
        assert!(!engine.failed.get());
        assert!(fail.violations().iter().any(|v| {
            v.constraint_component().to_string() == format!("{SH}{component}ConstraintComponent")
                && v.source()
                    .is_some_and(|s| s.to_string() == format!("{RULE}{shape}"))
        }));
    }
    let (condition, engine) = run(&ir, "accepted", "urn:test:draft");
    assert!(!condition.conforms()); // Mara maps this whenShape result to not_applicable.
    assert!(!engine.failed.get());
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), test)]
fn exact_parameter_binding_reaches_real_engine() {
    for (text, expected) in [
        ("abc123", true),
        ("other", false),
        ("", false),
        ("rule:abc123", false),
    ] {
        let mut graph: Value = serde_saphyr::from_str(r#"
- {id: rule:revision, type: PropertyShape, path: revision, hasValue: {parameter: subject_revision}, in: [{parameter: subject_revision}]}
"#).unwrap();
        let mut used = BTreeSet::new();
        let params = BTreeMap::from([("subject_revision".into(), text.into())]);
        bindings::bind_literal(&mut graph[0]["hasValue"], &params, &mut used).unwrap();
        bindings::bind_literal(&mut graph[0]["in"][0], &params, &mut used).unwrap();
        assert_eq!(used, BTreeSet::from(["subject_revision".into()]));
        let ir = IRSchema::from_str(
            &json!({"@context": bindings::context(&schema()), "@graph": graph}).to_string(),
            &RDFFormat::JsonLd,
            None,
            &ReaderMode::Strict,
        )
        .unwrap();
        let (outcome, engine) = run(&ir, "revision", "urn:test:req");
        assert_eq!(outcome.conforms(), expected);
        assert!(!engine.failed.get());
    }
    assert!(
        bindings::bind_literal(
            &mut json!({"parameter": "missing"}),
            &BTreeMap::new(),
            &mut BTreeSet::new()
        )
        .is_err()
    );
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), test)]
fn nested_engine_errors_are_not_successful_coverage() {
    // Deliberately bypass definition validation to provoke the same unsupported
    // native constraint as the production adapter's error-ledger regression.
    for obligation in [
        "sh:not [ sh:minLength 1 ]",
        "sh:property [ sh:path <urn:mara:field:status>; sh:qualifiedValueShape [ sh:minLength 1 ]; sh:qualifiedMaxCount 0 ]",
    ] {
        let ir = IRSchema::from_str(
            &format!("@prefix sh: <{SH}> . <{RULE}error> a sh:NodeShape; {obligation} ."),
            &RDFFormat::Turtle,
            None,
            &ReaderMode::Strict,
        )
        .unwrap();
        let (outcome, engine) = run(&ir, "error", "urn:test:req");
        assert!(outcome.conforms()); // Upstream swallowed the unsupported error.
        assert!(engine.failed.get()); // Mara must classify it unavailable.
    }
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), test)]
fn document_dependencies_and_host_supplied_identity() {
    use rushdown::{
        parser::{self, Parser},
        text::BasicReader,
    };
    use sha2::{Digest, Sha256};
    use unicode_casefold::UnicodeCaseFold;
    use unicode_normalization::UnicodeNormalization;
    use unicode_segmentation::UnicodeSegmentation;

    let source = "# Café 🦀\n\nUnicode **source**.\n";
    let parser = Parser::with_extensions(
        parser::Options::default(),
        parser::EmptyParserExtension::new(),
    );
    let (_, _) = parser.parse(&mut BasicReader::new(source));
    assert_eq!(Sha256::digest(source.as_bytes()).len(), 32);
    assert!(similar::TextDiff::from_chars(source, "changed").ratio() < 1.0);
    assert_eq!(
        schemars::schema_for!(BTreeMap<String, String>).to_value()["type"],
        "object"
    );
    let config: toml::Value = toml::from_str("format_version = 3\n").unwrap();
    assert_eq!(config["format_version"].as_integer(), Some(3));
    let matcher = globset::GlobBuilder::new("**/*.mara.md")
        .literal_separator(true)
        .build()
        .unwrap()
        .compile_matcher();
    assert!(matcher.is_match("docs/new.mara.md"));
    let mut ignore = ignore::gitignore::GitignoreBuilder::new("/workspace");
    ignore.add_line(None, "excluded/*.mara.md").unwrap();
    ignore.add_line(None, "!excluded/kept.mara.md").unwrap();
    let ignore = ignore.build().unwrap();
    assert!(ignore.matched("excluded/new.mara.md", false).is_ignore());
    assert!(
        ignore
            .matched("excluded/kept.mara.md", false)
            .is_whitelist()
    );
    assert!(ignore.matched("docs/new.mara.md", false).is_none());

    // Ulid::new uses SystemTime. WASM supplies browser time/entropy explicitly;
    // native keeps its normal host services. The canonical ULID format is shared.
    #[cfg(target_family = "wasm")]
    let mid = {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).unwrap();
        ulid::Ulid::from_parts(js_sys::Date::now() as u64, u128::from_be_bytes(bytes))
    };
    #[cfg(not(target_family = "wasm"))]
    let mid = ulid::Ulid::new();
    assert_eq!(mid.to_string().parse::<ulid::Ulid>().unwrap(), mid);
    assert_eq!(mid.to_string().len(), 26);
    assert_eq!("CAFÉ".case_fold().collect::<String>(), "café");
    assert_eq!("Cafe\u{301}".nfc().collect::<String>(), "Café");
    assert_eq!("🦀".graphemes(true).count(), 1);
    assert!(strsim::normalized_levenshtein("requirement", "requiremnt") > 0.8);
    assert!(url::Url::parse("https://example.test/path").is_ok());
    let mut graph = petgraph::graph::DiGraph::<(), ()>::new();
    let a = graph.add_node(());
    graph.add_edge(a, a, ());
    assert_eq!(petgraph::algo::kosaraju_scc(&graph), vec![vec![a]]);
}
