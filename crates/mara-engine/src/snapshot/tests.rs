//! Tests of the snapshot.

use super::*;
use crate::document::Zone;
use crate::schema::parse_schema;

const A: &str = "01M7D8DZ2663WP3A037SR2HJH2";
const B: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";
const C: &str = "01M7D8DZ39AMW9NPW9NA17D4S4";
const MISSING: &str = "01M7D8DZ3MXVBN7QW52PEJCX4N";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n\
    [types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n\n\
    [relations.satisfies]\ninverse = \"satisfied_by\"\ndescription = \"Satisfies.\"\n\
    targets = { requirement = [\"requirement\"] }\n";

fn schema() -> Arc<Schema> {
    let (schema, diagnostics) = parse_schema(".mara/schema.toml", SCHEMA.as_bytes());
    assert_eq!(diagnostics, []);
    Arc::new(schema.expect("the schema loads"))
}

fn input(path: &str, text: &str) -> DocumentInput {
    let mut work = WorkCounts::default();
    let hash = Hash::of_file(text.as_bytes(), &mut work);
    DocumentInput {
        path: path.to_owned(),
        key: DocumentKey::new(hash),
        file: Arc::new(ParsedFile::parse(text.as_bytes().to_vec(), &mut work)),
    }
}

/// An item with `rows` after its `Type` row and `body` after its table.
fn item(mid: &str, title: &str, rows: &str, body: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### {title}\n\n| Type | Requirement |\n| --- | --- |\n{rows}\n{body}\n"
    )
}

/// `docs/a.mara.md`: narrative that mentions A, then item A with a relation to B, a row of an
/// undeclared relation and a relation whose fragment is no MID.
fn document_a() -> String {
    let rows = format!(
        "| ID | REQ-ALPHA |\n| Status | Draft |\n| Satisfies | [REQ-BETA](b.mara.md#{B}) |\n\
         | Owner | [Someone](b.mara.md#{B}) |\n| Satisfies | [Elsewhere](#not-a-mid) |\n"
    );
    let body = "Body with `code word`, a [link text](https://example.com/dest#frag) and\n\n\
                <!-- hidden comment -->\n";
    format!(
        "# Intro\n\nThe narrative mentions [it](#{A}).\n\n---\n\n{}",
        item(A, "Alpha heading", &rows, body)
    )
}

/// `docs/b.mara.md`: item B mentions A and a MID no item has; item C repeats A's ID.
fn document_b() -> String {
    let b = item(
        B,
        "Beta",
        "| ID | REQ-BETA |\n| Status | Draft |\n",
        &format!("Beta mentions [Alpha](a.mara.md#{A}) and [gone](a.mara.md#{MISSING})."),
    );
    let c = item(
        C,
        "Gamma",
        "| ID | REQ-ALPHA |\n| Status | Draft |\n",
        "Repeats an ID.",
    );
    format!("{b}\n---\n\n{c}")
}

fn build(documents: Vec<DocumentInput>, schema: Option<Arc<Schema>>) -> Snapshot {
    SnapshotInputs::new(documents, schema, None, None).build(&mut WorkCounts::default())
}

fn example() -> Snapshot {
    build(
        vec![
            input("docs/b.mara.md", &document_b()),
            input("docs/a.mara.md", &document_a()),
        ],
        Some(schema()),
    )
}

#[test]
fn holds_documents_by_path_and_items_in_source_order() {
    let snapshot = example();
    let paths: Vec<&str> = snapshot
        .documents()
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(paths, ["docs/a.mara.md", "docs/b.mara.md"]);
    assert_eq!(snapshot.document("docs/b.mara.md"), Some(1));
    assert_eq!(snapshot.document("docs/c.mara.md"), None);
    let mids: Vec<&str> = (0..snapshot.items().len())
        .map(|index| snapshot.item(index).mid.as_str())
        .collect();
    assert_eq!(mids, [A, B, C]);
    assert_eq!(snapshot.documents()[1].items, 1..3);
    assert_eq!(
        snapshot.items()[2],
        ItemEntry {
            document: 1,
            index: 1,
            edges: 4..4
        }
    );
}

