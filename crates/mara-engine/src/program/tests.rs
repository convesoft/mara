//! The cases of VER-ADMITTED-LANGUAGE in `docs/rules/verification.mara.md` that are not
//! review probes on disk, and the parser's share of VER-PROGRAM-REACH-CASES: what it refuses.

use std::collections::BTreeMap;

use super::*;

const DECLARATIONS_TEXT: &str = include_str!("../../tests/programs/trial/mara.dl");

fn admit_files(files: &[(&str, &str)]) -> Result<Program, Refusal> {
    let programs: BTreeMap<String, Vec<u8>> = files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec()))
        .collect();
    let files = Files {
        declarations: DECLARATIONS_TEXT.as_bytes(),
        programs: &programs,
    };
    admit(files_root(&programs), &files)
}

fn files_root(programs: &BTreeMap<String, Vec<u8>>) -> &str {
    programs
        .keys()
        .find(|path| path.ends_with("p.dl"))
        .expect("a root p.dl")
}

fn admit_text(text: &str) -> Result<Program, Refusal> {
    admit_files(&[("p.dl", text)])
}

#[track_caller]
fn admitted(text: &str) -> Program {
    match admit_text(text) {
        Ok(program) => program,
        Err(refusal) => panic!("refused: {refusal:?}\n{text}"),
    }
}

/// Asserts that `text` is refused at the place of the first `⟨` in it, which is removed.
#[track_caller]
fn refused_at_mark(text: &str) -> Refusal {
    let at = text.find('⟨').expect("a mark");
    let text = text.replacen('⟨', "", 1);
    let expected = locate("p.dl", &text, &line_starts(&text), at);
    match admit_text(&text) {
        Ok(_) => panic!("admitted:\n{text}"),
        Err(refusal) => {
            assert_eq!(
                (refusal.location.line, refusal.location.column),
                (expected.line, expected.column),
                "{}\n{text}",
                refusal.message
            );
            refusal
        }
    }
}

#[track_caller]
fn boundary_refused(text: &str, name: &str) -> Refusal {
    match admitted(text).boundary(name) {
        Err(BoundaryError::Refused(refusal)) => refusal,
        other => panic!("{name} is not refused: {other:?}"),
    }
}

#[test]
fn hash_lines_are_refused_wherever_soufflé_reads_them() {
    for text in [
        "⟨#line 1 \"secret\"\n.decl r(x:number)\nr(\"a\").",
        "⟨# 1 \"secret\"\n.decl r(x:number)\nr(\"a\").",
        "  ⟨#line 1 \"secret\"\n.decl r(x:number)\nr(\"a\").",
        ".decl s(x:number) ⟨#line 1 \"secret\"\n.decl r(x:number)\nr(\"a\").",
        ".decl s(x:number) ⟨# 1 \"secret\"\n.decl r(x:number)\nr(\"a\").",
        "/* comment */⟨#line 1 \"secret\"\n.decl r(x:number)\nr(\"a\").",
        "/* comment */ ⟨# 1 \"secret\"\n.decl r(x:number)\nr(\"a\").",
    ] {
        refused_at_mark(text);
    }
}

