//! VER-CODE-ADD: `code add` through the real CLI, with `HOME` and the per-user cache inside the
//! test's temporary directory and the fixture grammars served by the test on a loopback port.
//! Only the ignored test reaches another machine.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::grammar_server::{GrammarServer, fixture};
use crate::{Project, Tree, paused, unchanged};

const MARKED: &str = "// @mara implements REQ-LOGIN\npub fn login() {}\n";

/// The cache directory of the fixture grammar of `language` below `home`.
fn cache_dir(home: &Path, language: &str) -> PathBuf {
    let (wasm, tags, version) = fixture(language);
    let checksum = mara::grammar::checksum(&wasm, &tags);
    home.join(".cache/mara/grammars")
        .join(language)
        .join(version)
        .join(checksum.to_string())
}

/// Every file below `home`'s grammar cache.
fn cached(home: &Path) -> Vec<PathBuf> {
    fn walk(directory: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            match path.is_dir() {
                true => walk(&path, out),
                false => out.push(path),
            }
        }
    }
    let mut out = Vec::new();
    walk(&home.join(".cache"), &mut out);
    out.sort();
    out
}

/// The `implemented_by` code endpoints of `node`: path, line and declaration.
fn endpoints(project: &Project, node: &str) -> Vec<(String, u64, Value)> {
    let args = ["relation", "list", node, "--relation", "implemented_by"];
    let result = project.json(&args, 0);
    (result["connections"].as_array().unwrap().iter())
        .map(|connection| {
            let code = &connection["neighbour"];
            let path = code["path"].as_str().unwrap().to_owned();
            (
                path,
                code["line"].as_u64().unwrap(),
                code["declaration"].clone(),
            )
        })
        .collect()
}

/// The tree without the configuration.
fn without_config(tree: &Tree) -> Tree {
    let mut tree = tree.clone();
    tree.remove(".mara/config.toml");
    tree
}

/// The `error.code` of a failed JSON result.
fn failure(project: &Project, args: &[&str], env: &[(&str, &Path)], status: i32) -> String {
    let mut all = vec!["--format", "json"];
    all.extend(args);
    let output = project.mara(&all, env).output().expect("run mara");
    assert_eq!(
        output.status.code(),
        Some(status),
        "mara {args:?}: {output:?}"
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("a JSON result");
    result["error"]["code"].as_str().unwrap().to_owned()
}

#[test]
fn adds_a_grammar_from_a_given_repository_and_holds_it_to_its_pin() {
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm.clone(), tags.clone());
    let url = server.url();
    let mut project = Project::new();
    let home = project.home();
    project.write("src/lib.rs", MARKED);
    let before = project.tree();

    let add = [
        "code",
        "add",
        "rust",
        "--repository",
        &url,
        "--version",
        version,
    ];
    let result = project.json(&add, 0);
    let checksum = format!("blake3:{}", mara::grammar::checksum(&wasm, &tags));
    assert_eq!(result["checksum"], checksum.as_str(), "{result}");
    assert_eq!(result["repository"], url.as_str());
    assert_eq!(
        (&result["fetched"], &result["pinned"]),
        (&Value::Bool(true), &Value::Bool(true))
    );
    assert_eq!(server.connections(), 2, "one connection per file");
    let directory = cache_dir(&home, "rust");
    assert_eq!(fs::read(directory.join("grammar.wasm")).unwrap(), wasm);
    assert_eq!(fs::read(directory.join("tags.scm")).unwrap(), tags);
    assert_eq!(cached(&home).len(), 2, "no temporary file is left");
    let config = project.read(".mara/config.toml");
    let pin = format!(
        "[code.languages.rust]\ninclude = [\"src/**/*.rs\"]\ncomment_prefixes = [\"//\", \"///\", \"*\"]\n\
         grammar = {{ version = \"{version}\", checksum = \"{checksum}\", repository = \"{url}\" }}\n"
    );
    assert!(config.contains(&pin), "{config}");
    let after = project.tree();
    unchanged(
        &without_config(&before),
        &without_config(&after),
        "code add",
    );
    assert_eq!(
        endpoints(&project, "REQ-LOGIN"),
        [("src/lib.rs".to_owned(), 2, Value::from("login"))],
        "the marker has a declaration endpoint"
    );

    // Again: cached and pinned, nothing fetched or written.
    let again = project.json(&["code", "add", "rust"], 0);
    assert_eq!(
        (&again["fetched"], &again["pinned"]),
        (&Value::Bool(false), &Value::Bool(false))
    );
    assert_eq!(server.connections(), 2);
    unchanged(&after, &project.tree(), "code add of a cached pin");

    // A copy of the project with an empty cache, the server now serving other bytes.
    let mut copy = Project::new();
    copy.write(".mara/config.toml", &config);
    copy.write("src/lib.rs", MARKED);
    let copy_home = copy.home();
    let mut changed = tags.clone();
    changed.extend_from_slice(b"\n; changed\n");
    server.serve_grammar("rust", version, wasm.clone(), changed);
    let copied = copy.tree();
    let output = copy.mara(&["code", "add", "rust"], &[]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[checksum-mismatch]") && stderr.contains(&checksum),
        "{stderr}"
    );
    assert_eq!(cached(&copy_home), Vec::<PathBuf>::new(), "nothing cached");
    unchanged(&copied, &copy.tree(), "a refused fetch");

    // A cached file changed after `code add`: every read falls back, and nothing connects.
    let connections = server.connections();
    fs::write(
        directory.join("tags.scm"),
        [tags.as_slice(), b"\n"].concat(),
    )
    .unwrap();
    let line = config
        .lines()
        .position(|line| line.starts_with("grammar = "))
        .unwrap() as u64
        + 1;
    let problems = project.problems();
    assert!(
        problems.contains(&(
            "unavailable-grammar".to_owned(),
            ".mara/config.toml".to_owned(),
            line
        )),
        "{problems:?}"
    );
    assert_eq!(project.code_links("REQ-LOGIN"), ["src/lib.rs:1"]);
    assert_eq!(server.connections(), connections, "a read fetches nothing");
}