#[test]
fn a_row_is_a_relation_edge_only_where_the_items_type_is_a_source() {
    let schema = format!(
        "{SCHEMA}\n[types.note]\nid_prefix = \"NOTE-\"\ndescription = \"A note.\"\n\n\
         [types.note.fields.satisfies]\nkind = \"text\"\n\n\
         [relations.implements]\ninverse = \"implemented_by\"\ndescription = \"Code.\"\n\
         targets = {{ code = [\"requirement\"] }}\n"
    );
    let (schema, diagnostics) = parse_schema(".mara/schema.toml", schema.as_bytes());
    assert_eq!(diagnostics, []);
    let link = format!("| Satisfies | [REQ-BETA](#{B}) |\n| Implements | [REQ-BETA](#{B}) |\n");
    let typed = |mid: &str, kind: &str| {
        item(mid, "Typed", "", "")
            .replace("| Type | Requirement |", &format!("| Type | {kind} |"))
            .replace("| --- | --- |\n", &format!("| --- | --- |\n{link}"))
    };
    let text = [typed(A, "Requirement"), typed(B, "Note"), typed(C, "Code")].join("\n---\n\n");
    let snapshot = build(
        vec![input("a.mara.md", &text)],
        Some(Arc::new(schema.unwrap())),
    );
    let relations: Vec<Vec<Option<&str>>> = (0..3)
        .map(|item| {
            let outgoing = snapshot.outgoing(item).iter();
            outgoing.map(|edge| snapshot.relation(edge)).collect()
        })
        .collect();
    // A requirement is a source of `satisfies`; a note has a field of that name; an item of
    // type `Code` is of no declared type, and the `code` source stands for code markers.
    assert_eq!(relations, [vec![Some("satisfies")], vec![], vec![]]);
}

#[test]
fn maps_mids_and_ids_to_every_item_that_has_them() {
    let snapshot = example();
    assert_eq!(snapshot.items_with_mid(A), [0]);
    assert_eq!(snapshot.items_with_mid(C), [2]);
    assert_eq!(snapshot.items_with_mid(MISSING), [] as [usize; 0]);
    assert_eq!(snapshot.items_with_id("REQ-ALPHA"), [0, 2]);
    assert_eq!(snapshot.items_with_id("REQ-BETA"), [1]);
    assert_eq!(snapshot.items_with_id("REQ-GAMMA"), [] as [usize; 0]);
    // An item that repeats its `ID` row is held once, under each of its values.
    let rows = "| ID | REQ-A |\n| ID | REQ-B |\n| ID | REQ-A |\n| Status | Draft |\n";
    let snapshot = build(vec![input("a.mara.md", &item(A, "A", rows, "A."))], None);
    assert_eq!(snapshot.items_with_id("REQ-A"), [0]);
    assert_eq!(snapshot.items_with_id("REQ-B"), [0]);
}

#[test]
fn links_relations_and_mentions_both_ways() {
    let snapshot = example();
    let described = |edge: &Edge| {
        let link = snapshot.link(edge);
        (
            edge.item,
            snapshot.relation(edge),
            link.text.clone(),
            link.fragment.clone().unwrap_or_default(),
        )
    };
    // Only the row of the declared relation whose link targets a MID is an edge.
    let outgoing: Vec<_> = snapshot.outgoing(0).iter().map(described).collect();
    assert_eq!(
        outgoing,
        [(Some(0), Some("satisfies"), "REQ-BETA".into(), B.into())]
    );
    assert_eq!(snapshot.outgoing(0)[0].source, EdgeSource::Row(3));
    let outgoing: Vec<_> = snapshot.outgoing(1).iter().map(described).collect();
    assert_eq!(
        outgoing,
        [
            (Some(1), None, "Alpha".into(), A.into()),
            (Some(1), None, "gone".into(), MISSING.into())
        ]
    );
    let narrative: Vec<_> = snapshot.narrative(0).iter().map(described).collect();
    assert_eq!(narrative, [(None, None, "it".into(), A.into())]);
    assert_eq!(snapshot.narrative(1), []);

    let incoming = |mid: &str| -> Vec<_> { snapshot.incoming(mid).map(described).collect() };
    assert_eq!(
        incoming(A),
        [
            (None, None, "it".into(), A.into()),
            (Some(1), None, "Alpha".into(), A.into())
        ]
    );
    assert_eq!(
        incoming(B),
        [(Some(0), Some("satisfies"), "REQ-BETA".into(), B.into())]
    );
    // A target no item has keeps its edges, so validation can report them.
    assert_eq!(incoming(MISSING).len(), 1);
    assert_eq!(incoming(C), []);
}

#[test]
fn without_a_schema_only_mentions_are_edges() {
    let snapshot = build(vec![input("docs/a.mara.md", &document_a())], None);
    assert_eq!(snapshot.outgoing(0), []);
    assert_eq!(snapshot.narrative(0).len(), 1);
    assert!(snapshot.schema().is_none());
}

