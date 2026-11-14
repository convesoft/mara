//! Tests of the read operations below every host.

use std::sync::Arc;

use super::distance::within;
use super::*;
use crate::document::DocumentKey;
use crate::hash::Hash;
use crate::paging::{DEFAULT_LIMIT, PAGE_BYTES};
use crate::schema::{Schema, parse_schema};
use crate::snapshot::edits_allowed;
use crate::snapshot::{DocumentInput, ParsedFile, SnapshotInputs};
use crate::work::WorkCounts;

const A: &str = "01M7D8DZ2663WP3A037SR2HJH2";
const B: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";
const C: &str = "01M7D8DZ39AMW9NPW9NA17D4S4";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\", \"accepted\"]\n\n\
    [types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n\n\
    [types.requirement.fields.priority]\nkind = \"enum\"\nvalues = [\"high\", \"low\"]\n\n\
    [types.requirement.fields.owner]\nkind = \"text\"\nrequired = true\nrepeatable = true\n\n\
    [relations.satisfies]\ninverse = \"satisfied_by\"\ndescription = \"Satisfies.\"\n\
    targets = { requirement = [\"requirement\"], code = [\"requirement\"] }\nacyclic = true\n";

fn schema() -> Arc<Schema> {
    let (schema, diagnostics) = parse_schema(".mara/schema.toml", SCHEMA.as_bytes());
    assert_eq!(diagnostics, []);
    Arc::new(schema.expect("the schema loads"))
}

fn item(mid: &str, id: &str, title: &str, rows: &str, body: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### {title}\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | {id} |\n| Status | Draft |\n{rows}\n{body}\n"
    )
}

fn snapshot(documents: &[(&str, String)]) -> Snapshot {
    let inputs = documents
        .iter()
        .map(|(path, text)| {
            let mut work = WorkCounts::default();
            DocumentInput {
                path: (*path).to_owned(),
                key: DocumentKey::new(Hash::of_file(text.as_bytes(), &mut work)),
                file: Arc::new(ParsedFile::parse(text.as_bytes().to_vec(), &mut work)),
            }
        })
        .collect();
    SnapshotInputs::new(inputs, Some(schema()), None, None).build(&mut WorkCounts::default())
}

/// `docs/a.mara.md`: narrative, then item A (REQ-ALPHA), which satisfies B, and item C.
/// `docs/b.mara.md`: item B (REQ-BETA), and narrative that mentions A.
fn example() -> Snapshot {
    let a = format!(
        "# Overview\n\nGlossary of zebras.\n\n---\n\n{}\n---\n\n{}",
        item(
            A,
            "REQ-ALPHA",
            "Alpha caching",
            &format!(
                "| Satisfies | [REQ-BETA](b.mara.md#{B}) |\n| Priority | High |\n| Owner | Ann |\n"
            ),
            "The engine keeps a cache of parsed documents.\n\n## Details\n\nMore words.",
        ),
        item(
            C,
            "REQ-GAMMA",
            "Gamma",
            "",
            "Mentions [alpha](#A) and a cache too."
                .replace("#A", &format!("#{A}"))
                .as_str()
        ),
    );
    let b = format!(
        "{}\n---\n\nSee [the alpha item](a.mara.md#{A}) here.\n",
        item(B, "REQ-BETA", "Beta", "", "Body of beta with zebras.")
    );
    snapshot(&[("docs/a.mara.md", a), ("docs/b.mara.md", b)])
}

fn documents(snapshot: &Snapshot) -> Documents<'_> {
    Documents {
        snapshot,
        diagnostics: &[],
    }
}

fn first_page() -> Paging<'static> {
    Paging::default()
}

/// The `<path>:<line>` and kind of each search result.
fn found(page: &Page<SearchHit>) -> Vec<String> {
    page.records
        .iter()
        .map(|hit| {
            let (path, line) = hit.node.location();
            match &hit.node {
                NodeSummary::Item(item) => item.id.clone().unwrap_or_default(),
                _ => format!("{path}:{line}"),
            }
        })
        .collect()
}

#[test]
fn counts_edits_as_optimal_string_alignment() {
    let chars = |text: &str| -> Vec<char> { text.chars().collect() };
    let within = |a: &str, b: &str, max: usize| within(&chars(a), &chars(b), max);
    assert!(within("cache", "cache", 0));
    assert!(within("cache", "cachd", 1));
    assert!(within("cache", "cahce", 1), "a swap is one edit");
    assert!(within("cache", "cach", 1));
    assert!(within("cache", "caches", 1));
    assert!(!within("cache", "cxxhe", 1));
    assert!(within("cache", "cxxhe", 2));
    assert!(
        !within("ca", "abc", 2),
        "no edit of a swapped pair: optimal string alignment"
    );
    assert!(within("", "ab", 2) && !within("", "abc", 2));
    assert!(within("ab", "", 2) && !within("abc", "", 2));
    let long: String = "x".repeat(5000);
    assert!(within(&long, &format!("{long}y"), 1));
    assert!(within("äöü", "äöu", 1), "characters, not bytes");
    assert_eq!(
        (1..=20).map(edits_allowed).collect::<Vec<_>>(),
        [0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 0, 0]
    );
}