#[test]
fn every_left_out_construct_is_refused_at_its_token() {
    for text in [
        // Input and output, which Mara generates.
        ".decl r(x:number)\n⟨.output r(IO=file, filename=\"/tmp/out\")",
        ".decl s(x:symbol)\n⟨.input s(IO=file, filename=\"secret\")",
        ".decl r(x:number)\n⟨.printsize r",
        ".decl r(x:number)\n⟨.limitsize r(n=3)",
        ".decl r(x:symbol) ⟨output printsize",
        ".decl r(x:symbol) ⟨input",
        ".decl r(x:symbol) ⟨printsize",
        // Native code and directives.
        "⟨.functor f(x: number): number",
        "⟨.pragma \"library-dir\" \".\"",
        "⟨.pragma \"libraries\" \"functors\"",
        "⟨.pragma \"output-dir\" \"/tmp\"",
        "⟨.lattice L<> { Bottom -> 0 }",
        "⟨.number_type N",
        ".decl r(x:number)\nr(⟨@f(1)).",
        "⟨@[foo]\n.decl r(x:number)",
        ".decl r(x:symbol)\nr(⟨__FILE__).",
        ".decl r(x:symbol)\nr(⟨__INCL__).",
        ".decl r(x:number)\nr(⟨__LINE__).",
        // Values that depend on evaluation order.
        ".decl r(x:number)\nr(⟨autoinc()).",
        ".decl r(x:number)\nr(⟨$).",
        ".decl r(x:symbol, n:number)\nr(\"x\", ⟨ord(\"x\")).",
        ".decl s(x:symbol, y:symbol)\n.decl r(x:symbol, y:symbol) ⟨choice-domain x",
        ".decl r(x:number)\nr(0). r(x + 1) :- r(x), x < 9, ⟨recursive_iteration_cnt() < 3.",
        // Constructs whose hosts differ or that Soufflé gets wrong.
        ".decl r(x:⟨float)",
        ".decl r(x:⟨unsigned)",
        ".type F <: ⟨float",
        ".decl r(x:number)\nr(⟨1.5).",
        ".decl r(x:number)\nr(to_number(⟨2147483648.0)).",
        ".decl r(x:symbol)\nr(to_string(⟨4294967295u)).",
        ".decl r(x:number)\nr(⟨1u).",
        ".decl r(x:number)\nr(⟨1e5).",
        ".decl r(x:number)\nr(⟨1.2.3.4).",
        ".decl a(n:number)\n.decl r(x:symbol)\nr(⟨as(n, symbol)) :- a(n).",
        ".decl a(x:number) ⟨magic",
        ".decl a(x:number) ⟨no_magic",
        ".decl a(x:number) ⟨btree_delete",
        ".decl a(x:number)\na(x) ⟨<= a(y) :- x <= y.",
        ".decl r(x:symbol)\nr(\"fail\") :- ⟨match(\"[\", \"a\").",
        ".decl r(x:number)\nr(10 ⟨/ 2).",
        ".decl r(x:number)\nr(10 ⟨% 3).",
        ".decl r(x:number)\nr(2 ⟨^ 40).",
        ".decl r(x:number)\n.decl a(x:number)\nr(⟨mean x : a(x)).",
        ".decl r(x:number)\nr(⟨to_float(1)).",
        ".decl r(x:number)\nr(⟨to_unsigned(1)).",
        ".type T = ⟨A {x:number} | B {}",
        ".decl r(x:number, y:number) ⟨(x)",
        ".decl r(x:number⟨<>)",
        ".decl r(x:number)\nr(1) :- ⟨//! inner\n true.",
        ".type X\n⟨.decl r(x:X)",
    ] {
        refused_at_mark(text);
    }
}

#[test]
fn integers_outside_signed_32_bits_are_refused() {
    for text in [
        ".decl r(x:number)\nr(⟨2147483648).",
        ".decl r(x:number)\nr(-⟨2147483649).",
        ".decl r(x:number)\nr(⟨4294967295).",
        ".decl r(x:number)\nr(⟨0xffffffff).",
        ".decl r(x:number)\nr(⟨0x80000000).",
        ".decl r(x:number)\nr(⟨0b11111111111111111111111111111111).",
        ".decl r(x:number)\nr(⟨9999999999999999999999999999999999999999).",
        ".decl r(x:number)\nr(-(⟨2147483648)).",
        ".decl r(x:number)\nr(⟨- -2147483648).",
        ".decl r(x:number)\nr(1 - ⟨2147483648).",
    ] {
        refused_at_mark(text);
    }
    admitted(
        ".decl r(x:number)\nr(-2147483648). r(- 2147483648). r(2147483647). r(-0x80000000). r(0x7fffffff). r(0b101). r(010). r(--5).",
    );
}

