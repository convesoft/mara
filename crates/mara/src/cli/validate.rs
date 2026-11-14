//! `project validate`, `item validate` and `schema validate`: REQ-REPORT-EVERY-PROBLEM,
//! REQ-VALIDITY-COVERS-WHOLE-PROJECT and DES-DIAGNOSTICS in `docs/validation.mara.md`.
//!
//! Validation is never refused: configuration and schema errors are reported with the rest
//! (DES-PROJECT-CONFIGURATION).

use std::fmt::Write as _;

use mara::inputs::Inputs;
use mara::project::{self, Project};
use mara_engine::config::CONFIG_PATH;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::is_mid;
use mara_engine::paging::PAGE_BYTES;
use mara_engine::validation::{self, PageError, Report, result_key};

use super::{
    Args, CURSOR, Context, FAILURE, Failure, LIMIT, Operation, Opt, Response, SUCCESS, project_path,
};

pub const PROJECT: Operation = Operation {
    words: &["project", "validate"],
    arguments: &[],
    options: &[PATH, LIMIT, CURSOR],
    run: project_validate,
};

pub const ITEM: Operation = Operation {
    words: &["item", "validate"],
    arguments: &["id-or-mid"],
    options: &[LIMIT, CURSOR],
    run: item_validate,
};

pub const SCHEMA: Operation = Operation {
    words: &["schema", "validate"],
    arguments: &[],
    options: &[LIMIT, CURSOR],
    run: schema_validate,
};

/// `--path <path>`: report only diagnostics in this file or directory, relative to the project
/// root ([`project_path`]). Repeatable.
pub(super) const PATH: Opt = Opt {
    name: "path",
    value: "path",
    repeatable: true,
    check: |path| project_path(path).map(drop),
};

/// Every problem of the project: configuration, schema, inventory and built-in checks.
fn project_validate(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let root = context.root()?;
    let generation = Inputs::new(root).snapshot(context.now, context.work);
    let findings = validation::check(&generation.snapshot, context.work);
    // One key for every spelling of the same paths (DES-DIAGNOSTICS).
    let mut paths: Vec<String> = args
        .values(PATH.name)
        .map(|path| project_path(path).expect("checked before the operation ran"))
        .collect();
    paths.sort_unstable();
    paths.dedup();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let mut diagnostics = generation.project.diagnostics.clone();
    diagnostics.extend(generation.diagnostics.iter().cloned());
    let mut request = vec!["project validate"];
    request.extend(&paths);
    let key = result_key(&request, &[Some(generation.snapshot.key().0)], &diagnostics);
    let primary = findings.into_iter().filter(|finding| finding.primary);
    diagnostics.extend(primary.map(|finding| finding.diagnostic));
    respond(Report::new(diagnostics, key), &paths, args)
}

/// The problems of one item, named by its ID or MID, checked against the whole project.
fn item_validate(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let reference = args.argument(0);
    let root = context.root()?;
    let generation = Inputs::new(root).snapshot(context.now, context.work);
    let snapshot = &generation.snapshot;
    let mut items = match is_mid(reference) {
        true => snapshot.items_with_mid(reference),
        false => snapshot.items_with_id(reference),
    }
    .to_vec();
    items.dedup();
    let item = match items[..] {
        [item] => item,
        [] => {
            let message = format!("no item has the ID or MID `{reference}`");
            return Err(Failure::new("unknown-item", message));
        }
        ref several => {
            let places = several.iter().map(|&item| {
                let path = &snapshot.documents()[snapshot.items()[item].document].path;
                format!("`{path}:{}`", snapshot.item(item).line)
            });
            let other = if is_mid(reference) { "ID" } else { "MID" };
            let message = format!(
                "{} items have `{reference}`, at {}: name one by its {other}",
                several.len(),
                validation::list(places, several.len())
            );
            return Err(Failure::new("ambiguous-item", message));
        }
    };
    let findings = validation::check(snapshot, context.work);
    let diagnostics = findings
        .into_iter()
        .filter(|finding| finding.item == Some(item))
        .map(|finding| finding.diagnostic)
        .collect();
    let key = result_key(
        &["item validate", reference],
        &[Some(snapshot.key().0)],
        &[],
    );
    respond(Report::new(diagnostics, key), &[], args)
}

/// The problems of the schema, and those of the configuration that keep it from being read.
/// Reads no document.
fn schema_validate(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let root = context.root()?;
    let project = project::load(root, context.work);
    let key = result_key(
        &["schema validate"],
        &[project.config_hash, project.schema_hash],
        &project.diagnostics,
    );
    respond(Report::new(schema_problems(&project), key), &[], args)
}

/// The schema file's diagnostics and the configuration's at the line that names the schema;
/// every configuration diagnostic when the configuration names no schema it could read.
fn schema_problems(project: &Project) -> Vec<Diagnostic> {
    let concerns_schema = |diagnostic: &&Diagnostic| match &project.config.schema {
        Some(setting) => {
            diagnostic.path == setting.path
                || (diagnostic.path == CONFIG_PATH && diagnostic.line == setting.line)
        }
        None => true,
    };
    project
        .diagnostics
        .iter()
        .filter(concerns_schema)
        .cloned()
        .collect()
}

/// The page that `args` ask for, in both forms, with the exit status of the whole report.
fn respond(report: Report, paths: &[&str], args: &Args) -> Result<Response, Failure> {
    let page = report
        .page(paths, args.cursor().as_ref(), args.limit())
        .map_err(|error| match error {
            PageError::StaleCursor => Failure::new(
                "stale-cursor",
                "the cursor belongs to another result: the project or the request changed since \
                 the page that returned it; validate again without `--cursor`"
                    .to_owned(),
            ),
            PageError::TooLarge { path, line } => Failure::new(
                "record-too-large",
                format!(
                    "the diagnostic at {path}:{line} does not fit a page of {PAGE_BYTES} bytes"
                ),
            ),
        })?;
    let mut human = String::new();
    for diagnostic in &page.diagnostics {
        writeln!(human, "{diagnostic}").expect("writing to a String cannot fail");
    }
    let verdict = if report.valid() { "valid" } else { "invalid" };
    writeln!(
        human,
        "{verdict}: {}, {}",
        count(report.errors(), "error"),
        count(report.warnings(), "warning")
    )
    .expect("writing to a String cannot fail");
    if let Some(cursor) = page.next_cursor {
        writeln!(human, "more diagnostics: repeat with `--cursor {cursor}`")
            .expect("writing to a String cannot fail");
    }
    Ok(Response {
        json: page.to_json(),
        human,
        status: if report.valid() { SUCCESS } else { FAILURE },
    })
}

/// `1 error`, `2 errors`.
fn count(number: usize, noun: &str) -> String {
    match number {
        1 => format!("1 {noun}"),
        number => format!("{number} {noun}s"),
    }
}
