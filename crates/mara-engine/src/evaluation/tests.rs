use std::collections::BTreeMap;

use super::*;
use crate::program::{Files, admit};

const MAIN: &str = ".include \"mara.dl\"\n.include \"a/q.dl\"\n.include \"b/q.dl\"\n\
                    .decl r(m: Mid, n: number)\nr(m, 1) :- item(m, _, _, _, _, _), s(1), t(1).\n";

fn program() -> Program {
    let mut programs = BTreeMap::new();
    programs.insert("rules/main.dl".to_owned(), MAIN.as_bytes().to_vec());
    let a = ".once\n.decl s(x: number)\ns(1).\n";
    programs.insert("rules/a/q.dl".to_owned(), a.as_bytes().to_vec());
    let b = ".once\n.decl t(x: number)\nt(1). // é\n";
    programs.insert("rules/b/q.dl".to_owned(), b.as_bytes().to_vec());
    let files = Files {
        declarations: facts::MARA_DL.as_bytes(),
        programs: &programs,
    };
    admit("rules/main.dl", &files).unwrap()
}

fn limits() -> Limits {
    Limits::with_time(10)
}

#[test]
fn copies_name_each_file_distinctly_and_rewrite_only_include_paths() {
    let program = program();
    assert_eq!(
        program.files,
        ["rules/main.dl", "mara.dl", "rules/a/q.dl", "rules/b/q.dl"]
    );
    let copies = copies(&program);
    let names: Vec<&str> = copies.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["p0.dl", "mara.dl", "p2.dl", "p3.dl"]);
    let main = &copies[0].1;
    assert_eq!(
        main.lines().take(3).collect::<Vec<_>>(),
        [
            ".include \"mara.dl\"",
            ".include \"p2.dl\"",
            ".include \"p3.dl\""
        ]
    );
    assert_eq!(
        main.lines().skip(3).collect::<Vec<_>>(),
        MAIN.lines().skip(3).collect::<Vec<_>>()
    );
    assert_eq!(copies[1].1, facts::MARA_DL);
    assert_eq!(
        wrapper(&program, &["item"], &["r"]),
        ".include \"p0.dl\"\n.input item(IO=jsonfile, format=list, filename=\"item.json\")\n\
         .output r(IO=jsonfile, format=list, filename=\"out/r.json\")\n"
    );
}

fn exit(success: bool) -> Exit {
    Exit {
        success,
        limit: None,
    }
}

#[test]
fn compile_errors_map_to_project_files_and_their_lines() {
    let program = program();
    let stderr = "Warning: No rules/facts defined for relation t in file p3.dl at line 2\n\
                  .decl t(x: number)\n------^-----------\n\
                  Error: Unable to deduce type for variable m in file p0.dl at line 5\n\
                  r(m, 1) :- item(m, _, _, _, _, _), s(1), t(1).\n--^---\n\
                  Following constraints are incompatible:\n\
                  \x20  Type of variable 'x' should be a subtype of 'number' in file p3.dl at line 3\n\
                  t(1). // é\n--^--------\n\
                  1 errors generated, evaluation aborted\n";
    let fault = interpret(&program, exit(false), b"", stderr.as_bytes(), limits()).unwrap_err();
    let printed: Vec<String> = fault
        .diagnostics(&program, 9)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        printed,
        [
            "rules/b/q.dl:2: error[program-error]: Soufflé warns: No rules/facts defined for relation t\n\
             .decl t(x: number)\n      ^",
            "rules/main.dl:5: error[program-error]: Unable to deduce type for variable m\n\
             r(m, 1) :- item(m, _, _, _, _, _), s(1), t(1).\n  ^",
            "rules/b/q.dl:3: error[program-error]: Following constraints are incompatible:\n\
             \x20  Type of variable 'x' should be a subtype of 'number'\nt(1). // é\n  ^",
        ]
    );
}

#[test]
fn a_compile_warning_alone_is_an_error() {
    let program = program();
    let stderr =
        "Warning: Variable x only occurs once in file p0.dl at line 5\nr(m, 1) :- x.\n--------^\n";
    let fault = interpret(&program, exit(true), b"", stderr.as_bytes(), limits()).unwrap_err();
    let diagnostic = &fault.diagnostics(&program, 9)[0];
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        (diagnostic.code, diagnostic.path.as_str(), diagnostic.line),
        ("program-error", "rules/main.dl", 5)
    );
    assert!(
        diagnostic
            .message
            .ends_with("\nr(m, 1) :- item(m, _, _, _, _, _), s(1), t(1).\n        ^")
    );
}