#[test]
fn includes_name_only_mara_dl_and_declared_programs() {
    for path in [
        "/etc/passwd",
        "../p.dl",
        "a/../p.dl",
        "./q.dl",
        "",
        "a//b.dl",
        "q\\\\x.dl",
    ] {
        let text = format!(".include ⟨\"{path}\"\n");
        let at = text.find('⟨').unwrap();
        let text = text.replacen('⟨', "", 1);
        let refusal = admit_files(&[("p.dl", &text), ("q.dl", "")]).unwrap_err();
        assert_eq!(
            refusal.location.column,
            at + 1,
            "{path}: {}",
            refusal.message
        );
    }
    let refusal = admit_files(&[("p.dl", ".include \"other.dl\"\n")]).unwrap_err();
    assert_eq!(refusal.location.column, 10, "{}", refusal.message);
    for text in [
        ".include /* c */ \"q.dl\"",
        ".include\n\"q.dl\"",
        ".comp C { .include \"q.dl\" }",
        "/// doc\n.include \"q.dl\"",
        ".decl r(x:number)\nr(1) :- .include \"q.dl\"",
    ] {
        assert!(
            admit_files(&[("p.dl", text), ("q.dl", ".decl q(x:number)")]).is_err(),
            "{text}"
        );
    }
    let program = admit_files(&[
        (
            "rules/p.dl",
            ".include \"mara.dl\"\n.include \"lib/q.dl\"\n.include \"mara.dl\"",
        ),
        (
            "rules/lib/q.dl",
            ".once\n.include \"mara.dl\"\n.decl q(m: Mid)",
        ),
    ])
    .unwrap();
    assert_eq!(
        program.files,
        ["rules/p.dl", DECLARATIONS, "rules/lib/q.dl"]
    );
    assert!(program.boundary("q").is_ok());
    assert!(program.boundary("item").is_ok());
    // A declared program named `mara.dl` next to the including file is that file, as Soufflé
    // looks next to the including file first.
    let program = admit_files(&[
        ("rules/p.dl", ".include \"mara.dl\""),
        ("rules/mara.dl", ".decl own(x:number)"),
    ])
    .unwrap();
    assert_eq!(program.files, ["rules/p.dl", "rules/mara.dl"]);
}

#[test]
fn a_file_included_twice_starts_with_once() {
    let refusal = admit_files(&[
        ("p.dl", ".include \"q.dl\"\n.include \"q.dl\""),
        ("q.dl", ".decl q(x:number)"),
    ])
    .unwrap_err();
    assert_eq!(refusal.location.line, 2);
    admitted(".include \"mara.dl\"\n.include \"mara.dl\"");
    let refusal = admit_files(&[
        ("p.dl", ".include \"q.dl\""),
        ("q.dl", ".decl q(x:number)\n.once"),
    ])
    .unwrap_err();
    assert_eq!(refusal.location.line, 2, "{}", refusal.message);
    let refusal = admit_files(&[("p.dl", ".include \"p.dl\"")]).unwrap_err();
    assert!(refusal.message.contains("`.once`"), "{}", refusal.message);
}

#[test]
fn boundary_relations_hold_only_symbols_and_numbers() {
    let refusal = boundary_refused(".type R = [x:number]\n.decl r(x:R)", "r");
    assert_eq!(refusal.location.column, 11);
    boundary_refused(".type R = [x:number]\n.type A = R\n.decl r(x:A)", "r");
    boundary_refused(
        ".type R = [x:number]\n.comp C<T> { .decl r(x:T) }\n.init c = C<R>",
        "c.r",
    );
    boundary_refused(
        ".type R = [x:number]\n.comp C<T> { .decl r(x:T) }\n.comp D<U> : C<U> { }\n.init d = D<R>",
        "d.r",
    );
    boundary_refused(
        ".comp C { .type R = [x:number] .decl r(x:R) }\n.init c = C",
        "c.r",
    );
    boundary_refused(".decl r(x:number) inline", "r");
    refused_at_mark(".type R = [x:number]\n.type S <: ⟨R");
    refused_at_mark(".type R = [x:number]\n.type U = R | ⟨number");
    refused_at_mark(".type A <: symbol\n.type B <: number\n.type U = A | ⟨B");
    refused_at_mark(".type A = B\n.type ⟨B = A");
    refused_at_mark(".decl r(x:⟨Undeclared)");
    refused_at_mark(
        ".type R = [x:number]\n.decl a(x:R)\n.decl r(x:number)\nr(x) :- a([x]), a(y), [x] ⟨< y.",
    );
    refused_at_mark(
        ".type R = [x:number]\n.decl a(x:R)\n.decl r(x:number)\nr(1) :- a(y), y ⟨>= nil.",
    );
    admitted(
        ".type R = [x:number]\n.decl a(x:R)\n.decl r(x:number)\nr(x) :- a([x]), a(y), [x] != y, y = nil.",
    );
    // The same record type is admitted where it does not cross.
    let program = admitted(
        ".type R = [x:number, next:R]\n.decl a(x:R)\na([1, nil]).\n.decl r(x:number)\nr(x) :- a([x, _]).",
    );
    assert!(program.boundary("r").is_ok());
    assert_eq!(program.relations["a"].columns[0].kind, ColumnKind::Record);
    assert_eq!(program.boundary("missing"), Err(BoundaryError::Undeclared));
    let program = admitted(
        ".type Mid <: symbol\n.type Big = Small\n.type Small <: number\n.type Either = Mid | Text\n.type Text <: symbol\n.decl r(a:Mid, b:Big, c:Either)",
    );
    let kinds: Vec<_> = program.relations["r"]
        .columns
        .iter()
        .map(|column| column.kind)
        .collect();
    assert_eq!(
        kinds,
        [ColumnKind::Symbol, ColumnKind::Number, ColumnKind::Symbol]
    );
}