#[test]
fn indexes_the_words_of_each_item_by_zone() {
    let snapshot = example();
    let postings = |word: &str| -> Vec<(usize, Zone)> {
        snapshot
            .postings(word)
            .iter()
            .map(|posting| (posting.item, posting.zone))
            .collect()
    };
    assert_eq!(
        postings("alpha"),
        [
            (0, Zone::Id),
            (0, Zone::Title),
            (1, Zone::Body),
            (2, Zone::Id)
        ]
    );
    assert_eq!(
        postings("beta"),
        [
            (0, Zone::Metadata),
            (1, Zone::Id),
            (1, Zone::Title),
            (1, Zone::Body)
        ]
    );
    assert_eq!(postings(&A.to_lowercase()), [(0, Zone::Id)]);
    assert_eq!(
        postings("requirement"),
        [
            (0, Zone::Metadata),
            (1, Zone::Metadata),
            (2, Zone::Metadata)
        ]
    );
    assert_eq!(postings("someone"), [(0, Zone::Metadata)]);
    assert_eq!(postings("code"), [(0, Zone::Body)]);
    assert_eq!(postings("link"), [(0, Zone::Body)]);
    // Link destinations, HTML and the narrative are not item text.
    for word in ["dest", "example", "hidden", "comment", "narrative", "intro"] {
        assert_eq!(postings(word), [], "{word}");
    }
    assert_eq!(postings("Alpha"), [], "words are looked up normalized");
}

#[test]
fn counts_one_build_and_no_parse() {
    let documents = vec![input("docs/a.mara.md", &document_a())];
    let mut work = WorkCounts::default();
    SnapshotInputs::new(documents, Some(schema()), None, None).build(&mut work);
    assert_eq!(
        work,
        WorkCounts {
            snapshot_builds: 1,
            ..WorkCounts::default()
        }
    );
}

#[test]
fn the_key_covers_paths_document_keys_schema_and_configuration() {
    let hash = |text: &str| Hash::of_file(text.as_bytes(), &mut WorkCounts::default());
    let (schema_hash, config_hash) = (Some(hash("schema")), Some(hash("config")));
    let key = |documents: Vec<DocumentInput>, schema_hash, config_hash| {
        SnapshotInputs::new(documents, None, schema_hash, config_hash).key()
    };
    let (a, b) = (document_a(), document_b());
    let base = key(
        vec![input("a.mara.md", &a), input("b.mara.md", &b)],
        schema_hash,
        config_hash,
    );
    let same = key(
        vec![input("b.mara.md", &b), input("a.mara.md", &a)],
        schema_hash,
        config_hash,
    );
    assert_eq!(base, same, "the order of the documents given");
    let other_keys = [
        key(
            vec![input("a.mara.md", &a), input("c.mara.md", &b)],
            schema_hash,
            config_hash,
        ),
        key(
            vec![input("a.mara.md", &a), input("b.mara.md", &a)],
            schema_hash,
            config_hash,
        ),
        key(vec![input("a.mara.md", &a)], schema_hash, config_hash),
        key(
            vec![input("a.mara.md", &a), input("b.mara.md", &b)],
            Some(hash("other")),
            config_hash,
        ),
        key(
            vec![input("a.mara.md", &a), input("b.mara.md", &b)],
            schema_hash,
            Some(hash("other")),
        ),
        key(
            vec![input("a.mara.md", &a), input("b.mara.md", &b)],
            None,
            config_hash,
        ),
        key(
            vec![input("a.mara.md", &a), input("b.mara.md", &b)],
            config_hash,
            schema_hash,
        ),
    ];
    for (index, other) in other_keys.iter().enumerate() {
        assert_ne!(*other, base, "change {index}");
    }
}

/// Item spans stay byte offsets into the file, whatever its line endings (DES-REGIONS).
#[test]
fn item_spans_index_the_original_contents() {
    for ending in ["\n", "\r\n", "\r"] {
        let text = document_b().replace('\n', ending);
        let snapshot = build(vec![input("docs/b.mara.md", &text)], None);
        let contents = &snapshot.documents()[0].file.contents;
        for index in 0..snapshot.items().len() {
            let item = snapshot.item(index);
            let source = std::str::from_utf8(&contents[item.span.clone()]).unwrap();
            assert!(
                source.starts_with(&format!("<a id=\"{}\"></a>{ending}", item.mid)),
                "{source:?}"
            );
            let body = std::str::from_utf8(&contents[item.body.clone()]).unwrap();
            assert!(source.ends_with(body), "{body:?} ends {source:?}");
        }
        let body = std::str::from_utf8(&contents[snapshot.item(1).body.clone()]).unwrap();
        assert_eq!(body, "Repeats an ID.");
    }
}
