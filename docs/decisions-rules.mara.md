# Rules decisions

---

<a id="01M7BF6GSZ16MZQAGBY5M9TDKZ"></a>

### Write project logic in Soufflé Datalog

| Type | Decision |
| --- | --- |
| ID | ADR-SOUFFLE-DATALOG |
| Status | Accepted |
| Justifies | [DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE) |

- Chosen: Soufflé. A program declares its typed input and output relations, mistakes are rejected before evaluation, and modules are native (`.include`, `.once`, `.comp`).
- Fallback if Soufflé becomes unworkable: Nemo. Not chosen first because it is dynamically typed and silently skips mismatching rows.
- Rejected: GraphQL. Computed fields need custom resolvers or Mara-specific directives.
- Rejected: SPARQL. It has no modules or typed input and output declarations, so it needed a JSON wrapper plus separate SHACL constraints.
- Rejected: Rego (`regorus`). It forbids recursive rules.

---

<a id="01M7CQ57SYJ9BH5XMVVV9F6QA3"></a>

### Register invariants, gates and views in the configuration

| Type | Decision |
| --- | --- |
| ID | ADR-CONSUMERS-IN-CONFIGURATION |
| Status | Accepted |
| Justifies | [DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE) |

- Chosen: named configuration entries that point at a program's output relations. Programs stay plain Soufflé.
- Rejected: output relations that register themselves by name, such as `invariant_error_<name>`. Names would carry hidden meaning, and a message has no place.
- Rejected: reaching any output relation by program file and relation name. Helper relations would become public, without a description.

---

<a id="01M7BF6GT9PVXSKV1ANGEWA7WR"></a>

### Run Soufflé through its command line

| Type | Decision |
| --- | --- |
| ID | ADR-SOUFFLE-AS-PROCESS |
| Status | Accepted |
| Justifies | [DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) |

