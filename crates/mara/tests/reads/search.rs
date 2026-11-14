//! VER-SEARCH-CASES: `search` on a project whose items and narrative hold planted words.

use serde_json::Value;

use crate::{Project, document, item, line_of, link, mid, names};

fn search(project: &Project, args: &[&str]) -> Value {
    let mut all = vec!["search"];
    all.extend_from_slice(args);
    all.extend_from_slice(&["--format", "json"]);
    project.run(&all).ok().json()
}

fn found(project: &Project, args: &[&str]) -> Vec<String> {
    names(&search(project, args)["results"])
}

#[test]
fn every_query_word_in_the_node_s_own_text() {
    let project = Project::new();
    let target = item(
        &mid(3),
        "Requirement",
        "REQ-TARGET",
        "Target",
        &[],
        "Holds axolotl only here.",
    );
    let linking = item(
        &mid(4),
        "Requirement",
        "REQ-LINKING",
        "Linking",
        &[("Refines", &link("REQ-TARGET", "", &mid(3)))],
        &format!("Points at {}.", link("the target", "", &mid(3))),
    );
    project.write(
        "docs/a.mara.md",
        &document(&[
            item(
                &mid(1),
                "Requirement",
                "REQ-FIRST",
                "First",
                &[],
                "The quokka sleeps\nunder the baobab.",
            ),
            item(
                &mid(2),
                "Requirement",
                "REQ-SECOND",
                "Second",
                &[],
                "A quokka alone.",
            ),
            "## Field notes\n\nThe wombat digs.\n".to_owned(),
            target,
            linking,
        ]),
    );

    // Both words in one body, one of them in another's: only the first, located, one excerpt.
    let results = &search(&project, &["quokka baobab"])["results"];
    assert_eq!(names(results), ["REQ-FIRST"]);
    let hit = &results[0];
    assert_eq!(
        (
            hit["kind"].as_str(),
            hit["path"].as_str(),
            hit["line"].as_u64()
        ),
        (Some("item"), Some("docs/a.mara.md"), Some(1))
    );
    assert_eq!(hit["excerpt"]["text"], "The quokka sleeps");
    assert_eq!(hit["excerpt"]["line"], 10);

    // A word only in a narrative unit: the unit, as `<path>:<line>`.
    let results = &search(&project, &["wombat"])["results"];
    let unit = line_of(&project, "docs/a.mara.md", "## Field notes");
    assert_eq!(names(results), [format!("docs/a.mara.md:{unit}")]);
    assert_eq!(results[0]["kind"], "narrative");
    assert_eq!(results[0]["heading"], "Field notes");

    // A word only in the body of an item that another links to: not the linking item.
    assert_eq!(found(&project, &["axolotl"]), ["REQ-TARGET"]);
}

/// A project for the filter cases: items of each type in two documents, and narrative.
pub fn filter_project() -> Project {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &document(&[
            item(
                &mid(1),
                "Need",
                "NEED-ONE",
                "Need one",
                &[],
                "A common need.",
            ),
            item(
                &mid(2),
                "Requirement",
                "REQ-ONE",
                "Requirement one",
                &[
                    ("Priority", "High"),
                    ("Owner", "Ann"),
                    ("Satisfies", &link("NEED-ONE", "", &mid(1))),
                ],
                "A common requirement.",
            ),
            "## Notes\n\nCommon narrative.\n".to_owned(),
        ]),
    );
    project.write(
        "docs/b.mara.md",
        &document(&[
            item(
                &mid(3),
                "Requirement",
                "REQ-TWO",
                "Requirement two",
                &[
                    ("Priority", "Low"),
                    ("Refines", &link("REQ-ONE", "a.mara.md", &mid(2))),
                ],
                "A common detail.",
            ),
            item(
                &mid(4),
                "Design",
                "DES-ONE",
                "Design one",
                &[("Satisfies", &link("REQ-TWO", "", &mid(3)))],
                "A common design.",
            ),
        ]),
    );
    project
}