/// SplitMix64, for seeded test vocabularies.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[test]
fn search_matches_exactly_the_words_that_the_rule_allows() {
    // Words over a small alphabet, so that many are within an edit or two of each other, of 1 to
    // 24 characters, with words one and two edits from long ones around the 18-character limit.
    let mut state = 11;
    let mut random = |length: usize| -> String {
        (0..length)
            .map(|_| ['a', 'b', 'c'][(splitmix(&mut state) % 3) as usize])
            .collect()
    };
    let mut vocabulary = std::collections::BTreeSet::new();
    for length in 1..=24 {
        for _ in 0..12 {
            vocabulary.insert(random(length));
        }
    }
    for length in 15..=21 {
        let base: Vec<char> = random(length).chars().collect();
        let at = length / 2;
        let mut variants = vec![base.clone()];
        let mut substituted = base.clone();
        substituted[at] = 'd';
        variants.push(substituted.clone());
        substituted[at + 2] = 'd';
        variants.push(substituted);
        let mut deleted = base.clone();
        deleted.remove(at);
        variants.push(deleted.clone());
        deleted.remove(at);
        variants.push(deleted);
        let mut inserted = base.clone();
        inserted.insert(at, 'd');
        variants.push(inserted.clone());
        inserted.insert(0, 'd');
        variants.push(inserted);
        let mut swapped = base.clone();
        swapped.swap(at, at + 1);
        variants.push(swapped.clone());
        swapped.swap(1, 2);
        variants.push(swapped);
        vocabulary.extend(
            variants
                .into_iter()
                .map(|chars| chars.into_iter().collect::<String>()),
        );
    }
    let words: Vec<String> = vocabulary.into_iter().collect();
    // Each word in a narrative unit of its own, at line 1 + 4 × its index.
    let text = words.join("\n\n---\n\n");
    let snapshot = snapshot(&[("docs/w.mara.md", text)]);
    let documents = documents(&snapshot);
    let chars = |word: &str| -> Vec<char> { word.chars().collect() };
    let mut comparisons = 0;
    for query in words.iter().filter(|word| word.len() >= 3) {
        let query_chars = chars(query);
        // The rule of REQ-TYPO-TOLERANT-SEARCH, written here apart from the code.
        let edits = match query_chars.len() {
            4..=7 => 1,
            8..=18 => 2,
            _ => 0,
        };
        let expected: Vec<&str> = words
            .iter()
            .map(String::as_str)
            .filter(|word| {
                *word == query || (edits > 0 && within(&query_chars, &chars(word), edits))
            })
            .collect();
        comparisons += words.len();
        let mut found: Vec<&str> = Vec::new();
        let mut cursor = None;
        loop {
            let paging = Paging {
                limit: 100,
                cursor: cursor.as_ref(),
            };
            let page = search(&documents, query, &Filters::default(), &paging).unwrap();
            found.extend(
                page.records
                    .iter()
                    .map(|hit| words[(hit.node.location().1 - 1) / 4].as_str()),
            );
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        found.sort_unstable();
        assert_eq!(
            found,
            expected,
            "{query} ({} characters, {edits} edits)",
            query_chars.len()
        );
    }
    assert!(comparisons > 50_000, "{comparisons} comparisons");
}

#[test]
fn search_finds_items_and_narrative_holding_every_word() {
    let snapshot = example();
    let documents = documents(&snapshot);
    let search =
        |query: &str| search(&documents, query, &Filters::default(), &first_page()).unwrap();
    assert_eq!(found(&search("cache engine")), ["REQ-ALPHA"]);
    assert_eq!(found(&search("zebras")), ["docs/a.mara.md:1", "REQ-BETA"]);
    assert_eq!(found(&search("glossary")), ["docs/a.mara.md:1"]);
    // Exact matches first, then those that need an edit; an ID or title before a body.
    assert_eq!(found(&search("cache")), ["REQ-ALPHA", "REQ-GAMMA"]);
    assert_eq!(
        found(&search("alpha")),
        ["REQ-ALPHA", "REQ-GAMMA", "docs/b.mara.md:14"]
    );
    assert_eq!(found(&search("details")), ["REQ-ALPHA"]);
    assert_eq!(found(&search("zebraz")), ["docs/a.mara.md:1", "REQ-BETA"]);
}

#[test]
fn ranks_exact_before_typo_and_high_zones_before_low() {
    let one = item(A, "REQ-ONE", "Plain", "", "Mentions widget in the body.");
    let two = item(B, "REQ-TWO", "Widget title", "", "Nothing.");
    let three = item(C, "REQ-THREE", "Widgte typo", "", "Nothing.");
    let text = format!("{one}\n---\n\n{two}\n---\n\n{three}");
    let snapshot = snapshot(&[("docs/a.mara.md", text)]);
    let documents = documents(&snapshot);
    let page = search(&documents, "widget", &Filters::default(), &first_page()).unwrap();
    assert_eq!(found(&page), ["REQ-TWO", "REQ-ONE", "REQ-THREE"]);
    let hit = &page.records[1];
    assert_eq!(hit.excerpt.text, "Mentions widget in the body.");
    assert_eq!(hit.excerpt.line, 10);
}

#[test]
fn ids_and_mids_match_only_exactly() {
    let snapshot = example();
    let documents = documents(&snapshot);
    let search =
        |query: &str| search(&documents, query, &Filters::default(), &first_page()).unwrap();
    assert_eq!(found(&search(A)), ["REQ-ALPHA"]);
    let near = format!("{}Z", &A[..25]);
    assert_eq!(found(&search(&near)), [] as [&str; 0]);
    assert_eq!(found(&search("gamma")), ["REQ-GAMMA"]);
    assert_eq!(
        found(&search("gamme")),
        ["REQ-GAMMA"],
        "a typo matches the title"
    );
}

#[test]
fn filters_match_exactly_and_exclude_narrative() {
    let snapshot = example();
    let documents = documents(&snapshot);
    let list = |filters: Filters| -> Vec<String> {
        item_list(&documents, &filters, &first_page())
            .unwrap()
            .records
            .into_iter()
            .map(|item| item.id.unwrap())
            .collect()
    };
    assert_eq!(
        list(Filters::default()),
        ["REQ-ALPHA", "REQ-GAMMA", "REQ-BETA"]
    );
    let types = |t: &str| Filters {
        types: vec![t.into()],
        ..Filters::default()
    };
    assert_eq!(list(types("Requirement")).len(), 3);
    assert_eq!(list(types("requirements")), [] as [&str; 0]);
    let field = |k: &str, v: &str| Filters {
        fields: vec![(k.into(), v.into())],
        ..Filters::default()
    };
    assert_eq!(
        list(field("priority", "high")),
        ["REQ-ALPHA"],
        "an enum value is a label"
    );
    assert_eq!(list(field("Owner", "Ann")), ["REQ-ALPHA"]);
    assert_eq!(
        list(field("owner", "ann")),
        [] as [&str; 0],
        "text matches as written"
    );
    assert_eq!(list(field("status", "draft")).len(), 3);
    let relation = |r: &str| Filters {
        relations: vec![r.into()],
        ..Filters::default()
    };
    assert_eq!(list(relation("satisfies")), ["REQ-ALPHA"]);
    assert_eq!(list(relation("Satisfied by")), ["REQ-BETA"]);
    assert_eq!(list(relation("satisfie")), [] as [&str; 0]);
    let path = |p: &str| Filters {
        paths: vec![p.into()],
        ..Filters::default()
    };
    assert_eq!(list(path("docs/b.mara.md")), ["REQ-BETA"]);
    assert_eq!(list(path("docs/")).len(), 3);
    assert_eq!(list(path("doc")), [] as [&str; 0]);
    let ids = |i: &str| Filters {
        ids: vec![i.into()],
        ..Filters::default()
    };
    assert_eq!(list(ids("REQ-BETA")), ["REQ-BETA"]);
    assert_eq!(list(ids(C)), ["REQ-GAMMA"]);
    assert_eq!(list(ids("REQ-BET")), [] as [&str; 0]);

    let search =
        |filters: Filters| found(&search(&documents, "zebras", &filters, &first_page()).unwrap());
    assert_eq!(search(path("docs/a.mara.md")), ["docs/a.mara.md:1"]);
    assert_eq!(search(types("requirement")), ["REQ-BETA"]);
}

#[test]
fn get_reads_items_units_and_documents() {
    let snapshot = example();
    let documents = documents(&snapshot);
    let by_id = get(&documents, "REQ-ALPHA", None).unwrap();
    let by_mid = get(&documents, A, None).unwrap();
    assert_eq!(by_id.rows, by_mid.rows);
    assert_eq!(by_id.content, by_mid.content);
    assert_eq!(by_id.revision, by_mid.revision);
    let keys: Vec<&str> = by_id.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(
        keys,
        ["Type", "ID", "Status", "Satisfies", "Priority", "Owner"]
    );
    assert!(by_id.content.starts_with("The engine keeps"));
    assert!(by_id.content.ends_with("More words."));
    assert_eq!(by_id.next_cursor, None);
    let at_body = get(&documents, "docs/a.mara.md:20", None).unwrap();
    assert_eq!(at_body.node, by_id.node);
    let unit = get(&documents, "docs/a.mara.md:3", None).unwrap();
    assert_eq!(unit.content, "# Overview\n\nGlossary of zebras.");
    let whole = get(&documents, "docs/b.mara.md", None).unwrap();
    assert!(whole.content.ends_with("here.\n"));
    for missing in [
        "docs/a.mara.md:5",
        "docs/a.mara.md:0",
        "docs/a.mara.md:999",
        "docs/c.mara.md",
    ] {
        let error = get(&documents, missing, None).unwrap_err();
        assert_eq!(error.code(), "unknown-node", "{missing}");
    }
    let error = get(&documents, "REQ-NONE", None).unwrap_err();
    assert_eq!(error.code(), "unknown-item");
}

#[test]
fn get_cuts_a_large_node_into_portions_that_concatenate_to_it() {
    let long_value = "v".repeat(PAGE_BYTES + 10);
    let long_line = "\"quoted\" ".repeat(PAGE_BYTES / 4);
    let body = format!("{long_line}\nshort line\n{}", "line of text\n".repeat(9000));
    let text = item(
        A,
        "REQ-BIG",
        "Big",
        &format!("| Owner | {long_value} |\n"),
        &body,
    );
    let snapshot = snapshot(&[("docs/a.mara.md", text)]);
    let documents = documents(&snapshot);
    let whole = snapshot.item(0);
    let (mut rows, mut content, mut cursor) = (Vec::<RowPart>::new(), String::new(), None);
    let mut pages = 0;
    loop {
        let portion = get(&documents, "REQ-BIG", cursor.as_ref()).unwrap();
        assert!(portion.json.len() <= PAGE_BYTES);
        if !content.is_empty() || !portion.content.is_empty() {
            assert!(
                portion.rows.is_empty() || content.is_empty(),
                "rows come first"
            );
        }
        for part in portion.rows {
            match rows.last_mut() {
                Some(last) if last.partial => {
                    last.value.push_str(&part.value);
                    last.partial = part.partial;
                }
                _ => rows.push(part),
            }
        }
        content.push_str(&portion.content);
        pages += 1;
        match portion.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert!(pages > 3);
    let values: Vec<&str> = rows.iter().map(|row| row.value.as_str()).collect();
    let expected: Vec<&str> = whole.rows.iter().map(|row| row.value.as_str()).collect();
    assert_eq!(values, expected);
    let contents = std::str::from_utf8(&snapshot.documents()[0].file.contents).unwrap();
    assert_eq!(content, &contents[whole.body.clone()]);
}

#[test]
fn a_cursor_of_another_request_or_snapshot_is_stale() {
    let snapshot = example();
    let documents = documents(&snapshot);
    let one = Paging {
        limit: 1,
        cursor: None,
    };
    let page = item_list(&documents, &Filters::default(), &one).unwrap();
    let cursor = page.next_cursor.unwrap();
    let next = Paging {
        cursor: Some(&cursor),
        ..one
    };
    assert_eq!(
        item_list(&documents, &Filters::default(), &next)
            .unwrap()
            .records
            .len(),
        1
    );
    let other = Filters {
        types: vec!["requirement".into()],
        ..Filters::default()
    };
    assert_eq!(
        item_list(&documents, &other, &next).unwrap_err(),
        ReadError::StaleCursor
    );
    let edited = snapshot_with(&snapshot, "# Changed\n".to_owned());
    let changed = Documents {
        snapshot: &edited,
        diagnostics: &[],
    };
    assert_eq!(
        item_list(&changed, &Filters::default(), &next).unwrap_err(),
        ReadError::StaleCursor
    );
    assert_eq!(Paging::default().limit, DEFAULT_LIMIT);
}

/// The example with a third document.
fn snapshot_with(_: &Snapshot, text: String) -> Snapshot {
    let example = example();
    let mut documents: Vec<(&str, String)> = example
        .documents()
        .iter()
        .map(|document| {
            let text = String::from_utf8(document.file.contents.to_vec()).unwrap();
            (document.path.as_str(), text)
        })
        .collect();
    documents.push(("docs/c.mara.md", text));
    snapshot(&documents)
}

#[test]
fn relation_list_reads_one_hop() {
    let snapshot = example();
    let documents = documents(&snapshot);
    let describe = |page: &RelationPage| -> Vec<String> {
        page.page
            .records
            .iter()
            .map(|c| {
                let neighbour = match &c.neighbour {
                    Neighbour::Node(NodeSummary::Item(item)) => item.id.clone().unwrap(),
                    Neighbour::Node(node) => format!("{}:{}", node.location().0, node.location().1),
                    Neighbour::Unresolved { mid } => mid.clone(),
                };
                format!(
                    "{} {} {} {} @{}",
                    c.kind.name(),
                    c.relation.as_deref().unwrap_or("-"),
                    c.direction.name(),
                    neighbour,
                    c.source.line
                )
            })
            .collect()
    };
    let alpha = relation_list(&documents, "REQ-ALPHA", &[], &first_page()).unwrap();
    assert_eq!(
        describe(&alpha),
        [
            "relation satisfies outgoing REQ-BETA @15",
            "mention - incoming REQ-GAMMA @36",
            "mention - incoming docs/b.mara.md:14 @14"
        ]
    );
    let beta = relation_list(&documents, B, &[], &first_page()).unwrap();
    assert_eq!(
        describe(&beta),
        ["relation satisfied_by incoming REQ-ALPHA @15"]
    );
    let narrowed = relation_list(
        &documents,
        "REQ-ALPHA",
        &["Satisfies".into()],
        &first_page(),
    )
    .unwrap();
    assert_eq!(
        describe(&narrowed),
        ["relation satisfies outgoing REQ-BETA @15"]
    );
    let unit = relation_list(&documents, "docs/b.mara.md:14", &[], &first_page()).unwrap();
    assert_eq!(describe(&unit), ["mention - outgoing REQ-ALPHA @14"]);
    // A whole document: every relation and mention written in it, in source order.
    let whole = relation_list(&documents, "docs/a.mara.md", &[], &first_page()).unwrap();
    assert_eq!(
        describe(&whole),
        [
            "relation satisfies outgoing REQ-BETA @15",
            "mention - outgoing REQ-ALPHA @36"
        ]
    );
    let b = relation_list(&documents, "docs/b.mara.md", &[], &first_page()).unwrap();
    assert_eq!(describe(&b), ["mention - outgoing REQ-ALPHA @14"]);
}

#[test]
fn schema_inspection_returns_every_declaration_as_loaded() {
    let schema = schema();
    let layer = SchemaLayer {
        schema: &schema,
        key: schema_layer(None, None),
        path: ".mara/schema.toml",
    };
    let list = schema_list(&layer, &first_page()).unwrap();
    let names: Vec<&str> = list.records.iter().map(Declaration::name).collect();
    assert_eq!(names, ["requirement", "satisfies"]);
    let requirement = schema_get(&layer, "Requirement", &first_page()).unwrap();
    assert!(requirement.json.contains(
        "\"fields\":[{\"name\":\"owner\",\"kind\":\"text\",\"values\":null,\"required\":true,\
         \"repeatable\":true},{\"name\":\"priority\",\"kind\":\"enum\",\"values\":[\"high\",\"low\"],\
         \"required\":false,\"repeatable\":false}]"
    ), "{}", requirement.json);
    let relation = schema_get(&layer, "satisfied by", &first_page()).unwrap();
    assert!(
        relation.json.contains(
            "\"targets\":{\"code\":[\"requirement\"],\"requirement\":[\"requirement\"]},\
         \"same_type\":false,\"acyclic\":true"
        ),
        "{}",
        relation.json
    );
    assert_eq!(
        schema_get(&layer, "nothing", &first_page())
            .unwrap_err()
            .code(),
        "unknown-declaration"
    );
}

#[test]
fn a_record_larger_than_a_page_fails_with_a_diagnostic_naming_it() {
    let title = "t".repeat(PAGE_BYTES);
    let text = item(A, "REQ-HUGE", &title, "", "Body.");
    let snapshot = snapshot(&[("docs/a.mara.md", text)]);
    let documents = documents(&snapshot);
    let error = item_list(&documents, &Filters::default(), &first_page()).unwrap_err();
    let ReadError::TooLarge(diagnostic) = error else {
        panic!("{error:?}");
    };
    assert_eq!(
        (diagnostic.path.as_str(), diagnostic.line),
        ("docs/a.mara.md", 1)
    );
}

#[test]
fn words_that_share_a_hash_slow_only_their_lookups_and_still_match_by_the_rule() {
    // Nine-letter words built to share one hash of the vocabulary index (short vectors of the
    // lattice of its polynomial hash modulo 2^64), and words one and two edits from the first.
    let colliding = [
        "怀急恊息悔悹惞愃愨",
        "怅怭怓怳悐恼悏愒惯",
        "怈怜恠恃惁悀愓悳愦",
        "怍怤怩怇悽恃惄惂惭",
        "怞忝恷怸悵悘愆愭愲",
        "怣忥恀忼悱恛悷愼惹",
        "怦忔悍怌惢恟愻惝愰",
        "怫応恖忐惞怢惬惬惷",
    ];
    let mut words: Vec<String> = colliding.iter().map(|w| (*w).to_owned()).collect();
    let first: Vec<char> = colliding[0].chars().collect();
    let mut one = first.clone();
    one[4] = '一';
    let mut two = one.clone();
    two.swap(0, 1);
    let mut shorter = first.clone();
    shorter.remove(8);
    words.extend([one, two, shorter].map(|chars| chars.into_iter().collect::<String>()));
    let text = words.join("\n\n---\n\n");
    let snapshot = snapshot(&[("docs/w.mara.md", text)]);
    let chars = |word: &str| -> Vec<char> { word.chars().collect() };
    // They collide: every one is a candidate of every other's lookup.
    let index = snapshot.spellings(2);
    for word in colliding {
        let candidates = index.candidates(&chars(word));
        assert!(
            colliding.iter().all(|w| candidates.contains(w)),
            "{word}: {candidates:?}"
        );
    }
    // Search still returns exactly the words within two edits.
    let documents = documents(&snapshot);
    for query in &words {
        let expected: Vec<&str> = words
            .iter()
            .map(String::as_str)
            .filter(|word| within(&chars(query), &chars(word), 2))
            .collect();
        let page = search(&documents, query, &Filters::default(), &first_page()).unwrap();
        let mut found: Vec<&str> = page
            .records
            .iter()
            .map(|hit| words[(hit.node.location().1 - 1) / 4].as_str())
            .collect();
        found.sort_unstable();
        let mut expected = expected;
        expected.sort_unstable();
        assert_eq!(found, expected, "{query}");
    }
}

/// What `pulldown-cmark` renders of `range` of `text`, with a space where the parser ends a
/// word: a break, inline HTML and the edges of a block.
fn rendered(text: &str, range: std::ops::Range<usize>) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let inline = |tag: &Tag<'_>| {
        matches!(
            tag,
            Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }
        )
    };
    let inline_end = |tag: &TagEnd| {
        matches!(
            tag,
            TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image
        )
    };
    let mut out = String::new();
    for (event, span) in Parser::new_ext(text, options).into_offset_iter() {
        if span.start < range.start || span.end > range.end {
            continue;
        }
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::Start(tag) if !inline(&tag) => out.push(' '),
            Event::End(tag) if !inline_end(&tag) => out.push(' '),
            Event::SoftBreak | Event::HardBreak | Event::InlineHtml(_) | Event::Html(_) => {
                out.push(' ')
            }
            _ => {}
        }
    }
    out
}

