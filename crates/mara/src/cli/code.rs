//! `code add`: REQ-ONLY-CODE-ADD-DOWNLOADS and DES-GRAMMAR-ASSETS in `docs/code/index.mara.md`.
//!
//! It fetches a grammar's `.wasm` and `tags.scm` from the built-in catalog or from a given
//! repository, checks them against the checksum it knows, stores them in the per-user cache and
//! pins them in the configuration through the write procedure. The client runs it itself: no
//! daemon and no MCP tool serves it, and it reads no document or code file.

mod catalog;
mod fetch;
mod pin;

use std::fs;
use std::io::Write as _;
use std::path::Path;

use mara::grammar::{self, TAGS_FILE, WASM_FILE};
use mara::write::{self, Basis, FileWrite, WriteLock};
use mara_engine::config::{
    CHECKSUM_PREFIX, CONFIG_PATH, GrammarPin, check_repository, is_grammar_version,
    is_language_name, parse_config, repository,
};
use mara_engine::diagnostic::Severity;
use mara_engine::hash::Hash;
use mara_engine::json::Object;
use tree_sitter::wasmtime;

use super::{
    Args, Argument, Context, FORMAT_VERSION, Failure, Kind, Operation, Opt, Response, Run, SUCCESS,
};

pub const ADD: Operation = Operation {
    words: &["code", "add"],
    summary: "Fetches a language's tree-sitter grammar from the built-in catalog or a given \
              repository, checks its checksum, caches it for this user and pins it in the \
              configuration.",
    arguments: &[Argument {
        name: "language",
        help: "The code language, as `[code.languages.<language>]` names it.",
    }],
    options: &[REPOSITORY, VERSION],
    run: Run::Command(code_add),
};

/// `--repository <url>`: a grammar repository instead of the catalog, trusted on first use.
const REPOSITORY: Opt = Opt {
    name: "repository",
    value: "url",
    help: "The repository to fetch from instead of the catalog: `<url>/<language>/<version>/` \
           holds `grammar.wasm` and `tags.scm`. An `https://` URL, or `http://` to a loopback \
           address. Needs `--version`.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: check_repository,
};

/// `--version <version>`: the release of a given repository.
const VERSION: Opt = Opt {
    name: "version",
    value: "version",
    help: "The release to fetch from `--repository`.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |value| match is_grammar_version(value) {
        true => Ok(()),
        false => Err(format!(
            "`{value}` is no grammar version: letters, digits, `.`, `-`, `_` and `+`, not \
             starting with `.`"
        )),
    },
};

/// No grammar source: neither the catalog nor a repository names the grammar to fetch, or no
/// one declares its language.
const UNKNOWN_GRAMMAR: &str = "unknown-grammar";
/// A file could not be fetched, or is no WebAssembly module.
const FETCH_FAILED: &str = "fetch-failed";
/// The fetched files do not have the checksum that the catalog or the pin gives.
const CHECKSUM_MISMATCH: &str = "checksum-mismatch";

/// Where a grammar's files come from.
struct Source {
    wasm: String,
    tags: String,
    version: String,
    /// The repository to pin: `None` for the catalog.
    repository: Option<String>,
    /// The checksum the files must have, unless trusted on first use.
    expected: Option<Hash>,
    /// For a message: where the files come from.
    shown: String,
}

impl Source {
    fn repository(url: &str, language: &str, version: &str, expected: Option<Hash>) -> Source {
        let base = format!("{}/{language}/{version}", url.trim_end_matches('/'));
        Source {
            wasm: format!("{base}/{WASM_FILE}"),
            tags: format!("{base}/{TAGS_FILE}"),
            version: version.to_owned(),
            repository: Some(url.to_owned()),
            expected,
            shown: url.to_owned(),
        }
    }

    fn catalog(entry: &catalog::Entry) -> Source {
        Source {
            wasm: entry.wasm.clone(),
            tags: entry.tags.clone(),
            version: entry.version.clone(),
            repository: None,
            expected: Some(entry.checksum),
            shown: "the built-in catalog".to_owned(),
        }
    }
}