/// Each filter, a value one edit away from it, and the items it matches in path and source
/// order.
pub const FILTER_CASES: &[(&str, &str, &str, &[&str])] = &[
    ("--type", "need", "nees", &["NEED-ONE"]),
    (
        "--type",
        "Requirement",
        "requirements",
        &["REQ-ONE", "REQ-TWO"],
    ),
    ("--field", "priority=high", "priority=hihg", &["REQ-ONE"]),
    ("--field", "Owner=Ann", "Owner=Anne", &["REQ-ONE"]),
    (
        "--relation",
        "satisfies",
        "satisfiez",
        &["REQ-ONE", "DES-ONE"],
    ),
    (
        "--relation",
        "satisfied_by",
        "satisfied_bu",
        &["NEED-ONE", "REQ-TWO"],
    ),
    (
        "--path",
        "docs/b.mara.md",
        "docs/b.mara.mb",
        &["REQ-TWO", "DES-ONE"],
    ),
    ("--id", "REQ-TWO", "REQ-TWP", &["REQ-TWO"]),
];

#[test]
fn filters_match_exactly() {
    let project = filter_project();
    for (option, value, near, expected) in FILTER_CASES {
        assert_eq!(
            found(&project, &["common", option, value]),
            *expected,
            "{option} {value}"
        );
        assert_eq!(
            found(&project, &["common", option, near]),
            [] as [&str; 0],
            "{option} {near}"
        );
    }
    // A path filter keeps narrative; a directory names the documents below it.
    let notes = line_of(&project, "docs/a.mara.md", "## Notes");
    assert_eq!(
        found(&project, &["common", "--path", "docs/a.mara.md"]),
        [
            "NEED-ONE".to_owned(),
            "REQ-ONE".to_owned(),
            format!("docs/a.mara.md:{notes}")
        ]
    );
    assert_eq!(found(&project, &["common", "--path", "docs"]).len(), 5);
    // Paths are read as validation reads them: the same nodes and the same cursor.
    for path in ["docs/", "./docs", "docs//", "docs/./"] {
        assert_eq!(
            found(&project, &["common", "--path", path]).len(),
            5,
            "{path}"
        );
    }
    let cursor = |path: &str| {
        let args = [
            "item", "list", "--limit", "1", "--path", path, "--format", "json",
        ];
        project.run(&args).ok().json()["next_cursor"].clone()
    };
    assert_eq!(cursor("./docs/"), cursor("docs"));
    // Values of one filter are alternatives; filters all apply.
    assert_eq!(
        found(&project, &["common", "--type", "need", "--type", "design"]),
        ["NEED-ONE", "DES-ONE"]
    );
    assert_eq!(
        found(
            &project,
            &[
                "common",
                "--type",
                "requirement",
                "--path",
                "docs/b.mara.md"
            ]
        ),
        ["REQ-TWO"]
    );
}

#[test]
fn exact_matches_first_then_ids_titles_and_headings() {
    let project = Project::new();
    let plain = |n: u32, id: &str, rows: &[(&str, &str)], body: &str| {
        item(
            &mid(n),
            "Requirement",
            id,
            &format!("Plain {n}"),
            rows,
            body,
        )
    };
    project.write(
        "docs/a.mara.md",
        &document(&[
            "# Falcon notes\n\nSome narrative.\n".to_owned(),
            plain(1, "REQ-A-ONE", &[], "We saw a falcon."),
            item(
                &mid(2),
                "Requirement",
                "REQ-A-TWO",
                "Falcon title",
                &[],
                "Nothing.",
            ),
            plain(3, "REQ-A-THREE", &[("Owner", "falcon")], "Nothing."),
        ]),
    );
    project.write(
        "docs/b.mara.md",
        &document(&[
            plain(4, "REQ-FALCON", &[], "Nothing."),
            plain(5, "REQ-B-TWO", &[], "A falcan flew."),
            plain(6, "REQ-B-THREE", &[], "## Falcno\n\nText."),
            item(
                &mid(7),
                "Requirement",
                "REQ-B-FOUR",
                "Falcons rest",
                &[],
                "Nothing.",
            ),
            plain(8, "REQ-B-FIVE", &[("Owner", "falcen")], "Nothing."),
        ]),
    );
    assert_eq!(
        found(&project, &["falcon"]),
        [
            // Exact, in an ID, a title or a heading, in path and source order.
            "docs/a.mara.md:1",
            "REQ-A-TWO",
            "REQ-FALCON",
            // Exact, in a body or a field.
            "REQ-A-ONE",
            "REQ-A-THREE",
            // One edit away, in a heading or a title.
            "REQ-B-THREE",
            "REQ-B-FOUR",
            // One edit away, in a body or a field.
            "REQ-B-TWO",
            "REQ-B-FIVE",
        ]
    );
}