#[test]
fn indexed_text_is_what_the_markdown_parser_renders_with_offsets_in_the_source() {
    use crate::document::{Node, Zone, words};
    use std::collections::{BTreeMap, BTreeSet};
    // Each construct, and whether it runs over lines.
    let constructs: &[(&str, bool)] = &[
        ("plain words here", false),
        (
            "*em* and **strong** and _under_ and __double__ and ~~struck~~",
            false,
        ),
        ("an [inline link](https://x.test/destination) here", false),
        ("a [reference link][r] and [r][] and [shortcut] here", false),
        (
            "an <https://auto.test/path> and <mail@auto.test> here",
            false,
        ),
        ("an ![image alt](https://x.test/i.png) here", false),
        ("`one` and ``two `tick` two`` and ```three``three```", false),
        ("` spaced ` and `   ` and ` a\\|needle `", false),
        ("x` a\nb `y and `aaaa\nbbbb` tail", true),
        ("three `a1\nb2\nc3` lines", true),
        ("`\nnewline first `", true),
        ("`a\n spaced second`", true),
        ("caf&eacute; &#233;t&#xe9; &amp; done", false),
        ("\\*not em\\* and \\[bracket\\] and a\\|pipe", false),
        ("soft\nbreak and *emphasised\nwrapped* words", true),
        (
            "hard  \nbreak and *hard  \nwrapped* and back\\\nslash",
            true,
        ),
        ("[link\ntext over lines](https://x.test/d) tail", true),
        ("<span title=\"t\">inline html</span> tail", false),
        ("<span\ntitle=\"x\">wrapped html</span> tail", true),
    ];
    let zones = [
        "title",
        "heading",
        "metadata",
        "body",
        "list",
        "quote",
        "narrative",
        "lead",
    ];
    for nl in ["\n", "\r\n", "\r"] {
        let mut text = String::new();
        // Case number, zone, construct, the snippet's range, and the words of its zone that
        // the template adds.
        let mut cases = Vec::new();
        let mut n = 0u32;
        for zone in zones {
            for &(construct, lines) in constructs {
                if lines && matches!(zone, "title" | "heading" | "metadata") {
                    continue;
                }
                n += 1;
                let indent = match zone {
                    "list" => format!("{nl}  "),
                    "quote" => format!("{nl}> "),
                    _ => nl.to_owned(),
                };
                let snippet = construct.replace('\n', &indent);
                let id = format!("REQ-C{n}");
                let head = |title: &str| {
                    format!(
                        "<a id=\"{}\"></a>{nl}{nl}### {title}{nl}{nl}| Type | Requirement |{nl}\
                         | --- | --- |{nl}| ID | {id} |{nl}| Status | Draft |{nl}",
                        mid(n)
                    )
                };
                let (before, after, fixed): (String, String, &[&str]) = match zone {
                    "title" => (head(""), String::new(), &[]),
                    "heading" => (format!("{}{nl}#### ", head("Plain")), String::new(), &[]),
                    "metadata" => (
                        format!("{}| Owner | ", head("Plain")),
                        format!(" |{nl}{nl}Body text."),
                        &["requirement", "draft"],
                    ),
                    "body" => (format!("{}{nl}", head("Plain")), String::new(), &[]),
                    "list" => (format!("{}{nl}- ", head("Plain")), String::new(), &[]),
                    "quote" => (format!("{}{nl}> ", head("Plain")), String::new(), &[]),
                    "narrative" => (String::new(), String::new(), &[]),
                    _ => (
                        format!("before <a id=\"{}\"></a> ", mid(n)),
                        String::new(),
                        &["body", "text"],
                    ),
                };
                if n > 1 {
                    text.push_str(&format!("{nl}{nl}---{nl}{nl}"));
                }
                if zone == "title" {
                    // The heading is the snippet.
                    let start_of = text.len() + before.find("### ").unwrap() + 4;
                    text.push_str(&before.replacen("### ", &format!("### {snippet}"), 1));
                    text.push_str(&format!("{nl}Body text."));
                    cases.push((
                        n,
                        zone,
                        construct,
                        start_of..start_of + snippet.len(),
                        vec![],
                    ));
                    continue;
                }
                text.push_str(&before);
                // After a merged anchor the parser's text starts with the space before the
                // snippet.
                let start = text.len() - usize::from(zone == "lead");
                text.push_str(&snippet);
                let end = text.len();
                text.push_str(&after);
                if zone == "lead" {
                    text.push_str(&format!("{nl}{nl}### Plain{nl}{nl}| Type | Requirement |{nl}| --- | --- |{nl}| ID | {id} |{nl}| Status | Draft |{nl}{nl}Body text."));
                }
                cases.push((n, zone, construct, start..end, fixed.to_vec()));
            }
        }
        text.push_str(&format!(
            "{nl}{nl}---{nl}{nl}[r]: https://x.test/r{nl}[shortcut]: https://x.test/s{nl}"
        ));
        // The parser reads a lone CR as LF; `pulldown-cmark` sees that text.
        let parsed = if nl == "\r" {
            text.replace('\r', "\n")
        } else {
            text.clone()
        };
        let snapshot = snapshot(&[("docs/c.mara.md", text.clone())]);
        let documents = documents(&snapshot);
        let line_of = |at: usize| 1 + crate::read::tests::breaks(&text[..at]);
        // The start of each code span over several lines, with the words the parser rendered of
        // it: a word in one has that position (DES-SEARCH-REQUESTS).
        let code_starts = multi_line_code_spans(&parsed);
        for (n, zone, construct, range, fixed) in cases {
            let case = format!("{zone}, {construct:?}, {nl:?}");
            let node = match zone {
                "narrative" => {
                    let units = &snapshot.documents()[0].file.document.units;
                    let local = units
                        .iter()
                        .position(|u| u.span.contains(&range.start))
                        .unwrap_or_else(|| panic!("{case}: no unit"));
                    Node::Unit(snapshot.documents()[0].units.start + local)
                }
                _ => Node::Item(snapshot.items_with_id(&format!("REQ-C{n}"))[0]),
            };
            let zone_of = match zone {
                "title" => Zone::Title,
                "heading" => Zone::Heading,
                "metadata" => Zone::Metadata,
                _ => Zone::Body,
            };
            // The words indexed in the zone are those of what the parser rendered.
            let mut expected: BTreeSet<String> = words(&rendered(&parsed, range.clone())).collect();
            expected.extend(fixed.iter().map(|w| (*w).to_owned()));
            let mut postings: BTreeMap<String, Vec<(Zone, usize)>> = BTreeMap::new();
            for (word, list) in snapshot_words(&snapshot) {
                for posting in list.iter().filter(|p| p.node == node) {
                    postings
                        .entry(word.clone())
                        .or_default()
                        .push((posting.zone, posting.at as usize));
                }
            }
            let indexed: BTreeSet<String> = postings
                .iter()
                .filter(|(_, list)| list.iter().any(|(z, _)| *z == zone_of))
                .map(|(word, _)| word.clone())
                .collect();
            assert_eq!(indexed, expected, "{case}");
            // Every posting points at the first character of its word in the original bytes,
            // or at the start of the entity that renders it.
            for (word, list) in &postings {
                for &(_, at) in list {
                    let first = text[at..].chars().next().unwrap();
                    let lowered: String = first.to_lowercase().collect();
                    let in_code = code_starts
                        .get(&at)
                        .is_some_and(|words| words.contains(word));
                    assert!(
                        word.starts_with(&lowered) || first == '&' || in_code,
                        "{case}: {word} at {at}: {:?}",
                        &text[at..(at + 12).min(text.len())]
                    );
                }
                // The excerpt for the word is the line of its first occurrence in the node.
                let first_at = list.iter().map(|&(_, at)| at).min().unwrap();
                let (path, line) = node_location(&snapshot, node);
                let mut cursor = None;
                let hit = loop {
                    let paging = Paging {
                        limit: 100,
                        cursor: cursor.as_ref(),
                    };
                    let page = search(&documents, word, &Filters::default(), &paging).unwrap();
                    let at = (path.as_str(), line);
                    let found = page
                        .records
                        .into_iter()
                        .find(|hit| hit.node.location() == at);
                    match (found, page.next_cursor) {
                        (Some(hit), _) => break Some(hit),
                        (None, Some(next)) => cursor = Some(next),
                        (None, None) => break None,
                    }
                };
                let hit = hit.unwrap_or_else(|| panic!("{case}: {word} finds the node"));
                assert_eq!(
                    hit.excerpt.line,
                    line_of(first_at),
                    "{case}: {word}: {:?}",
                    hit.excerpt.text
                );
            }
            // A metadata value's text is what the parser rendered of its cell.
            if zone == "metadata" {
                let Node::Item(item) = node else {
                    unreachable!()
                };
                let row = snapshot
                    .item(item)
                    .rows
                    .iter()
                    .find(|row| row.name == "owner")
                    .unwrap();
                let cell = rendered(&parsed, range.clone());
                assert_eq!(
                    row.text,
                    cell.split_whitespace().collect::<Vec<_>>().join(" "),
                    "{case}"
                );
            }
        }
    }
}

