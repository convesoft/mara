//! VER-PROGRAM-REACH-CASES: every way a program reached outside its evaluation in
//! EVD-SOUFFLE-PROGRAM-REACH, as a declared program consumed by invariants and a query, run by
//! `project validate` and `query execute`.
//!
//! The sandbox also holds `secret`, a file with a marker line, and a `libfunctors.so` built
//! with `cc` whose constructor creates a marker file and prints a marker. After every case no
//! marker file exists, neither marker is printed, and every file in the sandbox, `TMPDIR`
//! included, has the name, bytes and modification time it had before.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{QUERY, Run, Sandbox, both_severities};

const SECRET: &str = "SECRET-MARKER-7f3a";
const LIBRARY: &str = "LIBRARY-MARKER-91c2";

/// A sandbox with the secret and the library in `lib/`.
struct Reach {
    sandbox: Sandbox,
}

impl Reach {
    fn new() -> Reach {
        let sandbox = Sandbox::new();
        let root = sandbox.root();
        fs::write(root.join("secret"), format!("{SECRET}\n")).unwrap();
        fs::create_dir_all(root.join("lib")).unwrap();
        let source = root.join("lib/functors.c");
        let marker = root.join("library-loaded");
        let code = format!(
            "#include <fcntl.h>\n#include <stdio.h>\n#include <stdint.h>\n\
             __attribute__((constructor)) static void loaded(void) {{\n\
               open(\"{}\", O_CREAT | O_WRONLY, 0644);\n\
               fprintf(stderr, \"{LIBRARY}\\n\");\n}}\n\
             int32_t f(int32_t x) {{ return x; }}\n",
            marker.display()
        );
        fs::write(&source, code).unwrap();
        let status = Command::new("cc")
            .args(["-shared", "-fPIC", "-o"])
            .arg(root.join("lib/libfunctors.so"))
            .arg(&source)
            .status()
            .expect("run cc");
        assert!(status.success(), "cc failed");
        fs::remove_file(&source).unwrap();
        Reach { sandbox }
    }

    fn secret(&self) -> PathBuf {
        self.sandbox.root().join("secret")
    }

    fn library(&self) -> PathBuf {
        self.sandbox.root().join("lib/libfunctors.so")
    }

    /// Validates the project with `program` as `.mara/rules/p.dl`, consumed by an invariant of
    /// each severity and a query on `bad`, then runs the query, and checks that nothing outside
    /// the evaluation was touched. The query fails with the program's refusal and runs nothing,
    /// or returns a row for each failure that validation reports. Returns the validation.
    fn validate(&self, program: &str, extra: &[(&str, &Path)]) -> Run {
        let sandbox = &self.sandbox;
        sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
        let before = sandbox.files();
        let run = sandbox.mara_with(&["project", "validate"], extra);
        let query = sandbox.mara_with(&QUERY, extra);
        for run in [&run, &query] {
            let output = format!("{}{}", run.json, run.stderr);
            assert!(
                !output.contains(SECRET),
                "{program}: the secret was read: {output}"
            );
            assert!(
                !output.contains(LIBRARY),
                "{program}: the library was loaded"
            );
        }
        assert!(
            !sandbox.root().join("library-loaded").exists(),
            "{program}: library loaded"
        );
        let after = sandbox.files();
        let names = |files: &std::collections::BTreeMap<PathBuf, _>| {
            files.keys().cloned().collect::<Vec<_>>()
        };
        assert_eq!(names(&after), names(&before), "{program}: files changed");
        assert!(after == before, "{program}: a file's bytes or time changed");
        if run.with_code("program-refused").is_empty() {
            assert_eq!(query.status, 0, "{program}: {query:#?}");
            let rows = query.json["rows"].as_array().unwrap().len();
            assert_eq!(
                rows,
                run.with_code("as-error").len(),
                "{program}: {query:#?}"
            );
        } else {
            assert_eq!(query.json["error"]["code"], "program-failed", "{program}");
            let diagnostics = &query.json["error"]["diagnostics"];
            assert_eq!(
                diagnostics[0]["code"], "program-refused",
                "{program}: {query:#?}"
            );
            assert_eq!((query.spawns(), query.runs()), (0, 0), "{program}");
        }
        run
    }

    /// Requires `program` to be refused before any run.
    #[track_caller]
    fn refused(&self, program: &str) {
        let run = self.validate(program, &[]);
        assert_eq!(run.status, 1, "{program}: {run:#?}");
        assert_eq!(
            run.with_code("program-refused").len(),
            1,
            "{program}: {run:#?}"
        );
        assert_eq!((run.spawns(), run.runs()), (0, 0), "{program}");
    }
}

/// `bad` holds every accepted item, when `condition` holds.
fn admitted(condition: &str) -> String {
    format!(
        ".include \"mara.dl\"\n.decl bad(m: Mid)\n\
         bad(m) :- item(m, _, _, \"accepted\", _, _){condition}.\n"
    )
}

#[test]
fn input_and_output_directives_are_refused() {
    let reach = Reach::new();
    let written = reach.sandbox.root().join("written");
    reach.refused(&format!(
        "{}.output bad(IO=file, filename=\"{}\")\n",
        admitted(""),
        written.display()
    ));
    reach.refused(&format!(
        "{}.decl s(x: symbol)\n.input s(IO=file, filename=\"{}\")\n",
        admitted(""),
        reach.secret().display()
    ));
}