#[test]
fn components_resolve_as_soufflé_resolves_them() {
    let program = admitted(concat!(
        ".decl g(x:number) g(1).\n",
        ".comp O { .decl g(x:symbol) .comp I { .decl r(x:symbol) r(x) :- g(x). } .init i = I }\n",
        ".init o = O\n",
        ".comp A<T> { .type Local <: T .decl r(x:Local) overridable }\n",
        ".comp B<U> : A<U> { .override r r(1). }\n",
        ".init b = B<number>\n",
        ".decl top(x:b.Local) top(x) :- b.r(x).\n",
    ));
    let names: Vec<_> = program.relations.keys().map(String::as_str).collect();
    assert_eq!(names, ["b.r", "g", "o.g", "o.i.r", "top"]);
    assert_eq!(program.relations["b.r"].columns[0].kind, ColumnKind::Number);
    // An outer component's parameter is not visible in a nested component.
    refused_at_mark(".comp O<T> { .comp I { .decl r(x:⟨T) } .init i = I }\n.init o = O<number>");
    refused_at_mark(".comp A { .init ⟨a = A }\n.init x = A");
    refused_at_mark(".comp A : B { }\n.comp B : ⟨A { }\n.init x = A");
    refused_at_mark(".comp C<T> { }\n.init c = ⟨C");
    refused_at_mark(".init c = ⟨Missing");
    refused_at_mark(
        ".comp A { .decl ⟨r(x:number) }\n.comp B : A { }\n.comp C : A { }\n.comp D : B, C { }\n.init d = D",
    );
}

#[test]
fn expanding_components_is_bounded() {
    let mut text = String::from(".comp C0 { .decl r(x:number) }\n");
    for level in 1..30 {
        let previous = level - 1;
        text.push_str(&format!(
            ".comp C{level} {{ .init a = C{previous} .init b = C{previous} }}\n"
        ));
    }
    text.push_str(".init top = C29\n");
    let refusal = admit_text(&text).unwrap_err();
    assert!(refusal.message.contains("expands"), "{}", refusal.message);
}

#[test]
fn min_and_max_aggregates_take_only_numbers() {
    for text in [
        ".decl a(x:symbol)\n.decl r(x:symbol)\nr(x) :- x = ⟨min y : a(y).",
        ".decl a(x:symbol)\n.decl r(x:symbol)\nr(⟨max y : { a(y) }).",
        ".type Mid <: symbol\n.decl a(x:Mid)\n.decl r(x:Mid)\nr(⟨min y : a(y)).",
        ".decl a(x:symbol)\n.decl r(x:symbol)\nr(⟨min cat(y, \"\") : a(y)).",
        ".decl a(x:number)\n.decl s(x:symbol)\n.decl r(x:number, y:symbol)\nr(min x : a(x), ⟨min x : s(x)).",
        ".decl a(x:symbol)\n.decl r(x:symbol)\nr(⟨min y + y : a(y)).",
    ] {
        refused_at_mark(text);
    }
    admitted(concat!(
        ".decl a(x:number, s:symbol)\n.decl r(k:symbol, x:number)\n",
        "r(\"min\", min x : a(x, _)).\nr(\"max\", max x + 1 : { a(x, s), s != \"\" }).\n",
        "r(\"len\", min strlen(s) : a(_, s)).\nr(\"cmp\", max y : { a(x, _), y = x * 2 }).\n",
        "r(\"count\", count : { a(_, s), s = \"x\" }).\nr(\"sum\", sum x : a(x, _)).\n",
        ".decl t(x:symbol, y:symbol)\n.decl c(n:number)\nc(count : t(_, _)).\n",
        ".decl v(x:symbol)\nv(min(\"é\", \"a\", \"z\")). v(max(\"a\", \"b\")).\n",
    ));
}