#[test]
fn tolerates_one_edit_from_four_characters_and_two_from_eight() {
    let project = Project::new();
    let animals = [
        ("REQ-CAT", "cat"),
        ("REQ-LYNX", "lynx"),
        ("REQ-PANTHER", "a panther"),
        ("REQ-ELEPHANT", "an elephant"),
        ("REQ-HIPPO", "a hippopotamus"),
    ];
    let items: Vec<String> = animals
        .iter()
        .enumerate()
        .map(|(n, (id, body))| {
            item(
                &mid(n as u32 + 1),
                "Requirement",
                id,
                "Animal",
                &[],
                &format!("Body with {body}."),
            )
        })
        .collect();
    project.write("docs/a.mara.md", &document(&items));
    let cases: [(&str, &[&str]); 6] = [
        ("cot", &[]),
        ("lynz", &["REQ-LYNX"]),
        ("panthar", &["REQ-PANTHER"]),
        ("pxnthxr", &[]),
        ("elxphxnt", &["REQ-ELEPHANT"]),
        ("hxppxpxtamus", &[]),
    ];
    for (query, expected) in cases {
        assert_eq!(found(&project, &[query]), expected, "{query}");
    }
}

#[test]
fn ids_and_mids_match_only_exactly() {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &document(&[
            item(
                &mid(1),
                "Requirement",
                "REQ-ZORBLAX",
                "Unrelated title",
                &[],
                "Body.",
            ),
            item(&mid(2), "Requirement", "REQ-OTHER", "Other", &[], "Body."),
        ]),
    );
    assert_eq!(found(&project, &["REQ-ZORBLAX"]), ["REQ-ZORBLAX"]);
    assert_eq!(found(&project, &["REQ-ZORBLAQ"]), [] as [&str; 0]);
    let exact = mid(1);
    assert_eq!(found(&project, &[&exact]), ["REQ-ZORBLAX"]);
    let near = format!("{}Z", &exact[..25]);
    assert_eq!(found(&project, &[&near]), [] as [&str; 0]);
}

#[test]
fn a_heading_in_a_quote_or_a_list_ranks_as_a_heading() {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &item(
            &mid(1),
            "Requirement",
            "REQ-BODY",
            "Body only",
            &[],
            "The needle is here.",
        ),
    );
    project.write(
        "docs/b.mara.md",
        &document(&[
            item(
                &mid(2),
                "Requirement",
                "REQ-QUOTE",
                "Quoted",
                &[],
                "> #### needle\n>\n> Text.",
            ),
            item(
                &mid(3),
                "Requirement",
                "REQ-LIST",
                "Listed",
                &[],
                "- ### needle\n- other",
            ),
        ]),
    );
    assert_eq!(
        found(&project, &["needle"]),
        ["REQ-QUOTE", "REQ-LIST", "REQ-BODY"]
    );
}

#[test]
fn text_after_a_merged_anchor_is_searchable_in_its_item() {
    let project = Project::new();
    let anchored = item(
        &mid(1),
        "Requirement",
        "REQ-MERGED",
        "Merged",
        &[],
        "Body text.",
    );
    let merged = anchored.replacen(
        &format!("<a id=\"{}\"></a>", mid(1)),
        &format!("before <a id=\"{}\"></a> afterword", mid(1)),
        1,
    );
    project.write("docs/a.mara.md", &merged);
    let results = &search(&project, &["before"])["results"];
    assert_eq!(names(results), ["docs/a.mara.md:1"]);
    let unit = project
        .run(&["get", "docs/a.mara.md:1", "--format", "json"])
        .ok()
        .json();
    assert_eq!(unit["node"]["kind"], "narrative");
    assert_eq!(unit["content"], "before");
    // The text after the anchor starts the item's body: search, the excerpt and `get` show it.
    let results = &search(&project, &["afterword"])["results"];
    assert_eq!(names(results), ["REQ-MERGED"]);
    let excerpt = results[0]["excerpt"]["text"].as_str().unwrap();
    assert!(excerpt.contains("afterword"), "{excerpt}");
    let read = project
        .run(&["get", "REQ-MERGED", "--format", "json"])
        .ok()
        .json();
    let content = read["content"].as_str().unwrap();
    assert!(
        content.starts_with("afterword") && content.ends_with("Body text."),
        "{content}"
    );
    assert_eq!(read["node"]["line"], 1);
    // The item and its validation are as before: its anchor is merged, and nothing else.
    let validated = project.run(&["item", "validate", "REQ-MERGED", "--format", "json"]);
    let codes: Vec<String> = validated.json()["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| format!("{}@{}", d["code"].as_str().unwrap(), d["line"]))
        .collect();
    assert_eq!(codes, ["merged-anchor@1"]);
}

#[test]
fn the_excerpt_shows_the_words_that_search_matched() {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &document(&[
            item(
                &mid(1),
                "Requirement",
                "REQ-SPLIT",
                "Title",
                &[],
                "Other text.\n\nAn inter**nation**al phrase.",
            ),
            item(
                &mid(2),
                "Requirement",
                "REQ-DESTINATION",
                "Title",
                &[],
                "See [the page](https://example.com/zeppelin) first.\n\nA zeppelin flies.",
            ),
            item(
                &mid(3),
                "Requirement",
                "REQ-TYPO",
                "Title",
                &[],
                "Nothing here.\n\nA nearly maching word.",
            ),
        ]),
    );
    let excerpt = |query: &str| -> (String, String) {
        let results = search(&project, &[query])["results"].clone();
        let hit = &results[0];
        (
            hit["id"].as_str().unwrap().to_owned(),
            hit["excerpt"]["text"].as_str().unwrap().to_owned(),
        )
    };
    assert_eq!(
        excerpt("international"),
        ("REQ-SPLIT".into(), "An inter**nation**al phrase.".into())
    );
    assert_eq!(
        excerpt("zeppelin"),
        ("REQ-DESTINATION".into(), "A zeppelin flies.".into())
    );
    assert_eq!(
        excerpt("matching"),
        ("REQ-TYPO".into(), "A nearly maching word.".into())
    );
}