#[test]
fn rejects_other_repositories_before_a_connection() {
    let server = GrammarServer::start();
    let port = server.url().rsplit(':').next().unwrap().to_owned();
    let mut project = Project::new();
    let home = project.home();
    let before = project.tree();
    for url in [
        "http://example.com/grammars".to_owned(),
        "https://example.org:abc/grammars".to_owned(),
        "https://example.org:65536/grammars".to_owned(),
        format!("http://localhost:{port}"),
        format!("ftp://127.0.0.1:{port}"),
    ] {
        let args = [
            "code",
            "add",
            "rust",
            "--repository",
            &url,
            "--version",
            "0.24.2",
        ];
        assert_eq!(
            failure(&project, &args, &[], 2),
            "invalid-argument",
            "{url}"
        );
    }
    let url = server.url();
    for args in [
        &["code", "add", "rust", "--repository", &url][..],
        &["code", "add", "rust", "--version", "0.24.2"],
        &["code", "add", "Rust"],
    ] {
        assert_eq!(
            failure(&project, args, &[], 2),
            "invalid-argument",
            "{args:?}"
        );
    }
    assert_eq!(server.connections(), 0);
    assert_eq!(cached(&home), Vec::<PathBuf>::new());
    unchanged(&before, &project.tree(), "rejected arguments");
}

/// A catalog of the server's URLs for `language`, with `checksum`, in a file of the test.
fn catalog(server: &GrammarServer, language: &str, checksum: &str) -> tempfile::NamedTempFile {
    let (_, _, version) = fixture(language);
    let base = format!("{}/{language}/{version}", server.url());
    let text = format!(
        "[languages.{language}]\nversion = \"{version}\"\nwasm = \"{base}/grammar.wasm\"\n\
         tags = \"{base}/tags.scm\"\nchecksum = \"{checksum}\"\nextensions = [\"rs\"]\n\
         comment_prefixes = [\"//\", \"///\"]\n"
    );
    let mut file = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut file, text.as_bytes()).unwrap();
    file
}

