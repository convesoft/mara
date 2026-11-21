//! The MCP side of VER-AGENT-GUIDE: the instructions of the real stdio server, started inside
//! and outside disposable projects, compared with its tool list and with `mara prime`.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;

use crate::{Dir, Server, cli};

/// The heading that opens the full guide in both renderings (DES-AGENT-GUIDE).
const HEADING: &str = "# Mara workflow\n";
/// Its sections, in order.
const SECTIONS: [&str; 5] = [
    "## Find and read",
    "## Author",
    "## Repair after hand edits",
    "## Link code",
    "## Validate",
];
/// The tools the full guide leaves to the notes.
const NOTE_TOOLS: [&str; 2] = ["project_init", "project_convert"];
/// The most characters of the instructions (DES-AGENT-GUIDE).
const LIMIT: usize = 3_800;

/// What a server started in `directory` with `options` tells a client.
struct Seen {
    instructions: String,
    /// Each tool's name and parameters.
    tools: Vec<(String, BTreeSet<String>)>,
}

impl Seen {
    fn of(directory: &Path, options: &[&str]) -> Seen {
        let mut server = Server::spawn(directory, options);
        let response = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "mara-tests", "version": "1"},
            }),
        );
        let instructions = response["result"]["instructions"]
            .as_str()
            .unwrap_or_else(|| panic!("no instructions: {response}"))
            .to_owned();
        server.send_line(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        );
        let tools = (server.tools().iter())
            .map(|tool| {
                let name = tool["name"].as_str().unwrap().to_owned();
                let properties = tool["inputSchema"]["properties"].as_object().unwrap();
                (name, properties.keys().cloned().collect())
            })
            .collect();
        let (status, _) = server.finish();
        assert!(status.success());
        let length = instructions.chars().count();
        assert!(length <= LIMIT, "{length} characters:\n{instructions}");
        assert!(!instructions.contains("{{"), "{instructions}");
        Seen {
            instructions,
            tools,
        }
    }

    /// The tools the instructions name.
    fn tools_named(&self) -> BTreeSet<String> {
        let spans = spans(&self.instructions);
        (self.tools.iter())
            .map(|(name, _)| name.clone())
            .filter(|name| spans.contains(name))
            .collect()
    }

    /// Requires every tool and parameter that the instructions name to be listed: each code
    /// span that `prime`, given the same project, does not have is a name.
    fn check_names(&self, prime: &str, bound: bool) {
        let listed: BTreeSet<String> = (self.tools.iter())
            .flat_map(|(name, parameters)| parameters.iter().chain([name]))
            .cloned()
            .collect();
        let mut names: BTreeSet<String> = &spans(&self.instructions) - &spans(prime);
        if bound {
            // The bound server says that its tools take no `project`, and lists none.
            assert!(!listed.contains("project"), "{listed:?}");
            names.remove("project");
        }
        let unlisted: Vec<_> = names.difference(&listed).collect();
        assert!(unlisted.is_empty(), "named but not listed: {unlisted:?}");
    }
}

/// The text of each code span of `text`.
fn spans(text: &str) -> BTreeSet<String> {
    (text.split('`').skip(1).step_by(2))
        .map(str::to_owned)
        .collect()
}

/// `mara prime` in `directory` with `options`.
fn prime(directory: &Path, options: &[&str]) -> String {
    let run = cli(directory, &[options, &["prime"]].concat());
    assert_eq!(run.status, 0, "{run:?}");
    run.json()["text"].as_str().unwrap().to_owned()
}

fn full_guide(seen: &Seen) {
    let text = &seen.instructions;
    assert!(text.starts_with(HEADING), "{text}");
    let mut at = 0;
    for section in SECTIONS {
        let found = text[at..].find(&format!("\n{section}\n"));
        at += found.unwrap_or_else(|| panic!("no section {section:?} in order:\n{text}"));
    }
}

#[test]
fn in_a_project_the_full_guide_names_every_tool_it_needs_and_only_listed_ones() {
    let dir = Dir::project("engineering");
    let root = dir.root();
    let named = root.to_str().unwrap();
    let outside = Dir::new();
    let unbound = Seen::of(&root, &[]);
    let bound = Seen::of(&outside.root(), &["--project", named]);
    for (seen, bound, prime) in [
        (&unbound, false, prime(&root, &[])),
        (&bound, true, prime(&outside.root(), &["--project", named])),
    ] {
        full_guide(seen);
        seen.check_names(&prime, bound);
        let expected: BTreeSet<String> = (seen.tools.iter())
            .map(|(name, _)| name.clone())
            .filter(|name| !NOTE_TOOLS.contains(&name.as_str()))
            .collect();
        assert_eq!(seen.tools_named(), expected, "{}", seen.instructions);
        // Operations appear as tools, not as commands.
        assert!(
            !seen.instructions.contains("`mara "),
            "{}",
            seen.instructions
        );
    }
    // Only the sentence on the project differs between the bound and the unbound server.
    let differing: Vec<_> = (unbound.instructions.lines())
        .filter(|line| !bound.instructions.lines().any(|other| other == *line))
        .collect();
    assert_eq!(differing.len(), 1, "{differing:?}");
    // Found from a subdirectory, the same instructions.
    dir.write("docs/sub/a.mara.md", "Narrative.\n");
    let below = Seen::of(&root.join("docs/sub"), &[]);
    assert_eq!(below.instructions, unbound.instructions);
}

#[test]
fn outside_every_project_only_initialization_is_named() {
    let outside = Dir::new();
    let empty = Dir::new();
    let empty_path = empty.root();
    let empty_path = empty_path.to_str().unwrap();
    for (options, bound) in [(&[][..], false), (&["--project", empty_path][..], true)] {
        let seen = Seen::of(&outside.root(), options);
        assert!(
            !seen.instructions.contains(HEADING.trim_end()),
            "{}",
            seen.instructions
        );
        seen.check_names(&prime(&outside.root(), options), bound);
        let expected = BTreeSet::from(["project_init".to_owned()]);
        assert_eq!(seen.tools_named(), expected, "{}", seen.instructions);
    }
}

#[test]
fn a_legacy_project_gets_only_its_conversion() {
    let dir = Dir::new();
    dir.write(".mara/project.toml", "");
    let seen = Seen::of(&dir.root(), &[]);
    assert!(
        !seen.instructions.contains(HEADING.trim_end()),
        "{}",
        seen.instructions
    );
    seen.check_names(&prime(&dir.root(), &[]), false);
    let expected = BTreeSet::from(["project_convert".to_owned()]);
    assert_eq!(seen.tools_named(), expected, "{}", seen.instructions);
}

#[test]
fn the_instructions_depend_on_no_schema_and_no_valid_configuration() {
    let engineering = Seen::of(&Dir::project("engineering").root(), &[]);
    let empty = Seen::of(&Dir::project("empty").root(), &[]);
    assert_eq!(empty.instructions, engineering.instructions);
    let broken = Dir::project("engineering");
    broken.write(".mara/config.toml", "format_version = 2\nunknown = [\n");
    broken.write(".mara/schema.toml", "not toml at all = = =\n");
    let seen = Seen::of(&broken.root(), &[]);
    assert_eq!(seen.instructions, engineering.instructions);
}

impl Dir {
    /// A directory initialized with `template`.
    fn project(template: &str) -> Dir {
        let dir = Dir::new();
        let run = cli(&dir.root(), &["project", "init", "--template", template]);
        assert_eq!(run.status, 0, "{run:?}");
        dir
    }
}