fn code_add(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let language = args.argument(0);
    if !is_language_name(language) {
        return Err(Failure::rejected(format!(
            "`{language}` is no language name: lowercase ASCII letters and digits in parts \
             joined by single `_` or `-`"
        )));
    }
    let given = match (args.value(REPOSITORY.name), args.value(VERSION.name)) {
        (Some(url), Some(version)) => {
            let url = repository(url)
                .expect("checked before the operation ran")
                .url;
            Some((url, version))
        }
        (None, None) => None,
        (Some(_), None) => {
            return Err(Failure::rejected(
                "`--repository` needs `--version`: the release to fetch".to_owned(),
            ));
        }
        (None, Some(_)) => {
            return Err(Failure::rejected(
                "`--version` needs `--repository`: the catalog fixes each grammar's version"
                    .to_owned(),
            ));
        }
    };
    let root = context.root()?;
    let lock = WriteLock::take(&root)?;
    let (bytes, basis) = Basis::read(&lock, CONFIG_PATH, context.work)?;
    let Some(bytes) = bytes else {
        let message = format!("{CONFIG_PATH} is gone");
        return Err(Failure::new("changed-file", message));
    };
    let (config, problems) = parse_config(&bytes);
    let errors: Vec<_> = (problems.into_iter())
        .filter(|problem| problem.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(Failure {
            diagnostics: errors,
            ..Failure::new(
                "invalid-project",
                format!("{CONFIG_PATH} has errors: fix them before pinning a grammar"),
            )
        });
    }
    let catalog = catalog::load().map_err(|why| Failure::new("io-error", why))?;
    let entry = catalog::find(&catalog, language);
    let declared = config.code.iter().find(|known| known.name == language);
    let pinned = declared.and_then(|known| known.grammar.as_ref());
    let given = given
        .as_ref()
        .map(|(url, version)| (url.as_str(), *version));
    let source = source(language, given, pinned, entry, declared.is_some())?;
    let declaration = match (declared, entry) {
        (Some(_), _) => None,
        (None, Some(entry)) => Some(pin::Declaration {
            include: (entry.extensions.iter())
                .map(|extension| format!("**/*.{extension}"))
                .collect(),
            comment_prefixes: &entry.comment_prefixes,
        }),
        (None, None) => {
            return Err(Failure::new(
                UNKNOWN_GRAMMAR,
                format!(
                    "the configuration does not declare `{language}`, and the built-in catalog \
                     has no grammar for it to declare it with: add `[code.languages.{language}]` \
                     with its `include` and `comment_prefixes` to {CONFIG_PATH}, then run this \
                     again"
                ),
            ));
        }
    };
    // Every check of the configuration comes before a connection: the edit is planned with the
    // checksum the files must have, or a stand-in when they are trusted on first use.
    let unchanged = |checksum: Hash| {
        pinned.is_some_and(|known| {
            (&known.version, known.checksum, &known.repository)
                == (&source.version, checksum, &source.repository)
        })
    };
    let text = String::from_utf8(bytes)
        .map_err(|_| Failure::new("invalid-utf8", format!("{CONFIG_PATH} is not UTF-8")))?;
    let plan = |checksum: Hash| {
        let pin = GrammarPin {
            version: source.version.clone(),
            checksum,
            repository: source.repository.clone(),
            line: 0,
        };
        pin::pinned(&text, language, &pin, declaration.clone())
    };
    let planned = source.expected.unwrap_or_else(|| Hash::of_bytes(b""));
    if !unchanged(planned) {
        plan(planned).map_err(|why| {
            Failure::new(
                "unreadable-edit",
                format!("{why}; nothing was written or fetched"),
            )
        })?;
    }
    let cache = grammar::cache_root().ok_or_else(|| {
        let message = "the environment names no home directory for the grammar cache: set HOME";
        Failure::new("io-error", message.to_owned())
    })?;
    let cached = source.expected.and_then(|checksum| {
        let pin = GrammarPin {
            version: source.version.clone(),
            checksum,
            repository: None,
            line: 0,
        };
        let directory = grammar::grammar_dir(&cache, language, &pin);
        let wasm = fs::read(directory.join(WASM_FILE)).ok()?;
        let tags = fs::read(directory.join(TAGS_FILE)).ok()?;
        (grammar::checksum(&wasm, &tags) == checksum).then_some(checksum)
    });
    let fetched = cached.is_none();
    let checksum = match cached {
        Some(checksum) => checksum,
        None => fetch_and_store(&cache, language, &source)?,
    };
    let pin = GrammarPin {
        version: source.version.clone(),
        checksum,
        repository: source.repository.clone(),
        line: 0,
    };
    // A refusal from here on leaves the project's files as they were; the cache keeps the files
    // that matched their checksum.
    let kept = |mut failure: Failure| {
        if fetched {
            let directory = grammar::grammar_dir(&cache, language, &pin);
            let note = format!(
                "; the per-user grammar cache keeps the verified files in {}",
                directory.display()
            );
            let messages = (failure.diagnostics.iter_mut().map(|d| &mut d.message))
                .chain([&mut failure.message]);
            for message in messages {
                *message = message.replace("nothing was written", "no project file was written");
                message.push_str(&note);
            }
        }
        failure
    };
    let changed = !unchanged(checksum);
    if changed {
        let contents = plan(checksum).map_err(|why| {
            kept(Failure::new(
                "unreadable-edit",
                format!("{why}; {CONFIG_PATH} was not written"),
            ))
        })?;
        let edit = FileWrite {
            path: CONFIG_PATH.to_owned(),
            basis,
            contents: contents.into_bytes(),
        };
        write::apply(&lock, &[edit], context.work).map_err(|error| kept(error.into()))?;
    }
    Ok(respond(language, &pin, &source.shown, fetched, changed))
}