#[test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "needs the catalog hook of debug builds"
)]
fn adds_from_the_catalog_and_declares_a_language_the_configuration_lacks() {
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm.clone(), tags.clone());
    let checksum = format!("blake3:{}", mara::grammar::checksum(&wasm, &tags));
    let mut project = Project::bare();
    project.run(&["project", "init"], 0);
    let home = project.home();
    let before = project.tree();

    // The shipped catalog is empty: a language without a repository is refused.
    assert_eq!(
        failure(&project, &["code", "add", "rust"], &[], 1),
        "unknown-grammar"
    );
    unchanged(&before, &project.tree(), "a language the catalog lacks");

    let wrong = catalog(&server, "rust", &format!("blake3:{}", "ab".repeat(32)));
    let env = [("MARA_TEST_GRAMMAR_CATALOG", wrong.path())];
    assert_eq!(
        failure(&project, &["code", "add", "rust"], &env, 1),
        "checksum-mismatch"
    );
    assert_eq!(cached(&home), Vec::<PathBuf>::new());
    unchanged(&before, &project.tree(), "a catalog checksum mismatch");

    let file = catalog(&server, "rust", &checksum);
    let env = [("MARA_TEST_GRAMMAR_CATALOG", file.path())];
    assert_eq!(
        failure(&project, &["code", "add", "python"], &env, 1),
        "unknown-grammar"
    );
    let output = project
        .mara(&["--format", "json", "code", "add", "rust"], &env)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["repository"], Value::Null, "{result}");
    let config = project.read(".mara/config.toml");
    let declared = format!(
        "\n[code]\nformat_version = 1\n\n[code.languages.rust]\ninclude = [\"**/*.rs\"]\n\
         comment_prefixes = [\"//\", \"///\"]\ngrammar = {{ version = \"{version}\", checksum = \
         \"{checksum}\" }}\n"
    );
    assert!(config.ends_with(&declared), "{config}");
    assert!(config.starts_with(&String::from_utf8(before[".mara/config.toml"].0.clone()).unwrap()));
    project.write("src/lib.rs", MARKED);
    project.write(
        "docs/req.mara.md",
        format!(
            "# Requirements\n\n{}",
            crate::item(1, "Requirement", "REQ-LOGIN", "", "Users log in.")
        ),
    );
    assert_eq!(
        endpoints(&project, "REQ-LOGIN"),
        [("src/lib.rs".to_owned(), 2, Value::from("login"))]
    );
}

#[test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "needs the pause points of debug builds"
)]
fn refuses_a_configuration_changed_at_the_pause_before_step_2() {
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm, tags);
    let url = server.url();
    let mut project = Project::new();
    let home = project.home();
    let theirs = format!(
        "{}\n# changed by another program\n",
        project.read(".mara/config.toml")
    );
    let args = [
        "code",
        "add",
        "rust",
        "--repository",
        &url,
        "--version",
        version,
    ];
    let (output, pauses) = paused(&project, &args, |held| {
        if held.is_empty() {
            project.write(".mara/config.toml", &theirs);
        }
    });
    assert_eq!(pauses.first().map(String::as_str), Some(""), "{pauses:?}");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[changed-file]"), "{stderr}");
    assert_eq!(project.read(".mara/config.toml"), theirs);
    // The project's files are as the other program left them; the per-user cache keeps the
    // verified files, which the message names.
    let directory = cache_dir(&home, "rust");
    assert!(
        stderr.contains(&directory.display().to_string()),
        "{stderr}"
    );
    assert!(!stderr.contains("nothing was written"), "{stderr}");
    let (wasm, tags, _) = fixture("rust");
    assert_eq!(fs::read(directory.join("grammar.wasm")).unwrap(), wasm);
    assert_eq!(fs::read(directory.join("tags.scm")).unwrap(), tags);
    assert_eq!(cached(&home).len(), 2);
}

#[test]
fn holds_every_spelling_of_the_pinned_repository_to_its_pin() {
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm.clone(), tags.clone());
    let url = server.url();
    let mut project = Project::new();
    let home = project.home();
    project.json(
        &[
            "code",
            "add",
            "rust",
            "--repository",
            &url,
            "--version",
            version,
        ],
        0,
    );
    fs::remove_dir_all(home.join(".cache")).unwrap();
    server.serve_grammar(
        "rust",
        version,
        wasm,
        [tags.as_slice(), b"\n; changed\n"].concat(),
    );
    let before = project.tree();
    let upper = url.replacen("http://", "HTTP://", 1);
    for spelled in [
        format!("{url}/"),
        format!("{url}//"),
        upper.clone(),
        format!("{upper}/"),
    ] {
        let args = [
            "code",
            "add",
            "rust",
            "--repository",
            &spelled,
            "--version",
            version,
        ];
        assert_eq!(
            failure(&project, &args, &[], 1),
            "checksum-mismatch",
            "{spelled}"
        );
        assert_eq!(cached(&home), Vec::<PathBuf>::new(), "{spelled}");
        unchanged(&before, &project.tree(), &spelled);
    }
}

