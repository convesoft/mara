//! `prime` and the agent guide: REQ-AGENT-GUIDE and DES-AGENT-GUIDE in `docs/agents.mara.md`.
//!
//! One template, rendered by [`render`] for each surface: `prime` prints it, and the MCP server
//! sends it as its instructions. In the template, `{{<words> <arguments>}}` names an operation
//! and `{{--<name>}}` an option. A line that starts with `cli: ` or `mcp: ` belongs to that
//! surface only; `mcp-bound: ` and `mcp-unbound: ` to a server started with or without
//! `--project`.

use std::path::Path;

use mara::project::{self, SelectError};
use mara_engine::config::Root;
use mara_engine::json::Object;

use super::{Args, Context, FORMAT_VERSION, Failure, Operation, Response, Run, SUCCESS};

pub const PRIME: Operation = Operation {
    words: &["prime"],
    summary: "Prints the agent guide for the project, or how to initialize one.",
    arguments: &[],
    options: &[],
    run: Run::Command(prime),
};

/// Where the guide is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// `mara prime`: operations as commands.
    Cli,
    /// The MCP server's instructions: operations as tools, whose parameters are the CLI's
    /// arguments and options. A server is bound when started with `--project`; an unbound one
    /// takes the project on each call.
    Mcp { bound: bool },
}

/// The full guide, for a project.
const GUIDE: &str = include_str!("guide.md");

/// Outside every project: how to initialize one, and nothing more
/// (ADR-INIT-NOTE-WITHOUT-PROJECT).
const NO_PROJECT: &str = "\
No Mara project was found here.

cli: To make one, run `{{project init}}` in the directory that becomes the project root, or `{{project init}} --project <dir>`. It writes the `engineering` template, the types and relations of engineering documentation; `{{--template}}` `empty` writes a schema without types. Then run this command again for the workflow.
mcp: To make one, call `{{project init}}`. It writes the `engineering` template, the types and relations of engineering documentation; `{{--template}}` `empty` writes a schema without types. The workflow comes with these instructions once the server starts in a project.
mcp-unbound: `{{--project}}` names the directory that becomes the project root; without it, it is the directory the server started in.
";

/// In a 0.3.0 project, which only the conversion reads (DES-PROJECT-CONFIGURATION).
const LEGACY: &str = "\
This is a Mara 0.3.0 project (`.mara/project.toml`), which this Mara only converts.

cli: Run `{{project convert}}`: it rewrites the `:::mara` blocks in place as items, writes `.mara/config.toml` and the schema, and reports what it changed or left out. Review the report and the diff, then run this command again for the workflow.
mcp: Call `{{project convert}}`: it rewrites the `:::mara` blocks in place as items, writes `.mara/config.toml` and the schema, and reports what it changed or left out. Review the report and the diff; the workflow comes with these instructions once the server starts in the converted project.
";

// @mara implements REQ-AGENT-GUIDE
/// The guide for the project found from `start`, or named by `project` relative to it, as
/// `surface` shows it: the full guide in a project, a note on initializing one outside every
/// project, and a note on converting a 0.3.0 project. A project whose configuration has an
/// error gets the full guide. Reads no file.
pub fn render(
    surface: Surface,
    start: &Path,
    project: Option<&Path>,
) -> Result<String, SelectError> {
    let template = match project::find(start, project) {
        Ok((_, Root::Project)) => GUIDE,
        Ok((_, Root::Legacy)) => LEGACY,
        Err(SelectError::NoProject { .. } | SelectError::NotAProject { .. }) => NO_PROJECT,
        Err(error) => return Err(error),
    };
    Ok(fill(template, surface))
}

/// `template` with the lines of the other surface left out and every name written as
/// `surface` writes it.
fn fill(template: &str, surface: Surface) -> String {
    let own: &[&str] = match surface {
        Surface::Cli => &["cli: "],
        Surface::Mcp { bound: true } => &["mcp: ", "mcp-bound: "],
        Surface::Mcp { bound: false } => &["mcp: ", "mcp-unbound: "],
    };
    let mut out = String::with_capacity(template.len());
    for line in template.split_inclusive('\n') {
        let line = match SURFACE_PREFIXES.iter().find(|p| line.starts_with(**p)) {
            Some(prefix) if own.contains(prefix) => &line[prefix.len()..],
            Some(_) => continue,
            None => line,
        };
        let mut rest = line;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let (name, after) = rest[start + 2..]
                .split_once("}}")
                .expect("every `{{` of the guide is closed");
            out.push_str(&name_for(name, surface));
            rest = after;
        }
        out.push_str(rest);
    }
    out
}

/// The prefixes of lines that belong to one surface.
const SURFACE_PREFIXES: [&str; 4] = ["cli: ", "mcp: ", "mcp-bound: ", "mcp-unbound: "];

/// An operation with its arguments, `item move <item> <path>`, or an option, `--line`, as
/// `surface` names it: the command and option on the CLI; in MCP the tool, whose schema
/// carries the arguments, and the parameter, named as the option.
fn name_for(name: &str, surface: Surface) -> String {
    match (surface, name.strip_prefix("--")) {
        (Surface::Cli, Some(_)) => name.to_owned(),
        (Surface::Mcp { .. }, Some(option)) => option.to_owned(),
        (Surface::Cli, None) => format!("mara {name}"),
        (Surface::Mcp { .. }, None) => {
            let words: Vec<&str> = (name.split(' '))
                .take_while(|word| !word.starts_with('<'))
                .collect();
            words.join("_")
        }
    }
}

/// Prints the guide for the project of the working directory or `--project`.
fn prime(context: &mut Context, _: &Args) -> Result<Response, Failure> {
    let start = context.cwd()?.to_owned();
    let text = render(Surface::Cli, &start, context.project.as_deref())?;
    let mut json = String::new();
    let mut object = Object::new(&mut json);
    object.number("format_version", FORMAT_VERSION);
    object.string("text", &text);
    object.end();
    Ok(Response {
        json,
        human: text,
        status: SUCCESS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUND: Surface = Surface::Mcp { bound: true };
    const UNBOUND: Surface = Surface::Mcp { bound: false };

    #[test]
    fn renders_operations_as_commands_or_tools() {
        let line = "Run `{{item move <item> <path>}}` with `{{--clear-field}}`.\n";
        assert_eq!(
            fill(line, Surface::Cli),
            "Run `mara item move <item> <path>` with `--clear-field`.\n"
        );
        assert_eq!(fill(line, BOUND), "Run `item_move` with `clear-field`.\n");
        let lines = "both\ncli: CLI\nmcp: MCP\nmcp-bound: bound\nmcp-unbound: unbound\n";
        assert_eq!(fill(lines, Surface::Cli), "both\nCLI\n");
        assert_eq!(fill(lines, BOUND), "both\nMCP\nbound\n");
        assert_eq!(fill(lines, UNBOUND), "both\nMCP\nunbound\n");
    }
}
