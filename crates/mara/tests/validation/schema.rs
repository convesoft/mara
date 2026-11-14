//! VER-CUSTOM-SCHEMA: validation against a schema of the project's own, with none of the
//! `engineering` template's vocabulary.

use crate::{Dir, at, document, item, json, line_of};

const SCHEMA: &str = r#"format_version = 1
statuses = ["open", "closed"]

[types.story]
id_prefix = "STORY-"
description = "Something a user wants."

[types.story.fields.points]
kind = "enum"
values = ["one", "three", "five"]

[types.story.fields.component]
kind = "text"

[types.chore]
id_prefix = "CHORE-"
description = "Work without a user story."

[relations.blocks]
inverse = "blocked_by"
description = "The source must finish first."
targets = { story = ["story"], chore = ["story"] }
"#;

const A: &str = "01M7DM832Z7QD24R85CQBJ6J44";
const B: &str = "01M7DM8335BK6ZZXCAHJ3P0GBF";
const C: &str = "01M7DM833D9C3R9A8C6NW99RC5";
const D: &str = "01M7DM833RK33MPM2MCA91EW6Q";

#[test]
fn items_of_the_declared_vocabulary_are_valid() {
    let dir = Dir::project(SCHEMA);
    let stories = document(
        "Stories",
        &[
            item(
                A,
                "Sign in",
                "Story",
                "| ID | STORY-SIGN-IN |\n| Status | Open |\n| Points | Three |\n| Component | Auth |\n",
                "Users sign in.",
            ),
            item(
                B,
                "Sign out",
                "story",
                &format!(
                    "| ID | STORY-SIGN-OUT |\n| Status | closed |\n| Blocks | [STORY-SIGN-IN](#{A}) |\n"
                ),
                "Users sign out.",
            ),
            // A type without fields, with only `Type`, `ID` and `Status`.
            item(
                C,
                "Clean up",
                "Chore",
                "| ID | CHORE-CLEAN |\n| Status | Open |\n",
                "Tidy.",
            ),
        ],
    );
    dir.write("docs/stories.mara.md", stories);
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!((run.status, run.diagnostics()), (0, vec![]), "{run:?}");
}

#[test]
fn the_engineering_vocabulary_and_values_outside_the_lists_are_errors() {
    let dir = Dir::project(SCHEMA);
    let text = document(
        "Mixed",
        &[
            item(
                A,
                "An engineering requirement",
                "Requirement",
                "| ID | REQ-ONE |\n| Status | Open |\n",
                "Not a type here.",
            ),
            item(
                B,
                "Engineering keys",
                "Story",
                &format!(
                    "| ID | STORY-KEYS |\n| Status | Open |\n| Likelihood | Rare |\n| Satisfies | [STORY-OTHER](#{D}) |\n"
                ),
                "Keys of another vocabulary.",
            ),
            item(
                C,
                "Values outside the lists",
                "Story",
                "| ID | STORY-VALUES |\n| Status | Accepted |\n| Points | Eight |\n",
                "Neither is declared.",
            ),
            item(
                D,
                "Other",
                "Story",
                "| ID | STORY-OTHER |\n| Status | Open |\n",
                "Target.",
            ),
        ],
    );
    dir.write("docs/mixed.mara.md", &text);
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.status, 1);
    let path = "docs/mixed.mara.md";
    assert_eq!(
        run.diagnostics(),
        [
            at(
                "undeclared-type",
                path,
                line_of(&text, "| Type | Requirement |")
            ),
            at("undeclared-key", path, line_of(&text, "| Likelihood")),
            at("undeclared-key", path, line_of(&text, "| Satisfies")),
            at(
                "invalid-value",
                path,
                line_of(&text, "| Status | Accepted |")
            ),
            at("invalid-value", path, line_of(&text, "| Points | Eight |")),
        ]
    );
}

#[test]
fn adding_a_rejected_value_to_the_schema_makes_the_document_valid() {
    let dir = Dir::project(SCHEMA);
    let text = document(
        "Story",
        &[item(
            A,
            "Big story",
            "Story",
            "| ID | STORY-BIG |\n| Status | Open |\n| Points | Eight |\n",
            "Large.",
        )],
    );
    dir.write("docs/story.mara.md", &text);
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(
        (run.status, run.diagnostics()),
        (
            1,
            vec![at(
                "invalid-value",
                "docs/story.mara.md",
                line_of(&text, "| Points")
            )]
        )
    );
    dir.write(
        ".mara/schema.toml",
        SCHEMA.replace(
            r#"["one", "three", "five"]"#,
            r#"["one", "three", "five", "eight"]"#,
        ),
    );
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!((run.status, run.diagnostics()), (0, vec![]), "{run:?}");
}
