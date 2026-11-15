# Project initialization

## Design

---

<a id="01M7FVWHVXW71CWC0HJ52NTEQJ"></a>

### Write a template into a directory

| Type | Design |
| --- | --- |
| ID | DES-PROJECT-INIT |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-INIT](project.mara.md#01M7BF6H8Z02SNKXD2XPGVD87M) |

- `project init [--template engineering|empty]` initializes the working directory, or the directory that `--project` names. That directory and each missing directory above it are created, and symbolic links in its path are resolved, as selection resolves a root.
- The templates are embedded in the binary. Each writes its files in this order:

| Template | Files |
| --- | --- |
| `engineering` | `.mara/schema.toml`, the vocabulary of [DES-ENGINEERING-VOCABULARY](vocabulary.mara.md#01M7BF6HH8CA2B9TQJ9DT32WDG) as the same text as this repository's `.mara/schema.toml`; programs under `.mara/rules/` from 0.4.0-alpha.3 (mara-tx0fcf); `.mara/config.toml` |
| `empty` | `.mara/schema.toml`, with the statuses `draft`, `accepted` and `retired` and no type or relation; `.mara/config.toml` |

- The configuration is the example of [DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27). It comes last: until it exists, no operation selects the directory as a project.
- Before it creates anything, `project init` refuses a 0.3.0 project with `legacy-project`, a `.mara` that is a symbolic link with `symbolic-link`, and a file it would write that exists, whatever it is, or a `.mara` that is no directory, with `existing-file` ([DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS)). The message and a diagnostic at line 1 name each such file.
- The write procedure writes the files as files the edit creates ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)). A new project has no `.mara` to lock: `project init` creates `.mara` just before step 1, then takes the lock on it.
- A failure before the first rename removes each directory that `project init` created and that is empty again, the last created first. A directory that holds another program's file stays.
- A stop after the first rename leaves no configuration, so no project. A rerun refuses the files already written until they are removed.
- The result is `created`: each file written, relative to the project root, in the order written. The human form prints `created <path>` for each.
