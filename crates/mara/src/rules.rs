//! The files a program run reads: the fact relations of a snapshot and the supplied inputs, one
//! JSON file per relation (DES-FACT-BRIDGE in `docs/rules-facts.mara.md`), and the inputs a
//! command line supplies (DES-REQUEST-INPUTS in `docs/rules.mara.md`).

use std::fs;
use std::io;
use std::path::Path;

use mara_engine::facts::{Facts, InputError, Inputs, MARA_DL, Supplied, file_name};

/// Writes `mara.dl`, every fact relation of `facts` and every relation of `inputs` into `dir`,
/// the evaluation's private directory, each relation to the file [`file_name`] names. The JSON
/// text was rendered once with the snapshot's facts and is written as is.
pub fn write_facts(dir: &Path, facts: &Facts, inputs: &Inputs) -> io::Result<()> {
    fs::write(dir.join("mara.dl"), MARA_DL)?;
    let relations = facts.relations.iter().chain(&inputs.relations);
    for relation in relations.chain([&inputs.source_complete]) {
        fs::write(dir.join(file_name(&relation.name)), &relation.json)?;
    }
    Ok(())
}

/// The input that one `--input` argument supplies: `<name>=<value>` for one value inline, or
/// `<name>=@<file>` for a JSON file of rows, relative to `base`, the directory the command runs
/// in. A file that cannot be read or is not JSON refuses the request.
pub fn supplied_input(argument: &str, base: &Path) -> Result<(String, Supplied), InputError> {
    let Some((name, value)) = argument
        .split_once('=')
        .filter(|(name, _)| !name.is_empty())
    else {
        return Err(InputError::Invalid(format!(
            "`--input {argument}` names no input: write `<name>=<value>` or `<name>=@<file>`"
        )));
    };
    let Some(file) = value.strip_prefix('@') else {
        return Ok((name.to_owned(), Supplied::Inline(value.to_owned())));
    };
    let refuse = |problem: String| InputError::Invalid(format!("input `{name}`: {file} {problem}"));
    let bytes =
        fs::read(base.join(file)).map_err(|error| refuse(format!("cannot be read: {error}")))?;
    let rows =
        serde_json::from_slice(&bytes).map_err(|error| refuse(format!("is not JSON: {error}")))?;
    Ok((name.to_owned(), Supplied::Rows(rows)))
}