#[test]
fn left_out_words_in_comments_and_strings_are_admitted() {
    admitted(concat!(
        "// .functor .pragma \"libraries\" @f(1) as match # ^ % / __FILE__\n",
        "/* .input r(IO=file, filename=\"/etc/passwd\") #line 1 \"x\" $ */\n",
        "/*\n#line 1 \"secret\"\n*/\n",
        ".decl r(x:symbol)\n",
        "r(\".output r(IO=file) #line 1 \\\"x\\\" choice-domain @f $ % ^ / */ //\").\n",
        "r(\"tab\\tnewline\\nquote\\\"backslash\\\\apostrophe\\'cr\\r\").\n",
        "/**/ r(\"é🙂\"). /***/ r(\"x\"). // comment without a line break",
    ));
}

#[test]
fn texts_that_soufflé_could_read_differently_are_refused() {
    for text in [
        ".decl r(x:symbol)\nr(\"⟨\\x61\").",
        ".decl r(x:symbol)\nr(\"⟨\\0\").",
        ".decl r(x:symbol)\nr(\"a⟨\nb\").",
        ".decl r(x:symbol)\nr(\"a⟨\tb\").",
        ".decl r(x:symbol)\nr(⟨\"unterminated).",
        "⟨/* unterminated",
        ".decl r(x:number)⟨\u{c}",
        "⟨\u{0}",
        ".decl d⟨écl(x:number)",
        ".decl r(x:number)\nr(1)⟨.decl s(x:number)",
        ".decl ⟨output(x:number)",
        ".decl r(⟨number:number)",
        ".decl r(x:number)\nr(count⟨) :- r(count).",
        ".decl r⟨?(x:number)",
        "⟨//! inner doc",
        "⟨/*! inner doc */",
        ".decl r(x:number)\nr(1) :- ⟨/// doc\n true.",
        ".decl r(x:number)\n⟨/// doc at the end",
        ".decl r(x:number) ⟨.once",
        ".decl r(x:number) .decl ⟨__r(x:number)",
        ".decl r(x:number)\nr(1) ⟨:: r(2).",
        ".decl r(x:number)\nr(x) :- x ⟨-> 1.",
    ] {
        refused_at_mark(text);
    }
    let refusal = admit_files(&[("p.dl", "")]).map(|_| ());
    assert!(refusal.is_ok());
    let programs = BTreeMap::from([("p.dl".to_owned(), b".decl r(x:symbol)\n\xff".to_vec())]);
    let files = Files {
        declarations: DECLARATIONS_TEXT.as_bytes(),
        programs: &programs,
    };
    let refusal = admit("p.dl", &files).unwrap_err();
    assert_eq!((refusal.location.line, refusal.location.column), (2, 1));
}

#[test]
fn the_admitted_grammar_is_read_in_full() {
    admitted(concat!(
        ".once\n",
        "/// A doc comment before a declaration.\n",
        ".type Mid <: symbol\n.type Id = Mid\n.type U = Mid | Id\n.type P = [a:number, b:symbol]\n",
        "/** A block doc comment. */\n",
        ".decl a, b(x:Mid, n:number) brie\n.decl e(x:symbol, y:symbol) eqrel\n",
        ".decl i(x:number) inline no_inline btree overridable\n",
        ".decl rec(p:P)\nrec([1, \"s\"]). rec(nil).\n",
        "a(\"m\", 1). b(\"n\", -2).\n",
        ".decl r(x:symbol, n:number)\n",
        "r(x, n) :- a(x, n), !b(x, _), (n = 1; n > 2), n >= 0, n <= 9, n != 3, n < 4, true, !false.\n",
        "r(cat(x, \"y\", \"z\"), strlen(x) + 1) :- a(x, _), contains(\"m\", x).\n",
        "r(substr(x, 0, 1), to_number(\"7\")) :- a(x, _).\n",
        "r(to_string(n * 2 - 1), n band 1 bor 2 bxor 3) :- a(_, n), n bshl 1 bshr 1 bshru 1 > 0.\n",
        "r(x, bnot n), r(x, lnot n) :- a(x, n), n land 1 lor 0 lxor 1 = 1.\n",
        "r(\"range\", n) :- n = range(0, 10, 2).\n",
        "r(\"agg\", c) :- c = count : { a(x, _), !b(x, _) }, c > sum n : a(_, n).\n",
        ".decl t(x:number)\nt(0). t(x + 1) :- t(x), x < 5. .plan 0:(1)\n",
        ".decl u(x:number, y:number)\nu(1, 1).\n",
        "u(x, z) :- u(x, y), u(y, z), x < 9. .plan 0:(1,2), 1:(2,1)\n",
        ".comp C<T> {\n  /// doc\n  .decl r(x:T)\n  r(x) :- t(x).\n}\n.init c = C<number>\n",
        "c.r(7).\n",
        ".decl q(x:number)\nq(x) :- c.r(x), x = y, c.r(y).\nq(x) :- t(x), x > 1.\nq(1).\n",
    ));
}