/// Every search word of `snapshot` with its postings.
fn snapshot_words(snapshot: &Snapshot) -> Vec<(String, Vec<crate::document::Posting>)> {
    let mut all = Vec::new();
    for document in snapshot.documents() {
        for word in &document.file.document.words {
            all.push((word.text.clone(), snapshot.postings(&word.text).to_vec()));
        }
    }
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all.dedup_by(|a, b| a.0 == b.0);
    all
}

/// The path and line of `node`.
fn node_location(snapshot: &Snapshot, node: crate::document::Node) -> (String, usize) {
    use crate::document::Node;
    match node {
        Node::Item(item) => {
            let entry = &snapshot.items()[item];
            (
                snapshot.documents()[entry.document].path.clone(),
                snapshot.item(item).line,
            )
        }
        Node::Unit(unit) => {
            let entry = snapshot.units()[unit];
            (
                snapshot.documents()[entry.document].path.clone(),
                snapshot.unit(unit).line,
            )
        }
    }
}

/// The line breaks in `text`: LF, CRLF or a lone CR.
pub(crate) fn breaks(text: &str) -> usize {
    let bytes = text.as_bytes();
    (0..bytes.len())
        .filter(|&at| {
            bytes[at] == b'\n' || (bytes[at] == b'\r' && bytes.get(at + 1) != Some(&b'\n'))
        })
        .count()
}