/// Where the grammar comes from and the checksum it must have: the repository given, the pin's
/// repository, or the catalog. A given repository and version that the pin holds are checked
/// against it; others are trusted on first use and pinned in its place.
fn source(
    language: &str,
    given: Option<(&str, &str)>,
    pinned: Option<&GrammarPin>,
    entry: Option<&catalog::Entry>,
    declared: bool,
) -> Result<Source, Failure> {
    match (given, pinned) {
        (Some((url, version)), Some(pin))
            if pin.repository.as_deref() == Some(url) && pin.version == version =>
        {
            Ok(Source::repository(
                url,
                language,
                version,
                Some(pin.checksum),
            ))
        }
        (Some((url, version)), _) => Ok(Source::repository(url, language, version, None)),
        (None, Some(pin)) => match (&pin.repository, entry) {
            (Some(url), _) => Ok(Source::repository(
                url,
                language,
                &pin.version,
                Some(pin.checksum),
            )),
            (None, Some(entry))
                if entry.version == pin.version && entry.checksum == pin.checksum =>
            {
                Ok(Source::catalog(entry))
            }
            (None, entry) => {
                let has = match entry {
                    Some(entry) => format!("has {language} {}", entry.version),
                    None => format!("has no grammar for {language}"),
                };
                Err(Failure::new(
                    UNKNOWN_GRAMMAR,
                    format!(
                        "{CONFIG_PATH} pins the {language} grammar {} from the catalog, and this \
                         Mara's catalog {has}: give the repository that serves the pinned \
                         release with `--repository <url> --version {}`, or remove the pin to \
                         pin the catalog's",
                        pin.version, pin.version
                    ),
                ))
            }
        },
        (None, None) => match entry {
            Some(entry) => Ok(Source::catalog(entry)),
            None => {
                let declare = match declared {
                    true => String::new(),
                    false => format!(
                        ", and declare `[code.languages.{language}]` with its `include` and \
                         `comment_prefixes` in {CONFIG_PATH}"
                    ),
                };
                Err(Failure::new(
                    UNKNOWN_GRAMMAR,
                    format!(
                        "the built-in catalog has no grammar for `{language}`: give a repository \
                         with `--repository <url> --version <version>`{declare}"
                    ),
                ))
            }
        },
    }
}