#[test]
fn every_excerpt_is_the_line_of_its_word_in_every_zone() {
    let destination = "destination".repeat(100);
    // Contexts on one line, possible in every zone, then contexts over two lines, possible only
    // in a paragraph.
    let single: [(&str, String); 5] = [
        (
            "after a long link destination",
            format!("[label](https://example.test/{destination}) NEEDLE"),
        ),
        (
            "inside emphasis",
            "Words then *emphasised NEEDLE* text".to_owned(),
        ),
        (
            "after an image",
            format!("![alt](https://example.test/{destination}.png) NEEDLE"),
        ),
        (
            "after inline HTML",
            format!("<span title=\"{}\">x</span> NEEDLE", "t".repeat(300)),
        ),
        ("after a code span", format!("`{}` NEEDLE", "c".repeat(300))),
    ];
    let multiple: [(&str, &str); 5] = [
        ("a code span over two lines", "`aaaa\nNEEDLE` and more"),
        (
            "a soft break inside emphasis",
            "*emphasised\nNEEDLE* and more",
        ),
        (
            "a hard break inside emphasis",
            "*emphasised  \nNEEDLE* and more",
        ),
        (
            "link text over two lines",
            "[link\nNEEDLE text](https://example.test/x) and more",
        ),
        (
            "inline HTML over two lines",
            "<span\ntitle=\"x\">NEEDLE</span> and more",
        ),
    ];
    let mut contexts: Vec<(&str, String, bool)> =
        single.iter().map(|(c, t)| (*c, t.clone(), false)).collect();
    contexts.extend(multiple.iter().map(|(c, t)| (*c, (*t).to_owned(), true)));
    let zones = [
        "title",
        "heading",
        "metadata",
        "body",
        "narrative",
        "after a merged anchor",
    ];
    for ending in ["\n", "\r\n"] {
        let project = Project::new();
        let mut regions = Vec::new();
        let mut cases = Vec::new();
        for (z, zone) in zones.iter().enumerate() {
            for (c, (context, text, lines)) in contexts.iter().enumerate() {
                if *lines && !matches!(*zone, "body" | "narrative" | "after a merged anchor") {
                    continue;
                }
                // A word of three letters matches only exactly.
                let needle = format!("q{}{}", (b'a' + z as u8) as char, (b'a' + c as u8) as char);
                let text = text.replace("NEEDLE", &needle);
                let n = (z * contexts.len() + c) as u32 + 1;
                let id = format!("REQ-CASE-{n}");
                let region = match *zone {
                    "title" => item(&mid(n), "Requirement", &id, &text, &[], "Body."),
                    "heading" => item(
                        &mid(n),
                        "Requirement",
                        &id,
                        "Plain",
                        &[],
                        &format!("Intro.\n\n#### {text}\n\nMore."),
                    ),
                    "metadata" => item(
                        &mid(n),
                        "Requirement",
                        &id,
                        "Plain",
                        &[("Owner", &text)],
                        "Body.",
                    ),
                    "body" => item(
                        &mid(n),
                        "Requirement",
                        &id,
                        "Plain",
                        &[],
                        &format!("Intro.\n\n{text}\n\nMore."),
                    ),
                    "narrative" => format!("## Notes {n}\n\nIntro.\n\n{text}\n"),
                    _ => item(&mid(n), "Requirement", &id, "Plain", &[], "Body.").replacen(
                        &format!("<a id=\"{}\"></a>", mid(n)),
                        &format!("before <a id=\"{}\"></a> {text}", mid(n)),
                        1,
                    ),
                };
                regions.push(region);
                cases.push((format!("{zone}, {context}, {ending:?}"), needle, *context));
            }
        }
        let text = document(&regions).replace('\n', ending);
        project.write("docs/a.mara.md", &text);
        for (case, needle, context) in cases {
            let results = &search(&project, &[&needle])["results"];
            assert_eq!(results.as_array().unwrap().len(), 1, "{case}");
            let excerpt = &results[0]["excerpt"];
            let shown = excerpt["text"].as_str().unwrap();
            let needle_at = text.find(&needle).unwrap();
            if context == "a code span over two lines" {
                // By the maintainer's decision, a word in a code span over several lines has the
                // position of the code span's start, so its excerpt is the line where the span
                // starts and may not hold the word; this includes the case after a merged
                // anchor that started the fourth review.
                let span_at = text[..needle_at].rfind('`').unwrap();
                let line = 1 + text[..span_at].matches('\n').count();
                assert_eq!(excerpt["line"], line, "{case}: {shown:?}");
                continue;
            }
            let at = shown
                .find(&needle)
                .unwrap_or_else(|| panic!("{case}: {shown:?}"));
            let before = shown[..at].chars().count();
            assert!(
                before <= 60,
                "{case}: the excerpt starts {before} characters before the word"
            );
            let line = 1 + text[..needle_at].matches('\n').count();
            assert_eq!(excerpt["line"], line, "{case}: {shown:?}");
        }
    }
}