#[test]
fn functors_and_pragmas_are_refused() {
    let reach = Reach::new();
    let rules = reach.sandbox.project().join(".mara/rules");
    fs::create_dir_all(&rules).unwrap();
    fs::copy(reach.library(), rules.join("libfunctors.so")).unwrap();
    reach.refused(&format!(
        ".functor f(x: number): number\n{}",
        admitted(", @f(1) = 1")
    ));
    let lib = reach.sandbox.root().join("lib");
    for pragma in [
        format!(".pragma \"library-dir\" \"{}\"\n", lib.display()),
        ".pragma \"libraries\" \"functors\"\n".to_owned(),
        format!(".pragma \"output-dir\" \"{}\"\n", lib.display()),
    ] {
        reach.refused(&format!("{pragma}{}", admitted("")));
    }
}

/// The library in the program's directory, in `mara`'s working directory and on
/// `LD_LIBRARY_PATH` is never loaded, and the program's rows are returned.
#[test]
fn a_library_beside_the_program_is_not_loaded() {
    let reach = Reach::new();
    let project = reach.sandbox.project();
    let lib = reach.sandbox.root().join("lib");
    let places = [project.join(".mara/rules"), project.clone()];
    for place in &places {
        fs::create_dir_all(place).unwrap();
        fs::copy(reach.library(), place.join("libfunctors.so")).unwrap();
        let run = reach.validate(&admitted(""), &[]);
        assert_eq!(run.with_code("as-error").len(), 2, "{run:#?}");
        fs::remove_file(place.join("libfunctors.so")).unwrap();
    }
    let run = reach.validate(&admitted(""), &[("LD_LIBRARY_PATH", &lib)]);
    assert_eq!(run.with_code("as-error").len(), 2, "{run:#?}");
    assert_eq!((run.spawns(), run.runs()), (1, 1));
}

#[test]
fn line_directives_are_refused() {
    let reach = Reach::new();
    let secret = reach.secret();
    let secret = secret.display();
    for directive in [
        format!("#line 1 \"{secret}\"\n"),
        format!("# 1 \"{secret}\"\n"),
        format!(".decl z(x: number) #line 1 \"{secret}\"\n"),
        format!("/* c */ #line 1 \"{secret}\"\n"),
    ] {
        reach.refused(&format!(
            "{directive}.decl faulty(x: number, x: symbol)\n{}",
            admitted("")
        ));
    }
}

#[test]
fn file_and_include_names_are_refused() {
    let reach = Reach::new();
    reach.refused(&format!(
        ".decl f(x: symbol)\nf(__FILE__).\n{}",
        admitted("")
    ));
    reach.refused(&format!(
        ".decl f(x: symbol)\nf(__INCL__).\n{}",
        admitted("")
    ));
}

/// A value holding a newline and a tab stays one value.
#[test]
fn a_value_with_a_newline_and_a_tab_is_one_row() {
    let reach = Reach::new();
    let program = format!(
        ".decl v(x: symbol)\nv(\"a\\nb\\tc\").\n{}",
        admitted(", v(x), x = \"a\\nb\\tc\", 1 = count : v(_)")
    );
    let run = reach.validate(&program, &[]);
    assert_eq!(run.with_code("as-error").len(), 2, "{run:#?}");
    assert_eq!(run.with_code("as-warning").len(), 2);
}

#[test]
fn qualifiers_that_read_or_write_are_refused() {
    let reach = Reach::new();
    reach.refused(&format!(
        ".decl r(x: symbol) output printsize\n{}",
        admitted("")
    ));
    reach.refused(&format!(".decl r(x: symbol) input\n{}", admitted("")));
}

#[test]
fn constructs_whose_value_differs_or_that_soufflé_gets_wrong_are_refused() {
    let reach = Reach::new();
    for program in [
        format!(
            ".decl r(x: symbol, y: symbol) choice-domain x\n{}",
            admitted("")
        ),
        format!(".decl r(x: float)\n{}", admitted("")),
        admitted(", 2 ^ 40 = 0"),
        admitted(", match(\"[\", \"a\")"),
        admitted(", match(\"\\\\cI\", \"\\t\")"),
        format!(".decl s(x: symbol)\ns(as(1, symbol)).\n{}", admitted("")),
        admitted(", -2147483648 % -1 = 0"),
        admitted(", 1 / 0 = 0"),
        admitted(", -2147483648 / -1 = 0"),
        format!(
            ".decl a(x: number) magic\na(1).\na(x) <= a(y) :- x < y.\n{}",
            admitted("")
        ),
        admitted(", to_number(2147483648.0) = 0"),
    ] {
        reach.refused(&program);
    }
}

/// `+`, `*` and shifts at the limits of 32 bits, `strlen` and `to_number` of a hexadecimal
/// string give the rows the review probes recorded natively (the fixtures also compare them,
/// `fixtures.rs`).
#[test]
fn integer_arithmetic_wraps_as_recorded() {
    let reach = Reach::new();
    let checks = ", 2147483647 + 2 = -2147483647, -2147483648 - 2 = 2147483646, \
                  2147483647 * 2 = -2, -2147483648 * 2 = 0, 1 bshl 31 = -2147483648, \
                  -2147483648 bshr 31 = -1, -2147483648 bshru 31 = 1, \
                  strlen(\"h\u{e9}\") = 3, to_number(\"0x10\") = 16";
    let run = reach.validate(&admitted(checks), &[]);
    assert_eq!(run.with_code("as-error").len(), 2, "{run:#?}");
}