/// Fetches the grammar's files from `source`, checks their checksum and stores them in the cache
/// below `cache`, each under a temporary name in its directory and then renamed: nothing is
/// written before the checksum is checked, and nothing under its final name before it is whole.
fn fetch_and_store(cache: &Path, language: &str, source: &Source) -> Result<Hash, Failure> {
    let get = |url: &str| {
        fetch::get(url).map_err(|why| {
            Failure::new(
                FETCH_FAILED,
                format!(
                    "cannot fetch {url}: {why}; nothing was written. Check the URL and the \
                     network, and run this again"
                ),
            )
        })
    };
    let wasm = get(&source.wasm)?;
    // Validated, not compiled or run: a grammar runs only when an operation loads it.
    let engine = wasmtime::Engine::default();
    if let Err(error) = wasmtime::Module::validate(&engine, &wasm) {
        return Err(Failure::new(
            FETCH_FAILED,
            format!(
                "{} is no valid WebAssembly module, so it is no grammar ({error:#}); nothing was \
                 written",
                source.wasm
            ),
        ));
    }
    let tags = get(&source.tags)?;
    let checksum = grammar::checksum(&wasm, &tags);
    if let Some(expected) = source.expected
        && expected != checksum
    {
        return Err(Failure::new(
            CHECKSUM_MISMATCH,
            format!(
                "the {language} grammar {} from {} has the checksum {CHECKSUM_PREFIX}{checksum}, \
                 not {CHECKSUM_PREFIX}{expected} as {}; Mara runs no grammar it cannot verify, \
                 so nothing was written",
                source.version,
                source.shown,
                match source.repository {
                    Some(_) => "the configuration pins",
                    None => "the built-in catalog gives",
                }
            ),
        ));
    }
    let pin = GrammarPin {
        version: source.version.clone(),
        checksum,
        repository: None,
        line: 0,
    };
    let directory = grammar::grammar_dir(cache, language, &pin);
    let failed = |error: std::io::Error| {
        Failure::new(
            "io-error",
            format!(
                "cannot write the grammar cache {}: {error}",
                directory.display()
            ),
        )
    };
    fs::create_dir_all(&directory).map_err(failed)?;
    let staged = [(WASM_FILE, &wasm), (TAGS_FILE, &tags)].map(|(name, contents)| {
        let mut file = tempfile::Builder::new()
            .prefix(".mara-tmp-")
            .tempfile_in(&directory)?;
        file.write_all(contents)?;
        file.as_file().sync_all()?;
        Ok::<_, std::io::Error>((name, file))
    });
    let mut files = Vec::new();
    for staged in staged {
        files.push(staged.map_err(failed)?);
    }
    for (name, file) in files {
        file.persist(directory.join(name))
            .map_err(|error| failed(error.error))?;
    }
    Ok(checksum)
}

/// `{"format_version":1,"language","version","checksum","repository","fetched","pinned"}`, and a
/// line of what happened.
fn respond(language: &str, pin: &GrammarPin, shown: &str, fetched: bool, pinned: bool) -> Response {
    let checksum = format!("{CHECKSUM_PREFIX}{}", pin.checksum);
    let mut json = String::new();
    let mut object = Object::new(&mut json);
    object.number("format_version", FORMAT_VERSION);
    object.string("language", language);
    object.string("version", &pin.version);
    object.string("checksum", &checksum);
    object.optional_string("repository", pin.repository.as_deref());
    object.bool("fetched", fetched);
    object.bool("pinned", pinned);
    object.end();
    let got = match fetched {
        true => format!("fetched from {shown}"),
        false => "already in the grammar cache".to_owned(),
    };
    let pinned = match pinned {
        true => format!("pinned in {CONFIG_PATH}"),
        false => format!("already pinned in {CONFIG_PATH}"),
    };
    Response {
        json,
        human: format!(
            "the {language} grammar {} ({checksum}): {got}, {pinned}\n",
            pin.version
        ),
        status: SUCCESS,
    }
}
