# Mara project format v1

This reference groups the typed design contracts for `.mara/project.toml` format
version 1. Each contract below is independently addressable in the Mara graph.

:::design m_01KY82WX4F42MN861CDCNZ9TGK
:id: DES-PROJECT-FORMAT-ROOT
:title: Project format root and TOML profile
:status: accepted
:kind: interface
:satisfies: REQ-PROJECT-ROOT
:satisfies: REQ-PROJECT-CONFIG-STRICT

## TOML profile

The file is one UTF-8 TOML 1.0 document without a byte-order mark. Standard TOML
comments and string forms are accepted. Duplicate keys, duplicate tables, and a
key redefined through dotted/table syntax are syntax errors. Environment-variable,
home-directory, command, and template expansion never occurs.

The root contains integer `format_version` and exactly these required tables:
`project`, `content`, `index`, `validation`, and `git`. `format_version` must be
integer `1` and not a string or float. Table and key order has no semantic effect.

## Closed tables

### `[project]`

| Key | Type | Required | Constraint |
|---|---|---:|---|
| `name` | string | yes | `[a-z][a-z0-9]*(?:-[a-z0-9]+)*` |
| `schema` | path string | yes | existing readable regular file |

`schema` resolves from project root and normally names `.mara/schema.yaml`.

:::

:::design m_01KY82WX4G28E30KBD8GE93SWC
:id: DES-PROJECT-CONTENT-DISCOVERY
:title: Project content discovery configuration
:status: accepted
:kind: interface
:satisfies: REQ-CONTENT-GLOBS
:satisfies: REQ-CONTENT-GITIGNORE
:satisfies: REQ-CONTENT-UNTRACKED
:satisfies: REQ-CONTENT-SYMLINKS

### `[content]`

| Key | Type | Required | Constraint |
|---|---|---:|---|
| `include` | array of strings | yes | non-empty unique glob sequence |
| `exclude` | array of strings | yes | unique glob sequence, may be empty |
| `respect_gitignore` | boolean | yes | no default |
| `follow_directory_symlinks` | boolean | yes | no default |
| `allow_internal_file_symlinks` | boolean | yes | no default |

Discovery evaluates paths relative to the project root using `/` as separator on
every platform. Matching is case-sensitive, including on Windows and macOS.
Supported glob tokens are:

- `*` for zero or more non-`/` characters;
- `?` for exactly one non-`/` character;
- `[abc]`, `[a-z]`, and `[!abc]` character classes;
- `**` for zero or more complete path segments only when it occupies a complete
  segment between `/`, start, or end.

Braces, backslash escaping, platform separators, and other glob extensions are
not supported. Literal metacharacters use a single-character class, such as
`[?]`. Dot-prefixed segments have no special exclusion rule.

A candidate must match at least one include, must not match an exclude, and—when
`respect_gitignore` is true and a containing Git worktree is available—must not
be ignored by Git. Exclude and Git-ignore decisions override includes. Outside a
Git worktree, `respect_gitignore` has no effect. Matching untracked files remain
eligible.

Directory symlinks are skipped when `follow_directory_symlinks` is false. When
true, Mara follows only targets whose fully resolved path remains inside the
fully resolved project root and detects directory cycles by filesystem identity.
File symlinks are accepted only when `allow_internal_file_symlinks` is true, the
resolved target is an internal regular file, and no other selected logical path
resolves to that file identity. Read diagnostics use the selected logical path.
v0.1 structured writes reject an affected symlinked source during edit preflight
rather than mutating through the link.

:::

:::design m_01KY82WX4HPJY45C49EZRAVXVZ
:id: DES-PROJECT-RUNTIME-CONFIG
:title: Project runtime, validation, Git, and path configuration
:status: accepted
:kind: interface
:satisfies: REQ-PROJECT-CONFIG-STRICT
:satisfies: REQ-PATH-CONTAINMENT

### `[index]`

| Key | Type | Required | Constraint |
|---|---|---:|---|
| `path` | path string | yes | writable derived-file location |

The path may not equal the project config, schema, or a selected content file.
Its parent must resolve inside the project root. The file need not exist before
`mara index`.

### `[validation]`

| Key | Type | Required | Meaning |
|---|---|---:|---|
| `warnings_as_errors` | boolean | yes | warnings make check status invalid |

This setting changes command status and exit code, not the diagnostic's declared
severity.

### `[git]`

| Key | Type | Required | Meaning |
|---|---|---:|---|
| `require_clean_worktree_for_writes` | boolean | yes | mutating commands require clean relevant paths unless explicitly overridden |

When outside Git, the setting does not prevent a write; source preimage and
transaction checks still apply. An explicit command-line dirty override changes
only this precondition and is recorded in the transaction journal.

## Path strings and containment

A path string is non-empty UTF-8 using `/`. It must be relative, contain no NUL,
backslash, drive prefix, URI scheme, empty segment, `.` segment, or `..` segment.
After joining to project root, Mara resolves every existing path component and
rejects a result outside the resolved root. For a not-yet-existing output, it
resolves the nearest existing ancestor and validates each remaining segment.

Containment is checked again immediately before I/O to reduce symlink-swap risk.
Opening existing project inputs shall use no-follow or equivalent handle-based
verification where the platform provides it. A path passing lexical checks but
failing filesystem containment is invalid.

:::

:::design m_01KY82WX4J6P2M1F7E5H4AEESR
:id: DES-PROJECT-INIT-OUTPUT
:title: Initial project configuration and empty schema output
:status: accepted
:kind: interface
:satisfies: REQ-PROJECT-INIT

## Initialization output

`mara init` writes all required v1 tables and keys explicitly. Its initial
content include is `**/*.mara.md`; exclude is empty; Git-ignore handling is
enabled; directory symlinks are disabled; internal file symlinks are enabled;
the index path is `.mara/index.json`; warning escalation is disabled; and clean
Git writes are required.

The generated schema is valid format v1 with configured ULID identity and empty
`flavours: {}`, `relations: {}`, and `rules: []`. An empty flavour map is valid
for a newly initialized process-neutral project; content items cannot validate
until the author adds at least one flavour.
:::