#[test]
fn plans_follow_the_rules_soufflé_checks() {
    for text in [
        ".decl a(x:number)\na(1).\n.decl r(x:number)\nr(x) :- a(x). ⟨.plan 0:(1)",
        ".decl a(x:number)\na(1). a(x + 1) :- a(x), x < 3. ⟨.plan 0:(2)",
        ".decl a(x:number)\na(1). a(x + 1) :- a(x), x < 3. ⟨.plan 1:(1)",
        ".decl a(x:number)\na(1). a(x + 1) :- a(x), a(y), x < y. ⟨.plan 0:(1,1)",
        ".decl a(x:number)\na(1). a(x + 1) :- a(x), x < 3. .plan 0:(⟨9999999999999999999999)",
        ".decl a(x:number)\na(1). a(x + 1) :- a(x), x < 3. .plan ⟨0x0:(1)",
        ".decl a(x:number)\na(1). ⟨.plan 0:(1)",
    ] {
        refused_at_mark(text);
    }
}

#[test]
fn nesting_is_bounded() {
    let deep = format!(
        ".decl r(x:number)\nr({}1{}).",
        "(".repeat(150),
        ")".repeat(150)
    );
    let refusal = admit_text(&deep).unwrap_err();
    assert!(refusal.message.contains("nests"), "{}", refusal.message);
    admitted(&format!(
        ".decl r(x:number)\nr({}1{}).",
        "(".repeat(25),
        ")".repeat(25)
    ));
}

#[test]
fn a_refusal_is_a_diagnostic_with_a_caret() {
    let refusal = admit_text(".decl r(x:number)\n\tr(10 / 2).").unwrap_err();
    let diagnostic = refusal.diagnostic();
    assert_eq!(
        (diagnostic.path.as_str(), diagnostic.line, diagnostic.code),
        ("p.dl", 2, "program-refused")
    );
    assert_eq!(
        diagnostic.message,
        "program refused: `/` is not admitted: division is left out\n\tr(10 / 2).\n\t     ^"
    );
}

#[test]
fn duplicate_declarations_are_refused_in_the_program() {
    let refusal = admit_text(".include \"mara.dl\"\n.decl item(x:symbol)").unwrap_err();
    assert_eq!(
        (refusal.location.path.as_str(), refusal.location.line),
        ("p.dl", 2)
    );
    refused_at_mark(".decl r(x:number)\n.decl ⟨r(x:number)");
    refused_at_mark(".type T <: number\n.type ⟨T <: number");
}

const AGGREGATED: &str = ".decl a(x:number)\na(1).\n.decl r(x:number)\n";

#[test]
fn no_disjunction_at_any_depth_of_an_aggregate_body() {
    for body in [
        "r(count : { (a(x)⟨; a(x)) }).",
        "r(count : { !(a(x)⟨; a(x)) }).",
        "r(sum x : { a(x), (x = 1⟨; x = 2) }).",
        "r(count : { a(_), c = count : { (a(y)⟨; a(y)) } }).",
    ] {
        refused_at_mark(&format!("{AGGREGATED}{body}"));
    }
    admitted(&format!(
        "{AGGREGATED}r(x) :- (a(x); a(x)), x = count : {{ (a(_)) }}."
    ));
}

#[test]
fn a_min_or_max_value_may_start_with_a_parenthesis() {
    for body in [
        "r(min (x + 1) : a(x)).",
        "r(max (x * 2) : a(x)).",
        "r(min (x) * 2 + 1 : a(x)).",
        "r(max (x) : { a(x) }).",
        "r(min(x, 2, 3)) :- a(x).",
    ] {
        admitted(&format!("{AGGREGATED}{body}"));
    }
    refused_at_mark(&format!("{AGGREGATED}r(min(x)⟨) :- a(x)."));
}

