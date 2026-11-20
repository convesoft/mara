//! `project validate`, `item validate` and `schema validate`: REQ-REPORT-EVERY-PROBLEM,
//! REQ-VALIDITY-COVERS-WHOLE-PROJECT and DES-DIAGNOSTICS in `docs/validation.mara.md`.
//!
//! Validation is never refused: configuration and schema errors are reported with the rest
//! (DES-PROJECT-CONFIGURATION).

use std::fmt::Write as _;

use mara::inputs::CodeReads;
use mara::project::{self, Project};
use mara::rules::{self, Scope};
use mara_engine::code::CodeLinks;
use mara_engine::config::CONFIG_PATH;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::facts::CodeFacts;
use mara_engine::hash::Hash;
use mara_engine::paging::PAGE_BYTES;
use mara_engine::validation::{self, PageError, Report, result_key};

use super::{
    Args, Argument, CURSOR, Context, FAILURE, Failure, Kind, LIMIT, Operation, Opt, Response, Run,
    SUCCESS, project_path,
};

pub const PROJECT: Operation = Operation {
    words: &["project", "validate"],
    summary: "Reports every problem of the project: configuration, schema, documents, built-in \
              checks and invariants, and compiles every program. Also runs on a project whose \
              configuration or schema has errors.",
    arguments: &[],
    options: &[PATH, LIMIT, CURSOR],
    run: Run::Document(project_validate),
};

pub const ITEM: Operation = Operation {
    words: &["item", "validate"],
    summary: "Reports the problems of one item, and those of the configuration and of the programs \
              its invariants use.",
    arguments: &[Argument {
        name: "id-or-mid",
        help: "The item's ID or MID.",
    }],
    options: &[LIMIT, CURSOR],
    run: Run::Document(item_validate),
};

pub const SCHEMA: Operation = Operation {
    words: &["schema", "validate"],
    summary: "Reports the problems of the schema file, and of the configuration's line that \
              names it.",
    arguments: &[],
    options: &[LIMIT, CURSOR],
    run: Run::Document(schema_validate),
};

/// `--path <path>`: report only diagnostics in this file or directory, relative to the project
/// root ([`project_path`]). Repeatable.
pub(super) const PATH: Opt = Opt {
    name: "path",
    value: "path",
    help: "Only what is in this document, or in the documents below this directory, relative to \
           the project root.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |path| project_path(path).map(drop),
};

/// The keys of what a validation read: the snapshot, every declared program file, and each
/// evaluation.
fn input_keys(generation: &mara::inputs::Generation, evaluations: &[Hash]) -> Vec<Option<Hash>> {
    let mut programs = Vec::new();
    for (path, contents) in &generation.programs {
        programs.extend_from_slice(Hash::of_bytes(path.as_bytes()).as_bytes());
        programs.extend_from_slice(Hash::of_bytes(contents).as_bytes());
    }
    let mut keys = vec![
        Some(generation.snapshot.key().0),
        Some(Hash::of_bytes(&programs)),
    ];
    keys.extend(evaluations.iter().copied().map(Some));
    keys
}

/// Every problem of the project: configuration, schema, inventory, built-in checks, code
/// markers (DES-CODE-READERS), programs and invariants.
fn project_validate(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let root = context.root()?;
    let generation = context
        .inputs(root)
        .generation(context.now, context.work, CodeReads::Scan);
    let layer = generation.code.clone().expect("code was scanned");
    let code = CodeLinks::new(&generation.snapshot, layer);
    let findings = validation::check(&generation.snapshot, context.work);
    let mut facts = |_: &mut _| CodeFacts::new(&code, &generation.snapshot);
    let invariants = rules::invariants(&generation, Scope::Project, &mut facts, context.work);
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
    diagnostics.extend(invariants.diagnostics);
    let mut request = vec!["project validate"];
    request.extend(&paths);
    let mut keys = input_keys(&generation, &invariants.keys);
    keys.push(Some(code.key()));
    let key = result_key(&request, &keys, &diagnostics);
    let primary = findings.into_iter().filter(|finding| finding.primary);
    diagnostics.extend(primary.map(|finding| finding.diagnostic));
    diagnostics.extend(
        invariants
            .findings
            .into_iter()
            .map(|finding| finding.diagnostic),
    );
    diagnostics.extend(code.problems);
    respond(Report::new(diagnostics, key), &paths, args)
}

/// The problems of one item, named by its ID or MID, checked against the whole project. Code
/// is listed, and read only when an invariant's program uses code links (DES-CODE-READERS); the
/// problems of reading it are then the item's too.
fn item_validate(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let reference = args.argument(0);
    let root = context.root()?;
    let mut inputs = context.inputs(root);
    let now = context.now;
    let generation = inputs.generation(now, context.work, CodeReads::List);
    let snapshot = &generation.snapshot;
    let item = super::resolve_item(snapshot, reference)?;
    let findings = validation::check(snapshot, context.work);
    let mut diagnostics: Vec<Diagnostic> = findings
        .into_iter()
        .filter(|finding| finding.item == Some(item))
        .map(|finding| finding.diagnostic)
        .collect();
    // A configuration problem may drop an entry that would judge the item, and a program that
    // cannot be evaluated leaves the item's invariants unknown, so their problems are the
    // item's too.
    let mut scanned = None;
    let mut code_problems: Vec<Diagnostic> = Vec::new();
    let mut code = |work: &mut _| {
        let (layer, read) = inputs.scan_listed(&generation, now, work);
        // What changed the code links an invariant read: `unavailable-grammar`, a file that fell
        // back, and every problem that leaves code incomplete.
        code_problems = read;
        for problem in &layer.unread {
            if !code_problems.contains(problem) {
                code_problems.push(problem.clone());
            }
        }
        let links = CodeLinks::new(snapshot, layer);
        scanned = Some(links.key());
        CodeFacts::new(&links, snapshot)
    };
    let invariants = rules::invariants(&generation, Scope::Item, &mut code, context.work);
    let mut shared: Vec<Diagnostic> = generation
        .project
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.path == CONFIG_PATH)
        .cloned()
        .collect();
    shared.extend(invariants.diagnostics);
    shared.extend(code_problems);
    let mut keys = input_keys(&generation, &invariants.keys);
    keys.push(scanned);
    let key = result_key(&["item validate", reference], &keys, &shared);
    diagnostics.extend(shared);
    let rows = invariants.findings.into_iter();
    diagnostics.extend(
        rows.filter(|finding| finding.item == Some(item))
            .map(|finding| finding.diagnostic),
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