- Chosen: Soufflé's command-line contract: a bundled child process natively, the Emscripten build in the browser worker.
- Rejected: the C++ interface ([`souffle/SouffleInterface.h`](https://souffle-lang.github.io/interface)) and [SWIG](https://souffle-lang.github.io/swig). They run only programs translated with `souffle -g` and compiled; the interpreter's `ProgInterface` leaves `run()` and `loadAll()` empty (`src/interpreter/ProgInterface.h:258`). Every rule change would need a C++ compiler on the user's machine, and none of it works in a browser.
- Rejected: in-process evaluation. Even with `SOUFFLE_ALLOW_SIGNALS` (`src/include/souffle/SignalHandler.h:96`), integer division by zero raises SIGFPE, and any C++ exception reaches `SignalHandler::error()`, which calls `exit(1)` (`SignalHandler.h:148–154`). A child process contains both.

---

<a id="01M7CMW6ZGC0PY8DVYNQDSBPRH"></a>

### Refuse program constructs that reach outside the evaluation

| Type | Decision |
| --- | --- |
| ID | ADR-PROGRAM-PROFILE |
| Status | Accepted |
| Justifies | [DES-PROGRAM-PROFILE](rules-facts.mara.md#01M7CMW7051QVPFDWQBMB1A6K6) |
| Justifies | [REQ-EVALUATION-TIME-LIMIT](rules-evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0) |

- Chosen: Mara parses every program against the grammar of a small admitted language, with types resolved, and runs stock Soufflé 2.5 only on programs that pass, on parsed copies in a private directory with library loading off; a time limit bounds each evaluation ([EVD-SOUFFLE-PROGRAM-REACH](rules-evidence.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).
- Rejected: lists of refused or admitted words. Five reviews each found ways past one, and a word list cannot express literal types, casts or column types.
- Rejected: an evaluator inside the engine for the same language. It has no trial behind it and would reverse the choice of Soufflé; it stays the alternative if this boundary keeps failing.
- Rejected: patching Soufflé or tracking its master. The behaviours left out are by design upstream or differ between the two builds, and a fork must be verified again on every update.
- Rejected: trusting programs like build scripts. Validating an untrusted checkout would run its code.
- Rejected: forbidding records to bound evaluation. Plain recursion with arithmetic or `cat` never ends either.

---

<a id="01M7CT60PG5Q68ZNJ9125TZKXH"></a>

### Keep vocabulary out of the fact bridge

| Type | Decision |
| --- | --- |
| ID | ADR-GENERIC-FACT-BRIDGE |
| Status | Accepted |
| Justifies | [DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J) |
| Justifies | [DES-REQUEST-INPUTS](rules.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG) |

- Chosen: `mara.dl` declares only what every vocabulary has. A template declares its own inputs, such as results, and a request supplies them.
- Rejected: `verification_result` and `validation_result` in `mara.dl`. Other vocabularies would carry them, and changing a column would need a Mara release.
- Rejected: stored result files. Run results are not documentation.

---

<a id="01M7CT60PP6JY095YYYJMT9TVV"></a>

### Let Mara compute a gate's outcome

| Type | Decision |
| --- | --- |
| ID | ADR-MARA-COMPUTES-GATE-OUTCOME |
| Status | Accepted |
| Justifies | [DES-GATE-OUTCOME](rules.mara.md#01M7CT60KEEZRJCPJ0CPT7YT5B) |

- Chosen: a gate names subject, condition and unavailable relations, and Mara derives the outcome of each named item.
- Rejected: an outcome relation that each program fills. Every program would repeat the precedence of failed over unavailable and the handling of a wrong or empty scope.
- Rejected: a separate `required_validation` gate, as in the previous template. A validation is a named item of `release-ready`.

---

<a id="01M7CT60NYQQPN6DG5J6WJCMSA"></a>

### Keep validity, completeness and readiness apart

| Type | Decision |
| --- | --- |
| ID | ADR-VALIDITY-NOT-READINESS |
| Status | Accepted |
| Justifies | [REQ-TEMPLATE-ACCEPTANCE-RULES](template.mara.md#01M7CT60KVJ75RV9GPP3SEF0WX) |
| Justifies | [REQ-TEMPLATE-GATES-PROVE-READINESS](template-readiness.mara.md#01M7CT60MNF0XPXR9NB6X74F3B) |

- Chosen: invariants judge only whether accepted knowledge is authoritative, gates judge named items for a candidate, and missing downstream work is neither.
- Rejected: one validity that also covers implementation and results. It turns the whole development lifecycle into document validity.
- Rejected: implemented, verified or ready as a status or a field. They depend on the candidate and go stale.
- Rejected: a mandatory chain from goal to code. A small change needs only a requirement and its verification.

---

<a id="01M7CT60P4PXK6DMZZ28QQBBNY"></a>

### Ship a NASA-informed default risk matrix

| Type | Decision |
| --- | --- |
| ID | ADR-NASA-INFORMED-RISK-MATRIX |
| Status | Accepted |
| Justifies | [DES-TEMPLATE-RISK-LEVEL](template.mara.md#01M7CT60MFPF2Y871JJDZP5K0T) |

- Chosen: a default of 25 cells informed by NASA/SP-2011-3422 §4.3.2.6, which a project tailors. It is neither NASA's own matrix nor a compliance claim.
- Rejected: no default. Every result that depends on a risk level would stay unavailable until a project writes 25 cells.
- Rejected: likelihood times consequence as a score. A multiplication is not an adopted policy.

---

<a id="01M7CT60PAQ0JPNNETP6TH90DD"></a>

### Keep supersession links of retired items valid

| Type | Decision |
| --- | --- |
| ID | ADR-HISTORICAL-SUPERSESSION |
| Status | Accepted |
| Justifies | [DES-TEMPLATE-RELATION-LIFECYCLE](template.mara.md#01M7CT60M96Q91T5KXNQ1G58PP) |

- Chosen: a `Supersedes` link stays valid after the replacing item retires.
- Rejected: requiring an accepted source for every `Supersedes` link. A second replacement would invalidate the lineage that the first one records.

---

<a id="01M7CXC2NESPHY4HB3QXEDTAJ2"></a>

### Bind a result to the revision of what it checks

| Type | Decision |
| --- | --- |
| ID | ADR-RESULTS-BOUND-TO-BOTH-REVISIONS |
| Status | Accepted |
| Justifies | [DES-TEMPLATE-INPUTS](template-readiness.mara.md#01M7CT60N3AT6378YRQ62A5NM3) |

- Chosen: a result carries the revision of its Verification or Validation and of the Requirement, Need or Goal it was produced for, and counts only while both are current.
- Rejected: the revision of the Verification or Validation alone. A requirement that changes while its verification stays the same would keep its old passes.

---

<a id="01M7CZA3GYB8DH1YPKH0T06JZP"></a>

### Generate every input and output directive, as JSON

| Type | Decision |
| --- | --- |
| ID | ADR-GENERATED-JSON-IO |
| Status | Accepted |
| Justifies | [DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) |
| Justifies | [DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J) |

- Chosen: programs hold no input or output directive. Mara generates them for the relations a run needs and uses Soufflé's JSON files; a run at 16× takes 18 ms instead of 14 ms ([EVD-SOUFFLE-TRIAL](rules-evidence.mara.md#01M7BF6HDDJJ0Y0WWT6RK17CDN)).
- Rejected: Soufflé's default tab-separated files. A tab or a newline in a value reads as a column or row break ([EVD-SOUFFLE-PROGRAM-REACH](rules-evidence.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).
- Rejected: escaping values in tab-separated files. A program can still build such a value, and rules would see the escaped text.