#[test]
fn numeric_evidence_counts_through_groups_and_record_fields() {
    for body in [
        "r(min x : { (a(x)) }).",
        "r(max y : { a(x), (y = x + 1) }).",
    ] {
        admitted(&format!("{AGGREGATED}{body}"));
    }
    let records = ".type P = [n:number, s:symbol]\n.type Q = P\n.decl b(p:Q)\nb([1, \"a\"]).\n.decl r(x:number)\n.decl t(x:symbol)\n";
    for body in [
        "r(min x : b([x, _])).",
        "r(max x : { b(p), p = [x, _] }).",
        "r(max x : { b(p), [x, _] = p }).",
    ] {
        admitted(&format!("{records}{body}"));
    }
    refused_at_mark(&format!("{records}t(⟨min s : b([_, s]))."));
}

#[test]
fn a_component_argument_is_a_single_name() {
    refused_at_mark(
        ".comp A { .type T <: number }\n.init a = A\n.comp B<T> { .decl r(x:T) r(1). }\n.init b = B<a⟨.T>",
    );
}

#[test]
fn directives_are_read_only_before_a_space_or_a_tab() {
    for text in [
        "⟨.decl\nr(x:number)",
        "⟨.decl/*comment*/r(x:number)",
        "⟨.type\nT <: number",
        ".decl a(x:number)\na(1). a(x + 1) :- a(x), x < 3. ⟨.plan\n0:(1)",
    ] {
        refused_at_mark(text);
    }
    admitted(".once\n.decl\tr(x:number)\n.type\tT <: number");
}

/// Programs that used to abort with a stack overflow or expand without bound: each is refused
/// with a diagnostic that names its limit.
#[test]
fn small_programs_cannot_exhaust_the_stack_or_memory() {
    let refused = |text: &str, limit: &str| {
        let refusal = admit_text(text).unwrap_err();
        assert!(refusal.message.contains(limit), "{}", refusal.message);
    };
    // Each component inherits its predecessor twice: 2^20 bodies, all of them empty.
    let mut text = String::from(".comp C0 { }\n");
    for level in 1..=20 {
        text.push_str(&format!(
            ".comp C{level} : C{p}, C{p} {{ }}\n",
            p = level - 1
        ));
    }
    text.push_str(".init x = C20\n");
    refused(&text, "100000");
    // Inheritance and instances 40,000 deep.
    let mut inherit = String::from(".comp C0 { }\n");
    let mut nest = inherit.clone();
    for level in 1..40_000 {
        let p = level - 1;
        inherit.push_str(&format!(".comp C{level} : C{p} {{ }}\n"));
        nest.push_str(&format!(".comp C{level} {{ .init x = C{p} }}\n"));
    }
    inherit.push_str(".init x = C39999\n");
    nest.push_str(".init x = C39999\n");
    refused(&inherit, "more than 100 levels deep");
    refused(&nest, "more than 100 levels deep");
    // 200,000 operands in one expression, and atoms nested 200,000 deep in arguments.
    let operands = format!(".decl r(x:number)\nr(1{}).", "+1".repeat(199_999));
    refused(&operands, "more than 100 levels deep");
    let atoms = format!(
        ".decl r(x:number)\nr({}1{}).",
        "a(".repeat(200_000),
        ")".repeat(200_000)
    );
    refused(&atoms, "more than 100 levels deep");
}

const DEPTH_REFUSAL: &str = "more than 100 levels deep";

#[track_caller]
fn refused_for_depth(text: &str) {
    match admit_text(text) {
        Ok(_) => panic!("admitted, {} bytes", text.len()),
        Err(refusal) => assert!(
            refusal.message.contains(DEPTH_REFUSAL),
            "{}",
            refusal.message
        ),
    }
}

/// `r(sum sum … sum 1 : a(_) : … : a(_)).`, the targets nested `depth` deep.
fn aggregate_targets(depth: usize) -> String {
    format!(
        "{AGGREGATED}r({}1{}).",
        "sum ".repeat(depth),
        " : a(_)".repeat(depth)
    )
}

#[test]
fn refuses_150_nested_aggregate_targets() {
    refused_for_depth(&aggregate_targets(150));
}

#[test]
fn refuses_4000_aggregate_target_prefixes() {
    refused_for_depth(&format!("{AGGREGATED}r({}1).", "sum ".repeat(4000)));
}