#[test]
fn anything_else_soufflé_prints_is_a_fault() {
    let program = program();
    let runtime = "error: wrong string provided by `to_number(\"x\")` functor.\nFloating-point arithmetic exception signal in rule:\nr(..) :- \n   a(x).\nin file p0.dl [5:1-5:23]\n";
    let fault = interpret(&program, exit(false), b"", runtime.as_bytes(), limits()).unwrap_err();
    let diagnostic = &fault.diagnostics(&program, 9)[0];
    assert_eq!(
        (diagnostic.code, diagnostic.path.as_str(), diagnostic.line),
        ("program-fault", "rules/main.dl", 5)
    );
    assert!(
        diagnostic
            .message
            .contains("in file rules/main.dl [5:1-5:23]"),
        "{}",
        diagnostic.message
    );
    let warning = b"warning: wrong index position provided by substr(\"a\",100,1) functor.\n";
    let fault = interpret(&program, exit(true), b"", warning, limits()).unwrap_err();
    let diagnostic = &fault.diagnostics(&program, 9)[0];
    assert_eq!(
        (diagnostic.path.as_str(), diagnostic.line),
        (crate::config::CONFIG_PATH, 9)
    );
    assert!(
        diagnostic
            .message
            .starts_with("rules/main.dl: the evaluation failed: warning")
    );
    let fault = interpret(&program, exit(true), b"r\t1\n", b"", limits()).unwrap_err();
    assert!(
        matches!(fault, Fault::Runtime { at: None, .. }),
        "{fault:?}"
    );
    let fault = interpret(&program, exit(false), b"", b"", limits()).unwrap_err();
    assert!(matches!(fault, Fault::Runtime { .. }), "{fault:?}");
    let fault = interpret(&program, exit(false), b"", b"std::bad_alloc\n", limits()).unwrap_err();
    assert_eq!(fault, Fault::Limit(Limit::Memory, limits()));
    let stopped = Exit {
        success: false,
        limit: Some(Limit::Time),
    };
    let fault = interpret(&program, stopped, b"", b"", limits()).unwrap_err();
    let diagnostic = &fault.diagnostics(&program, 9)[0];
    assert!(
        diagnostic
            .message
            .ends_with("time limit of 10 s (`[rules] time_limit_seconds`)")
    );
    assert_eq!(interpret(&program, exit(true), b"", b"", limits()), Ok(()));
}

#[test]
fn outputs_are_read_only_when_they_fit_the_relation() {
    let program = program();
    let rows = read_output(
        &program,
        "r",
        br#"[["a\tb\n",1],["c",-2147483648]]"#,
        limits(),
    );
    assert_eq!(
        rows,
        Ok(vec![
            vec![Value::Symbol("a\tb\n".into()), Value::Number(1)],
            vec![Value::Symbol("c".into()), Value::Number(i32::MIN)]
        ])
    );
    for (bytes, problem) in [
        (&b"[[\"\xc3\",1]]"[..], "not valid UTF-8"),
        (b"[[\"a\"]]", "row 1"),
        (b"[[\"a\",2147483648]]", "row 1"),
        (b"[[1,1]]", "row 1"),
        (b"[[\"a\",1]", "not JSON"),
        (b"{}", "list of rows"),
    ] {
        let fault = read_output(&program, "r", bytes, limits()).unwrap_err();
        let Fault::Output { problem: found, .. } = &fault else {
            panic!("{fault:?}")
        };
        assert!(found.contains(problem), "{found}");
    }
    let small = Limits {
        output_bytes: 4,
        ..limits()
    };
    assert_eq!(
        read_output(&program, "r", b"[[\"a\",1]]", small),
        Err(Fault::Limit(Limit::Output, small))
    );
}

#[test]
fn the_key_covers_facts_program_inputs_outputs_and_limits() {
    let program = program();
    let input = Relation::new("candidate", vec![vec![Value::Symbol("a".into())]]);
    let other = Relation::new("candidate", vec![vec![Value::Symbol("b".into())]]);
    let snapshot = SnapshotKey(Hash::of_bytes(b"snapshot"));
    let base = key(Some(snapshot), &program, &[&input], &["r"], limits());
    assert_eq!(
        base,
        key(Some(snapshot), &program, &[&input], &["r"], limits())
    );
    assert_ne!(base, key(None, &program, &[&input], &["r"], limits()));
    assert_ne!(
        base,
        key(Some(snapshot), &program, &[&other], &["r"], limits())
    );
    assert_ne!(
        base,
        key(Some(snapshot), &program, &[&input], &[], limits())
    );
    assert_ne!(
        base,
        key(
            Some(snapshot),
            &program,
            &[&input],
            &["r"],
            Limits::with_time(1)
        )
    );
}
