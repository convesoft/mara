// The programs of the rules Verification items in the setup of their tests (`tests/programs`,
// VER-PROGRAM-CONSUMERS), for the browser cases: the configuration of the fixture `programs`
// with entries, the consumers of one relation, and the caller's writes that declare a program.

/** The configuration line of `[rules] programs`, where a fault without a line is reported. */
export const PROGRAMS_LINE = 9;

/** The configuration of the fixture `programs` followed by `entries`. */
export function config(entries = "", timeLimit = null) {
  const limit = timeLimit === null ? "" : `time_limit_seconds = ${timeLimit}\n`;
  return (
    'format_version = 1\nschema = ".mara/schema.toml"\n\n[documents]\nformat_version = 1\n' +
    'include = ["docs/**/*.mara.md"]\n\n[rules]\nprograms = [".mara/rules/**/*.dl"]\n' +
    `${limit}${entries}`
  );
}

/** An invariant entry of `severity` on `relation` of `program`, a path below `.mara/rules/`. */
export function invariant(name, program, relation, severity) {
  return (
    `\n[invariants.${name}]\nprogram = ".mara/rules/${program}"\nrelation = "${relation}"\n` +
    `severity = "${severity}"\nmessage = "${name} failed"\n`
  );
}

/**
 * An error and a warning invariant on `relation` of `program`, the query `as-query` and the view
 * `as-view` on it, and the gate `as-gate` whose subject and condition it is.
 */
export function bothSeverities(program, relation) {
  return (
    invariant("as-error", program, relation, "error") +
    invariant("as-warning", program, relation, "warning") +
    `\n[queries.as-query]\nprogram = ".mara/rules/${program}"\nrelation = "${relation}"\n` +
    `description = "d"\n\n[views.as-view]\nprogram = ".mara/rules/${program}"\n` +
    `relation = "${relation}"\ndescription = "d"\n\n[gates.as-gate]\n` +
    `program = ".mara/rules/${program}"\ndescription = "d"\nsubjects = ["${relation}"]\n` +
    `conditions = ["${relation}"]\n`
  );
}

/** `bad(m)` for each accepted item that meets `condition`, which starts with a comma. */
export function admitted(condition) {
  return `.include "mara.dl"\n.decl bad(m: Mid)\nbad(m) :- item(m, _, _, "accepted", _, _)${condition}.\n`;
}

/** The `n`th MID of the fixtures `programs` and `facts`. */
export function mid(n) {
  return `01M7D8DZ2663WP3A03000000${String(n).padStart(2, "0")}`;
}

/** Writes `text` as `.mara/rules/<name>` and the configuration with `entries` in `workspace`. */
export async function declare(workspace, text, entries, { name = "p.dl", timeLimit = null } = {}) {
  await workspace.write(`.mara/rules/${name}`, text);
  await workspace.write(".mara/config.toml", config(entries, timeLimit));
}