/// A construct's name, and the program that nests it a given depth deep.
type Nesting = (&'static str, fn(usize) -> String);

/// Each construct that can nest, as a program nesting it `depth` deep.
fn nested_constructs() -> Vec<Nesting> {
    vec![
        ("parentheses", |d| {
            format!("{AGGREGATED}r({}1{}).", "(".repeat(d), ")".repeat(d))
        }),
        ("groups", |d| {
            format!(
                "{AGGREGATED}r(1) :- {}a(1){}.",
                "(".repeat(d),
                ")".repeat(d)
            )
        }),
        ("negation", |d| {
            format!("{AGGREGATED}r(1) :- a(1), {}a(1).", "!".repeat(d))
        }),
        ("aggregate targets", aggregate_targets),
        ("aggregate bodies", |d| {
            format!(
                "{AGGREGATED}r({}1{}).",
                "count : { a(_), 1 = ".repeat(d),
                " }".repeat(d)
            )
        }),
        ("min and max targets", |d| {
            format!(
                "{AGGREGATED}r({}1{}).",
                "min (".repeat(d),
                ") : a(_)".repeat(d)
            )
        }),
        ("records", |d| {
            format!(
                ".decl r(x:symbol)\nr(x) :- x = \"a\", {}1{} = nil.",
                "[".repeat(d),
                "]".repeat(d)
            )
        }),
        ("unary minus", |d| {
            format!("{AGGREGATED}r({}1).", "- ".repeat(d))
        }),
        ("unary bnot", |d| {
            format!("{AGGREGATED}r({}1).", "bnot ".repeat(d))
        }),
        ("binary operators", |d| {
            format!("{AGGREGATED}r(1{}).", " + 1".repeat(d))
        }),
        ("call arguments", |d| {
            format!(
                ".decl r(x:symbol)\nr({}\"a\"{}).",
                "cat(".repeat(d),
                ")".repeat(d)
            )
        }),
        ("atom arguments", |d| {
            format!("{AGGREGATED}r({}1{}).", "a(".repeat(d), ")".repeat(d))
        }),
        ("component definitions", |d| {
            format!(
                "{}.decl r(x:number){}",
                ".comp C { ".repeat(d),
                " }".repeat(d)
            )
        }),
        ("component instances", |d| {
            let mut text = String::from(".comp C0 { .decl r(x:number) }\n");
            for level in 1..=d {
                text.push_str(&format!(".comp C{level} {{ .init x = C{} }}\n", level - 1));
            }
            text + &format!(".init top = C{d}\n")
        }),
        ("inheritance", |d| {
            let mut text = String::from(".comp C0 { .decl r(x:number) }\n");
            for level in 1..=d {
                text.push_str(&format!(".comp C{level} : C{} {{ }}\n", level - 1));
            }
            text + &format!(".init top = C{d}\n")
        }),
    ]
}

/// Every construct that can nest is refused 150 deep with the depth diagnostic, and 100,000
/// deep without exhausting the stack.
#[test]
fn every_nesting_construct_is_bounded() {
    for (name, program) in nested_constructs() {
        for depth in [150, 100_000] {
            let text = program(depth);
            match admit_text(&text) {
                Ok(_) => panic!("{name} {depth} deep: admitted"),
                Err(refusal) => assert!(
                    refusal.message.contains(DEPTH_REFUSAL),
                    "{name} {depth} deep: {}",
                    refusal.message
                ),
            }
        }
        // Shallow nesting is admitted, or refused for another reason than its depth.
        if let Err(refusal) = admit_text(&program(3)) {
            assert!(
                !refusal.message.contains(DEPTH_REFUSAL),
                "{name}: {}",
                refusal.message
            );
        }
    }
    // Types never recurse: a chain of 50,000 subtypes, aliases or records resolves.
    for chain in [
        |level: usize| format!(".type T{level} <: T{}\n", level - 1),
        |level: usize| format!(".type T{level} = T{}\n", level - 1),
        |level: usize| format!(".type T{level} = [x:T{}]\n", level - 1),
    ] {
        let mut text = String::from(".type T0 <: number\n");
        for level in 1..50_000 {
            text.push_str(&chain(level));
        }
        text.push_str(".decl r(x:T49999)");
        admitted(&text);
    }
}

/// The deepest programs the limits admit parse, resolve and are checked, in a debug build, on
/// a 1 MiB stack: half a spawned thread's default.
#[test]
fn programs_at_the_limits_fit_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            for (_, program) in nested_constructs() {
                for depth in (10..=150).step_by(10) {
                    let _ = admit_text(&program(depth));
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
