//! A request as a CLI call and as its MCP tool call, the files of a project, and the pairing of
//! the MIDs that two runs of a request generate (VER-CLI-MCP-PARITY in
//! `docs/hosts/verification.mara.md`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Map, Value, json};

/// Files and directories below a root: each file with its bytes, each directory with a
/// trailing `/` and no bytes.
pub type Files = BTreeMap<String, Vec<u8>>;

/// Every file and directory below `root`.
pub fn files(root: &Path) -> Files {
    fn walk(root: &Path, directory: &Path, out: &mut Files) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                out.insert(format!("{relative}/"), Vec::new());
                walk(root, &path, out);
            } else {
                out.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = Files::new();
    walk(root, root, &mut out);
    out
}

/// An operation as the CLI's usage lists it: its words, positional arguments and options.
#[derive(Debug)]
pub struct Usage {
    pub words: Vec<String>,
    pub arguments: Vec<String>,
    /// Each option's name, and whether it repeats.
    pub options: Vec<(String, bool)>,
}

/// The operations that bare `mara` lists.
pub fn usage() -> Vec<Usage> {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .current_dir(std::env::temp_dir())
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .stdin(Stdio::null())
        .output()
        .expect("start mara");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .expect("stdout is UTF-8")
        .lines()
        .filter_map(|line| line.strip_prefix("  mara "))
        .map(|line| {
            let mut usage = Usage {
                words: Vec::new(),
                arguments: Vec::new(),
                options: Vec::new(),
            };
            for part in line.split(' ') {
                if let Some(option) = part.strip_prefix("[--") {
                    usage.options.push((option.to_owned(), false));
                } else if part.ends_with("]...") {
                    usage.options.last_mut().expect("an option").1 = true;
                } else if part.ends_with(']') {
                    // An option's value.
                } else if let Some(argument) = part.strip_prefix('<') {
                    usage
                        .arguments
                        .push(argument.trim_end_matches('>').to_owned());
                } else {
                    usage.words.push(part.to_owned());
                }
            }
            usage
        })
        .collect()
}

/// The MCP form of the CLI request `args`: its tool and arguments, from the usage of its
/// operation and the types of the tool's parameters.
/// A file that `--input <name>=@<file>` names is read from `cwd`, where the CLI runs.
pub fn mcp_request(
    usages: &[Usage],
    tools: &[Value],
    args: &[&str],
    cwd: &Path,
) -> (String, Value) {
    let usage = usages
        .iter()
        .filter(|usage| {
            args.starts_with(&usage.words.iter().map(String::as_str).collect::<Vec<_>>())
        })
        .max_by_key(|usage| usage.words.len())
        .unwrap_or_else(|| panic!("no operation for {args:?}"));
    let name = usage.words.join("_");
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == name.as_str())
        .unwrap_or_else(|| panic!("no tool {name}"));
    let properties = &tool["inputSchema"]["properties"];
    let mut arguments = Map::new();
    let mut positional = usage.arguments.iter();
    let mut rest = args[usage.words.len()..].iter();
    while let Some(arg) = rest.next() {
        let (key, value) = match arg.strip_prefix("--") {
            Some(option) => match option.split_once('=') {
                Some((key, value)) => (key.to_owned(), (*value).to_owned()),
                None => (
                    option.to_owned(),
                    (*rest.next().expect("a value")).to_owned(),
                ),
            },
            None => (
                positional.next().expect("a declared argument").clone(),
                (*arg).to_owned(),
            ),
        };
        let schema = &properties[&key];
        // `--input <name>=<value>` or `<name>=@<file>`: MCP passes the value, or the file's
        // rows, under the input's name (DES-REQUEST-INPUTS).
        if schema["type"] == "object" {
            let (input, given) = value.split_once('=').expect("`<name>=<value>`");
            let given = match given.strip_prefix('@') {
                Some(file) => {
                    let path = cwd.join(file);
                    let text = fs::read(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
                    serde_json::from_slice(&text).expect("a JSON file")
                }
                None => json!(given),
            };
            let entry = arguments.entry(key).or_insert_with(|| json!({}));
            entry[input] = given;
            continue;
        }
        let kind = match schema["type"].as_str() {
            Some("array") => schema["items"]["type"].as_str(),
            kind => kind,
        };
        let value = match kind {
            Some("integer") => json!(value.parse::<i64>().expect("a whole number")),
            Some("string") => json!(value),
            _ => panic!("no parameter {key} of {name}: {tool}"),
        };
        if schema["type"] == "array" {
            arguments
                .entry(key)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .unwrap()
                .push(value);
        } else {
            arguments.insert(key, value);
        }
    }
    (name, Value::Object(arguments))
}

