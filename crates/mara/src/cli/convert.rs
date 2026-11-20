//! `project convert`: REQ-CONVERT-DIRECTIVE-CORPUS and DES-CONVERSION-MAPPING in
//! `docs/migration.mara.md`.

use std::fmt::Write as _;

use mara::convert::{self, REPORT_PATH};
use mara::project;
use mara_engine::config::{CONFIG_PATH, Root};
use mara_engine::json::{self, Object};

use super::{
    Args, Context, FORMAT_VERSION, Failure, Kind, Operation, Opt, REJECTED, Response, Run, SUCCESS,
};

pub const PROJECT_CONVERT: Operation = Operation {
    words: &["project", "convert"],
    summary: "Converts a 0.3.0 project, whose items are `:::mara` blocks, to this format, and \
              reports what it leaves out.",
    arguments: &[],
    options: &[STATUS],
    run: Run::Document(project_convert),
};

/// `--status <status>`: the status of each item that has none.
const STATUS: Opt = Opt {
    name: "status",
    value: "status",
    help: "The status of each item that has none.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |value| match convert::is_name(&convert::name_of(value)) {
        true => Ok(()),
        false => Err(format!(
            "`{value}` is no status: a status is lowercase letters and digits in parts joined by \
             `_` or `-`, or its label"
        )),
    },
};

/// The failure code of `project convert` on a project that is not a 0.3.0 project.
const NOT_LEGACY: &str = "not-a-legacy-project";

fn project_convert(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let start = context.cwd()?.to_owned();
    let (root, kind) = project::find(&start, context.project.as_deref())?;
    if kind == Root::Project {
        let message = format!(
            "{} is a project of this Mara ({CONFIG_PATH}), so there is nothing to convert",
            root.display()
        );
        return Err(Failure::new(NOT_LEGACY, message));
    }
    let outcome =
        convert::convert(&root, args.value(STATUS.name), context.work).map_err(|refusal| {
            Failure {
                code: refusal.code,
                message: refusal.message,
                diagnostics: refusal.diagnostics,
                status: if refusal.rejected {
                    REJECTED
                } else {
                    super::FAILURE
                },
            }
        })?;
    let records: Vec<String> = outcome
        .entries
        .iter()
        .map(|entry| {
            let mut record = String::new();
            entry.write_json(&mut record);
            record
        })
        .collect();
    let files: Vec<String> = outcome
        .files
        .iter()
        .map(|file| {
            let mut quoted = String::new();
            json::string(&mut quoted, file);
            quoted
        })
        .collect();
    let mut json = String::new();
    let mut object = Object::new(&mut json);
    object.number("format_version", FORMAT_VERSION);
    object.array("files", files.iter().map(String::as_str));
    object.array("entries", records.iter().map(String::as_str));
    object.end();
    let mut human = String::new();
    for entry in &outcome.entries {
        let _ = writeln!(human, "{}:{}: {}", entry.path, entry.line, entry.message);
    }
    let _ = writeln!(
        human,
        "converted: wrote {} files; {REPORT_PATH} holds the {} entries of the report",
        outcome.files.len(),
        outcome.entries.len()
    );
    Ok(Response {
        json,
        human,
        status: SUCCESS,
    })
}