#[test]
fn a_query_word_of_more_than_18_characters_matches_only_exactly() {
    let project = Project::new();
    let words = [
        ("REQ-EIGHTEEN", "abcdefghijklmnopqr"),
        ("REQ-NINETEEN", "abcdefghijklmnopqrs"),
        ("REQ-TWENTY", "abcdefghijklmnopqrst"),
    ];
    let items: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(n, (id, word))| {
            item(
                &mid(n as u32 + 1),
                "Requirement",
                id,
                "Long",
                &[],
                &format!("Holds {word}."),
            )
        })
        .collect();
    project.write("docs/a.mara.md", &document(&items));
    // Eighteen characters: within two edits of words up to 20 characters long.
    assert_eq!(
        found(&project, &["abcdefghijklmnopqr"]),
        ["REQ-EIGHTEEN", "REQ-NINETEEN", "REQ-TWENTY"]
    );
    assert_eq!(
        found(&project, &["abcdefghijklmnopxr"]),
        ["REQ-EIGHTEEN", "REQ-NINETEEN"]
    );
    // Nineteen characters: only the word itself.
    assert_eq!(found(&project, &["abcdefghijklmnopqrs"]), ["REQ-NINETEEN"]);
    assert_eq!(found(&project, &["abcdefghijklmnopqrx"]), [] as [&str; 0]);
    assert_eq!(found(&project, &["abcdefghijklmnopqrst"]), ["REQ-TWENTY"]);
}

#[test]
fn an_empty_query_reads_a_document_of_unicode_spaces() {
    let project = Project::new();
    project.write("docs/a.mara.md", "\u{a0}");
    let page = search(&project, &[""]);
    let results = page["results"].as_array().unwrap();
    assert!(
        results.iter().all(|hit| hit["excerpt"]["text"] == ""),
        "{page}"
    );
}

#[test]
fn a_code_span_over_two_lines_is_indexed_as_rendered() {
    let project = Project::new();
    project.write("docs/a.mara.md", "x` a\nb `y");
    // CommonMark reads `x`, `a b` and `y`: the words `xa` and `by`.
    assert_eq!(found(&project, &["xa"]), ["docs/a.mara.md:1"]);
    assert_eq!(found(&project, &["by"]), ["docs/a.mara.md:1"]);
}