/// The MID of each anchor line, by path and line.
pub fn anchors(files: &Files) -> BTreeMap<(String, usize), String> {
    let mut out = BTreeMap::new();
    for (path, bytes) in files {
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let mid = line
                .trim()
                .strip_prefix("<a id=\"")
                .and_then(|rest| rest.strip_suffix("\"></a>"));
            if let Some(mid) = mid.filter(|mid| mid.len() == 26) {
                out.insert((path.clone(), index + 1), mid.to_owned());
            }
        }
    }
    out
}

/// The `text` and `files` of a run of a request on the files `before`, with each MID that it
/// generated replaced by the one that the run which left `expected` generated at the same path
/// and line. Both runs must have generated MIDs at the same places, and the places must pair
/// them one to one: each MID of the run with one MID of `expected`, and no two with the same. A
/// MID that both runs wrote, such as one a conversion keeps, pairs with itself. `request` names
/// the request in a failure.
#[track_caller]
pub fn pair_mids(
    request: &[&str],
    before: &Files,
    expected: &Files,
    text: &str,
    files: Files,
) -> (String, Files) {
    let known: BTreeSet<String> = anchors(before).into_values().collect();
    let generated = |files: &Files| -> BTreeMap<(String, usize), String> {
        (anchors(files).into_iter())
            .filter(|(_, mid)| !known.contains(mid))
            .collect()
    };
    let (expected_mids, mids) = (generated(expected), generated(&files));
    assert_eq!(
        expected_mids.keys().collect::<Vec<_>>(),
        mids.keys().collect::<Vec<_>>(),
        "{request:?}: MIDs generated at other places"
    );
    let mut pairs: BTreeMap<&str, &str> = BTreeMap::new();
    for (place, mid) in &mids {
        let paired = expected_mids[place].as_str();
        let earlier = *pairs.entry(mid.as_str()).or_insert(paired);
        assert_eq!(earlier, paired, "{request:?}: {mid} pairs with two MIDs");
    }
    let targets: BTreeSet<&str> = pairs.values().copied().collect();
    assert_eq!(
        targets.len(),
        pairs.len(),
        "{request:?}: two MIDs pair with one: {pairs:?}"
    );
    // Through placeholders that no MID holds, so that a replacement never meets the result of
    // another.
    let replace = |text: &str| -> String {
        let mut text = text.to_owned();
        for (index, mid) in pairs.keys().enumerate() {
            text = text.replace(mid, &format!("\u{1}{index}\u{1}"));
        }
        for (index, paired) in pairs.values().enumerate() {
            text = text.replace(&format!("\u{1}{index}\u{1}"), paired);
        }
        text
    };
    let mut files = files;
    for bytes in files.values_mut() {
        if let Ok(contents) = std::str::from_utf8(bytes) {
            *bytes = replace(contents).into_bytes();
        }
    }
    (replace(text), files)
}

/// The text of a `tools/call` response and whether it is an error. DES-MCP-SERVER gives a
/// result one text content block and a boolean `isError`, and no structured content: a response
/// with anything else fails.
#[track_caller]
pub fn tool_result(response: &Value) -> (String, bool) {
    assert_eq!(response["jsonrpc"], "2.0", "{response}");
    assert!(response.get("error").is_none(), "{response}");
    let result = response["result"].as_object();
    let result = result.unwrap_or_else(|| panic!("no result: {response}"));
    let keys: BTreeSet<&str> = result.keys().map(String::as_str).collect();
    assert_eq!(keys, BTreeSet::from(["content", "isError"]), "{response}");
    let content = result["content"].as_array();
    let content = content.unwrap_or_else(|| panic!("no content: {response}"));
    assert_eq!(content.len(), 1, "{response}");
    let block = content[0].as_object();
    let block = block.unwrap_or_else(|| panic!("no block: {response}"));
    let keys: BTreeSet<&str> = block.keys().map(String::as_str).collect();
    assert_eq!(keys, BTreeSet::from(["text", "type"]), "{response}");
    assert_eq!(block["type"], "text", "{response}");
    let text = block["text"].as_str();
    let text = text.unwrap_or_else(|| panic!("no text: {response}"));
    let is_error = result["isError"].as_bool();
    let is_error = is_error.unwrap_or_else(|| panic!("no boolean isError: {response}"));
    (text.to_owned(), is_error)
}