/// The `n`th test MID: a canonical uppercase ULID.
fn mid(n: u32) -> String {
    const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut text = String::from("01M7D8DZ2663WP3A03");
    for shift in (0..8).rev() {
        text.push(CROCKFORD[((n >> (shift * 4)) & 0xF) as usize] as char);
    }
    text
}

#[test]
fn no_read_panics_on_random_markdown() {
    const TOKENS: &[&str] = &[
        "word",
        "Wörter",
        "漢字",
        "a1",
        "needle",
        " ",
        " ",
        "\n",
        "\n",
        "\r\n",
        "\r",
        "\n\n",
        "*",
        "**",
        "_",
        "`",
        "``",
        "[",
        "]",
        "(",
        ")",
        "<",
        ">",
        "|",
        "\\",
        "&amp;",
        "&",
        ";",
        "&#x1F600;",
        "&eacute;",
        "# ",
        "### ",
        "- ",
        "> ",
        "1. ",
        "---",
        "***",
        "```",
        "~~~",
        "<!--",
        "-->",
        "\u{a0}",
        "\u{feff}",
        "\t",
        "    ",
        "\\|",
        "<span>",
        "</span>",
        "<a id=\"01M7D8DZ2663WP3A037SR2HJH2\"></a>",
        "<a id=\"01M7D8DZ2RBHGQBYAV7V3789H7\"></a>",
        "| Type | Requirement |",
        "| --- | --- |",
        "| ID | REQ-X |",
        "| Owner | a `b\\|c` d |",
        "[x](#01M7D8DZ2663WP3A037SR2HJH2)",
        "<https://x.test/p>",
        "![i](u.png)",
    ];
    // Documents of Unicode spaces only, then random ones.
    let spaces = [
        "\u{a0}",
        " \u{a0} ",
        "\u{3000}",
        "\u{2028}",
        "\u{feff}\u{a0}",
        "\t\u{a0}\n",
        "\u{85}",
    ];
    let mut state = 0x5EED;
    for round in 0..250 + spaces.len() {
        let text: String = match spaces.get(round) {
            Some(text) => (*text).to_owned(),
            None => {
                let length = (splitmix(&mut state) % 80) as usize;
                (0..length)
                    .map(|_| TOKENS[(splitmix(&mut state) % TOKENS.len() as u64) as usize])
                    .collect()
            }
        };
        let snapshot = snapshot(&[("docs/f.mara.md", text.clone())]);
        let documents = documents(&snapshot);
        let walk = |query: &str| {
            let mut cursor = None;
            loop {
                let paging = Paging {
                    limit: 7,
                    cursor: cursor.as_ref(),
                };
                let page = search(&documents, query, &Filters::default(), &paging);
                match page.ok().and_then(|page| page.next_cursor) {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
        };
        walk("");
        for (word, _) in snapshot_words(&snapshot).into_iter().take(25) {
            walk(&word);
        }
        let mut nodes: Vec<String> = vec!["docs/f.mara.md".to_owned()];
        nodes.extend((0..snapshot.items().len()).map(|item| snapshot.item(item).mid.clone()));
        nodes.extend(
            (0..snapshot.units().len())
                .map(|unit| format!("docs/f.mara.md:{}", snapshot.unit(unit).line)),
        );
        for node in &nodes {
            let _ = get(&documents, node, None);
            let _ = relation_list(&documents, node, &[], &first_page());
        }
        let _ = item_list(&documents, &Filters::default(), &first_page());
    }
}

/// The start of each code span of `text` that runs over several lines, with the words of what
/// `pulldown-cmark` rendered of it, the first of each word starting in it included.
fn multi_line_code_spans(text: &str) -> std::collections::BTreeMap<usize, Vec<String>> {
    use pulldown_cmark::{Event, Options, Parser};
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let mut starts = std::collections::BTreeMap::new();
    let mut events = Parser::new_ext(text, options).into_offset_iter().peekable();
    let mut before = String::new();
    while let Some((event, span)) = events.next() {
        match event {
            Event::Code(code) if text[span.clone()].contains(['\n', '\r']) => {
                // A word that starts in the span may run on into the text after it.
                let after = match events.peek() {
                    Some((Event::Text(after), _)) => after.to_string(),
                    _ => String::new(),
                };
                let joined = format!("{code}{after}");
                let mut words: Vec<String> = crate::document::words(&joined).collect();
                // A word that runs from the text before into the span starts before it.
                if before.ends_with(|c: char| c.is_alphanumeric())
                    && code.starts_with(|c: char| c.is_alphanumeric())
                {
                    words.remove(0);
                }
                starts.insert(span.start, words);
                before = code.to_string();
            }
            Event::Text(t) | Event::Code(t) => before = t.to_string(),
            _ => before.clear(),
        }
    }
    starts
}

#[test]
fn a_word_in_a_code_span_over_lines_has_the_span_s_start() {
    // The reviewer's inputs: a code span over two lines in a block quote and a list item. By
    // the maintainer's decision the word has the position of the code span's start, the
    // backtick at byte 2, and its excerpt is line 1.
    for input in ["> `\n> needle `", "- `\n  needle `", "> `a\n>  needle`"] {
        for nl in ["\n", "\r\n", "\r"] {
            let text = input.replace('\n', nl);
            let snapshot = snapshot(&[("docs/q.mara.md", text.clone())]);
            let postings = snapshot.postings("needle");
            assert_eq!(postings.len(), 1, "{text:?}");
            assert_eq!(postings[0].at, 2, "{text:?}");
            let documents = documents(&snapshot);
            let page = search(&documents, "needle", &Filters::default(), &first_page()).unwrap();
            assert_eq!(page.records[0].excerpt.line, 1, "{text:?}");
        }
    }
    // On one line, a word in a code span keeps its own position.
    let snapshot = snapshot(&[("docs/q.mara.md", "> `a  needle`".to_owned())]);
    assert_eq!(snapshot.postings("needle")[0].at, 6);
}