#[test]
fn refuses_before_a_connection_a_pin_it_cannot_write_in_place() {
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm, tags);
    let url = server.url();
    let mut project = Project::bare();
    project.run(&["project", "init"], 0);
    let home = project.home();
    let config = project.read(".mara/config.toml");
    project.write(
        ".mara/config.toml",
        format!(
            "{config}\n[code]\nformat_version = 1\nlanguages = {{ rust = {{ include = \
             [\"src/**/*.rs\"], comment_prefixes = [\"//\"] }} }}\n"
        ),
    );
    let before = project.tree();
    let args = [
        "code",
        "add",
        "rust",
        "--repository",
        &url,
        "--version",
        version,
    ];
    assert_eq!(failure(&project, &args, &[], 1), "unreadable-edit");
    assert_eq!(server.connections(), 0);
    assert_eq!(cached(&home), Vec::<PathBuf>::new());
    unchanged(
        &before,
        &project.tree(),
        "a pin that cannot be written in place",
    );
}

#[test]
fn refuses_a_file_that_is_no_valid_module_and_an_unusable_proxy() {
    let server = GrammarServer::start();
    let (_, tags, version) = fixture("rust");
    // The magic and the version of WebAssembly, then a section that is cut short.
    server.serve_grammar("rust", version, b"\0asm\x01\0\0\0\x01\x05ab".to_vec(), tags);
    let url = server.url();
    let mut project = Project::new();
    let home = project.home();
    let before = project.tree();
    let args = [
        "code",
        "add",
        "rust",
        "--repository",
        &url,
        "--version",
        version,
    ];
    assert_eq!(failure(&project, &args, &[], 1), "fetch-failed");
    assert_eq!(cached(&home), Vec::<PathBuf>::new());
    unchanged(&before, &project.tree(), "an invalid module");

    let connections = server.connections();
    let socks = format!("socks5://{}", url.trim_start_matches("http://"));
    // `NO_PROXY` exempting the host changes nothing: a redirect could leave it.
    for (variable, proxy, exempt) in [
        ("ALL_PROXY", socks.as_str(), None),
        ("https_proxy", "socks5h://127.0.0.1:1", None),
        ("HTTP_PROXY", socks.as_str(), Some("127.0.0.1")),
    ] {
        let mut env = vec![(variable, Path::new(proxy))];
        env.extend(exempt.map(|host| ("NO_PROXY", Path::new(host))));
        let output = project.mara(&args, &env).output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let scheme = proxy.split("://").next().unwrap();
        assert!(
            stderr.contains("error[fetch-failed]")
                && stderr.contains(variable)
                && stderr.contains(scheme),
            "{stderr}"
        );
    }
    assert_eq!(
        server.connections(),
        connections,
        "an unusable proxy connects to nothing"
    );
    unchanged(&before, &project.tree(), "an unusable proxy");
}

/// Each language of the built-in catalog, added from its real repository: run with `--ignored`
/// before each release. It connects to the catalog's hosts.
#[test]
#[ignore = "fetches the catalog's grammars from their real repositories"]
fn adds_each_catalog_language_from_its_real_repository() {
    let catalog = include_str!("../../src/cli/code/catalog.toml");
    let languages =
        (catalog.lines()).filter_map(|line| line.strip_prefix("[languages.")?.strip_suffix(']'));
    for language in languages {
        let (path, sample, declaration) = match language {
            "rust" => ("src/lib.rs", MARKED, "login"),
            "python" => (
                "scripts/login.py",
                "# @mara implements REQ-LOGIN\ndef login():\n    pass\n",
                "login",
            ),
            other => panic!("add a marked sample of {other} to this test"),
        };
        let mut project = Project::new();
        project.home();
        project.write(path, sample);
        let result = project.json(&["code", "add", language], 0);
        assert_eq!(result["fetched"], true, "{language}: {result}");
        let found = endpoints(&project, "REQ-LOGIN");
        assert_eq!(found.len(), 1, "{language}: {found:?}");
        assert_eq!(
            found[0].2,
            Value::from(declaration),
            "{language}: {found:?}"
        );
    }
}
