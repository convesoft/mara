# Soufflé evidence

---

<a id="01M7BF6HDDJJ0Y0WWT6RK17CDN"></a>

### Soufflé trial

| Type | Evidence |
| --- | --- |
| ID | EVD-SOUFFLE-TRIAL |
| Status | Accepted |
| Supports | [ADR-SOUFFLE-DATALOG](decisions-rules.mara.md#01M7BF6GSZ16MZQAGBY5M9TDKZ) |
| Supports | [ADR-SOUFFLE-AS-PROCESS](decisions-rules.mara.md#01M7BF6GT9PVXSKV1ANGEWA7WR) |

Soufflé 2.5, 2026-11-13. Corpora: 1× = 200 items and 266 edges, 16× = 3,200 and 4,256, 64× = 12,800 and 17,024. Times include process start and program compilation.

| Check | Result |
| --- | --- |
| Risk score with a threshold parameter; missing inputs reported as unavailable | Pass |
| Coverage: at least one accepted verification | Pass |
| Ancestry across a cycle; cycle members found | Pass |
| Missing versus unavailable external results, by completeness | Pass |
| JSON explanation of a failed gate condition, with document locations | Pass |
| `implementation_ready` gate: 5 conditions plus an unavailable outcome | Pass |
| Compile errors: undefined relation, arity, `Mid` versus `Id`, symbol versus number, unstratifiable negation | All caught, with file, line and caret |
| Native gate time: empty, 1×, 16×, 64× | 6.1, 6.0, 11.3, 29.5 ms; 18 MB peak memory at 64× |
| Native growth from 16× to 64× (4× data) | Time ×2.6–2.8, instructions ×2.9–3.5 |
| `-j8` against `-j1` | Slower |
| Provenance mode overhead at 16× | 1.5× (21.0 against 14.2 ms) |
| WASM gate time: empty, 1×, 16×, 64× | 11.9, 13.4, 21.7, 44.8 ms |
| Native against WASM outputs, all programs at 16× | Identical (6,696 rows) |
| JSON facts and outputs through generated directives, 2026-11-14 | Tabs, newlines, quotes and backslashes in values round-trip exactly; a component's relation is written; trial gate at 16×: 18 ms against 14 ms tab-separated, same 960 rows |
| WASM explanation, 2026-11-14: console input from the module's `stdin` hook, under Node 26 | Two proofs, one through a negated atom, byte-identical to native; 110 ms at 16× |
| Native binary, static C++ runtime, no OpenMP | 15 MB stripped, 2.6 MB xz; needs only libc and libm |
| WASM module | 6.1 MB, 1.2 MB gzip, 0.7 MB Brotli |
| Build time, 10 jobs | Native 107–120 s; Emscripten 66 s |
| Trial gate compiled with `souffle -g` | 3,261 lines of C++; `g++ -O2` 10.3 s, `clang++ -O2` 6.4 s, `g++ -O0` 4.6 s |

The trial gates approximate the gates of the `engineering` template at commit `d7f0e73`. Artifacts (Soufflé source and builds, programs, fact generator, WASM runner, benchmarks) are on the maintainer's machine in `~/Work/local/workspace/scratch/souffle-spike/`.

---

<a id="01M7CMW70TAFCJKPAX3VY94QMG"></a>

### Soufflé programs reach outside their evaluation

| Type | Evidence |
| --- | --- |
| ID | EVD-SOUFFLE-PROGRAM-REACH |
| Status | Accepted |
| Supports | [ADR-PROGRAM-PROFILE](decisions-rules.mara.md#01M7CMW6ZGC0PY8DVYNQDSBPRH) |
| Supports | [RISK-SOUFFLE-REACHES-OUTSIDE](rules-facts.mara.md#01M7CYC90BZBBXGC0CZ66W8ZEV) |

The static binary of [EVD-SOUFFLE-TRIAL](rules-evidence.mara.md#01M7BF6HDDJJ0Y0WWT6RK17CDN), run on 2026-11-14 as `souffle --no-preprocessor -F <facts> -D <out> <program>.dl`, from the fourth row on with `-l ""` where stated.

| Program | Result |
| --- | --- |
| `.output r(IO=file, filename="<absolute path>")` | Wrote that file, outside `<out>` |
| `.input s(IO=file, filename="/etc/hostname")` | Read that file into `s` |
| `.functor f(x: number): number`, with a `libfunctors.so` in the working directory | Loaded the library and ran its code |
| `.pragma "library-dir"` and `.pragma "libraries"` naming a library elsewhere | Loaded that library and ran its code |
| `.pragma "output-dir"` | No effect; `-D` won |
| Any program, with a `libfunctors.so` in the working directory or on `LD_LIBRARY_PATH` | Loaded the library and ran its constructor, with no `.functor` in the program. `-l ""` prevented it |
| `#line 1 "<file>"` or `# 1 "<file>"` before a faulty declaration | The diagnostic printed the first line of that file, with `--no-preprocessor` |
| `#line 1 "<file>"` after other text on the same line | The same file read |
| `__FILE__` in a fact | The absolute path of the program in its private directory, different in each directory |
| Default tab-separated output of one row whose value holds a newline and a tab | The same bytes as two separate rows |
| `.decl r(x: symbol) output printsize`, under a wrapper without output directives | Wrote `r.csv` in the working directory |
| `choice-domain`, with the wrapper writing one more relation | A different answer for the same input: `("a","c")` became `("a","b")` |
| JSON input of 1.5 into a `float` column, and of `"not-a-number"` into a `number` column | 1.4013e-45 and 0, without an error |
| `2 ^ 40` | −2147483648 natively, 2147483647 in WASM |
| `+`, `*`, shifts, `/` and `%` at the limits of 32 bits, `strlen`, `to_number` of a hexadecimal string | The same on both hosts; division by zero and an unparsable `to_number` fail on both |
| `match("[", m)` in a rule | `warning: wrong pattern provided`, exit 0, no rows |
| `explain` for a row whose value holds a newline, written as an escape | A proof for another row, whose value holds a backslash and `n` |
| `r(as(n, symbol)) :- a(n).` with `a(0)` | Segmentation fault natively; a row with an empty symbol in WASM |
| `.decl a(x: number) magic` with a subsumption clause, read by a gate condition | The clause is dropped: the condition has no row, on both hosts; with `no_magic` it has one |
| `match("\\cI", "\t")` | No row natively, one row in WASM, no message on either |
| `x % y` with −2147483648 and −1 | Arithmetic exception natively; 0 in WASM, so a condition on it passes |
| `x / 0` | Arithmetic exception on x64 and a trap in WASM. On ARM64 the division instruction does not trap and yields 0: read from Soufflé's code and the compiled instruction, not run |
| `to_number(2147483648.0)`, a float literal without the word `float` | −2147483648 natively, 2147483647 in WASM |
| `explain` of `r(x) :- a(x), x < "z".` | A leaf `4 < 5`: internal numbers instead of the symbols |
| Upstream on 2026-11-14 | 2.5 is the latest release. Fixed on master only: `magic` with subsumption (issue 2322) and an explanation fix (2585). By design: `choice-domain` order, the `libfunctors.so` load, `#line`, unchecked `as`. Unchanged on master: the JSON reader |
| `-t explain` on a rule with `autoinc()`, on a rule with an ADT constructor in its body, or on a program with a subsumption clause | Aborted; normal evaluation of the same programs succeeded |
| `-t explain` with records, aggregates, negation, components or `eqrel` | Explained |
| `nat(0). nat(x + 1) :- nat(x).` | Still running when killed after 5 s |
| `s("a"). s(cat(x, "a")) :- s(x).` | Still running when killed after 5 s |

The two programs that never ended use no records, ADTs or `autoinc()`.
