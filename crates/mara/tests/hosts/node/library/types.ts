// The module of the `tsc` case of VER-NODE-LIBRARY (docs/hosts/verification.mara.md), which
// `types.test.mjs` checks with `tsc --strict --noEmit` where the library is installed: it calls
// every document operation, reads every member of its result, and passes each operation's
// argument and result types to `NoAny`. Each line marked `@ts-expect-error` must be an error.

import { createClient, MaraError, type Arguments, type OperationName, type Result } from "@convesoft/mara";

/** Whether `T` is `any`. */
type IsAny<T> = 0 extends 1 & T ? true : false;

/**
 * Whether `T`, or a member, array element or union member of it at any depth, is `any`. `any`
 * is checked before the depth: types that refer to themselves, as JSON values and proofs do, are
 * followed 24 levels deep, past a whole round of each of them, so an `any` anywhere in one is
 * met within that depth.
 */
type HasAny<T, Depth extends unknown[] = []> =
  IsAny<T> extends true
    ? true
    : Depth["length"] extends 24
      ? false
      : T extends readonly (infer Element)[]
        ? HasAny<Element, [...Depth, unknown]>
        : T extends object
          ? true extends { [K in keyof T]-?: HasAny<T[K], [...Depth, unknown]> }[keyof T]
            ? true
            : false
          : false;

/** Compiles only for a type that holds no `any` at any depth. */
type NoAny<T> = HasAny<T> extends false ? true : never;

/** Every operation's argument and result types, each without `any`. */
type Checked = { [N in OperationName]: [NoAny<Arguments<N>>, NoAny<Result<N>>] };
const checked: Checked = {
  project_validate: [true, true],
  project_init: [true, true],
  project_fix: [true, true],
  project_convert: [true, true],
  item_validate: [true, true],
  schema_validate: [true, true],
  search: [true, true],
  get: [true, true],
  relation_list: [true, true],
  item_list: [true, true],
  schema_list: [true, true],
  schema_get: [true, true],
  item_move: [true, true],
  item_rename: [true, true],
  item_create: [true, true],
  item_update: [true, true],
  item_delete: [true, true],
  relation_add: [true, true],
  relation_remove: [true, true],
  query_list: [true, true],
  query_execute: [true, true],
  query_view: [true, true],
  query_gate: [true, true],
  query_report: [true, true],
};

/** A fixture with `any` three members deep, which `NoAny` refuses. */
interface Fixture {
  outer: { middle: { inner: any } };
}
// @ts-expect-error: `any` three members deep
const refused: NoAny<Fixture> = true;

/** `any` 20 members deep, past the 12 levels that an earlier guard followed. */
type Nested<T, Depth extends unknown[] = []> = Depth["length"] extends 20
  ? T
  : { child: Nested<T, [...Depth, unknown]> };
// @ts-expect-error: `any` 20 members deep
const refusedDeep: NoAny<Nested<any>> = true;
const acceptedDeep: NoAny<Nested<string>> = true;

/** A type that refers to itself, as a proof does, with `any` in a union deep in each round. */
interface Tree {
  children: Tree[];
  value: string | { step: { atom: string; values: (string | any)[] } };
}
// @ts-expect-error: `any` inside a type that refers to itself
const refusedTree: NoAny<Tree> = true;

const client = createClient({ project: "." });

/** Reads every member of each result. */
async function everyOperation(): Promise<unknown[]> {
  const read: unknown[] = [checked, refused, refusedDeep, acceptedDeep, refusedTree];
  const validated = await client.run("project_validate", { path: ["docs"], limit: 10, cursor: "c" });
  read.push(validated.format_version, validated.valid, validated.errors, validated.warnings);
  read.push(validated.next_cursor);
  for (const diagnostic of validated.diagnostics) {
    read.push(diagnostic.code, diagnostic.severity, diagnostic.message, diagnostic.path);
    read.push(diagnostic.line, diagnostic.id, diagnostic.mid);
  }
  const initialized = await client.run("project_init", { template: "engineering" });
  read.push(initialized.format_version, initialized.created);
  const fixed = await client.run("project_fix");
  read.push(fixed.format_version, fixed.repairs_total);
  for (const repair of fixed.repairs) {
    read.push(repair.code, repair.path, repair.line, repair.message);
  }
  const converted = await client.run("project_convert", { status: "draft" });
  read.push(converted.format_version, converted.files);
  for (const entry of converted.entries) {
    read.push(entry.path, entry.line, entry.message);
  }
  const item = await client.run("item_validate", { "id-or-mid": "REQ-A", limit: 1 });
  read.push(item.format_version, item.valid, item.errors, item.warnings, item.diagnostics, item.next_cursor);
  const schema = await client.run("schema_validate", { limit: 1 });
  read.push(schema.format_version, schema.valid, schema.errors, schema.warnings, schema.diagnostics, schema.next_cursor);
  const found = await client.run("search", {
    query: "read",
    type: ["requirement"],
    field: ["status=draft"],
    relation: ["satisfies"],
    path: ["docs"],
    id: ["REQ-A"],
    limit: 5,
  });
  read.push(found.format_version, found.diagnostics, found.diagnostics_total, found.next_cursor);
  for (const result of found.results) {
    read.push(result.kind, result.path, result.excerpt.line, result.excerpt.text);
    if (result.kind === "item") {
      read.push(result.line, result.id, result.mid, result.type, result.title);
    } else if (result.kind === "narrative") {
      read.push(result.line, result.heading);
    } else if (result.kind === "code") {
      read.push(result.line, result.declaration);
    }
  }
  const got = await client.run("get", { node: "REQ-A", cursor: "c" });
  read.push(got.format_version, got.node.kind, got.node.path, got.content);
  read.push(got.diagnostics, got.diagnostics_total, got.next_cursor);
  if (got.node.kind === "item") {
    const revision: string = got.node.revision;
    read.push(revision, got.node.mid);
  }
  // A parameter set to `null` counts as left out.
  const unpaged = await client.run("item_list", { cursor: null, limit: null, type: null });
  read.push(unpaged.items);
  // `debugStats` known only when the call runs.
  const flag: boolean = read.length > 1;
  const either = await client.run("search", { query: "read" }, { debugStats: flag });
  read.push("stats" in either ? either.stats.hashes : either.results);
  for (const row of got.rows) {
    read.push(row.key, row.value, row.line, row.partial);
  }
  const related = await client.run("relation_list", { node: "REQ-A", relation: ["satisfies"], limit: 2 });
  read.push(related.format_version, related.node, related.diagnostics, related.diagnostics_total, related.next_cursor);
  for (const connection of related.connections) {
    read.push(connection.kind, connection.relation, connection.direction);
    read.push(connection.neighbour.kind, connection.source.path, connection.source.line);
    if (connection.neighbour.kind === "unresolved") {
      read.push(connection.neighbour.mid);
    }
  }
  const listed = await client.run("item_list", { type: ["need"], limit: 1 });
  read.push(listed.format_version, listed.diagnostics, listed.diagnostics_total, listed.next_cursor);
  for (const one of listed.items) {
    read.push(one.kind, one.path, one.line, one.id, one.mid, one.type, one.title);
  }
  const declarations = await client.run("schema_list", { limit: 3 });
  read.push(declarations.format_version, declarations.diagnostics, declarations.diagnostics_total, declarations.next_cursor);
  for (const declaration of declarations.declarations) {
    read.push(declaration.kind, declaration.name, declaration.description);
    read.push(declaration.kind === "type" ? declaration.id_prefix : declaration.inverse);
  }
  const declared = await client.run("schema_get", { name: "requirement" });
  read.push(declared.format_version, declared.diagnostics, declared.diagnostics_total, declared.next_cursor);
  for (const declaration of declared.declarations) {
    if (declaration.kind === "type") {
      read.push(declaration.id_prefix, declaration.statuses);
      for (const field of declaration.fields) {
        read.push(field.name, field.kind, field.values, field.required, field.repeatable);
      }
    } else {
      read.push(declaration.inverse, declaration.targets, declaration.same_type, declaration.acyclic);
    }
  }
  const moved = await client.run("item_move", { "id-or-mid": "REQ-A", path: "docs/b.mara.md", line: 1 });
  read.push(moved.format_version, moved.item, moved.files, moved.from.path, moved.from.line, moved.diagnostics);
  const renamed = await client.run("item_rename", { "id-or-mid": "REQ-A", "new-id": "REQ-B" });
  read.push(renamed.format_version, renamed.item, renamed.files, renamed.old_id, renamed.diagnostics);
  const created = await client.run("item_create", {
    type: "requirement",
    path: "docs/a.mara.md",
    title: "Cold read",
    body: "A cold read takes at most 200 ms.",
    id: "REQ-COLD",
    field: ["status=draft"],
    line: 1,
  });
  read.push(created.format_version, created.item.mid, created.files);
  const updated = await client.run("item_update", {
    item: "REQ-COLD",
    title: "Cold reads",
    body: "Faster.",
    field: ["status=accepted"],
    "clear-field": ["owner"],
  });
  read.push(updated.format_version, updated.item, updated.files);
  const deleted = await client.run("item_delete", { item: "REQ-COLD" });
  read.push(deleted.format_version, deleted.item, deleted.files);
  const added = await client.run("relation_add", { source: "REQ-B", relation: "satisfies", target: "NEED-A" });
  read.push(added.format_version, added.item, added.files, added.relation, added.target.mid, added.line);
  const removed = await client.run("relation_remove", { source: "REQ-B", relation: "satisfies", target: "NEED-A" });
  read.push(removed.format_version, removed.item, removed.files, removed.relation, removed.target, removed.line);
  const entries = await client.run("query_list", { limit: 5 });
  read.push(entries.format_version, entries.diagnostics, entries.diagnostics_total, entries.next_cursor);
  for (const entry of entries.entries) {
    read.push(entry.name, entry.kind, entry.description, entry.inputs);
  }
  for (const name of ["query_execute", "query_view"] as const) {
    const executed = await client.run(name, {
      name: "listed",
      id: ["REQ-B"],
      input: { candidate: "abc", verification_result: [["m", "pass", 1]] },
      complete: ["verification_result"],
      limit: 10,
    });
    read.push(executed.format_version, executed.diagnostics, executed.diagnostics_total, executed.next_cursor);
    read.push(executed.query.name, executed.query.kind, executed.query.relation);
    for (const column of executed.query.columns) {
      read.push(column.name, column.type);
    }
    for (const row of executed.rows) {
      read.push(row.item?.mid, row.values);
    }
  }
  const gated = await client.run("query_gate", { name: "verified", id: ["REQ-B"], input: { candidate: "abc" } });
  read.push(gated.format_version, gated.diagnostics, gated.diagnostics_total, gated.next_cursor);
  read.push(gated.gate.name, gated.gate.outcome);
  for (const judged of gated.items) {
    read.push(judged.item, judged.outcome);
    for (const row of judged.rows) {
      read.push(row.relation, row.role, row.values);
    }
  }
  const reported = await client.run("query_report", { name: "verified", id: "REQ-B" });
  read.push(reported.format_version, reported.diagnostics, reported.diagnostics_total, reported.next_cursor);
  read.push(reported.report.name, reported.report.kind, reported.report.item, reported.report.outcome);
  for (const row of reported.rows) {
    read.push(row.relation, row.role, row.values);
    read.push("proof" in row ? row.proof : row.unexplained);
  }
  const withStats = await client.run("item_list", {}, { debugStats: true });
  read.push(withStats.result.items, withStats.stats.format_version, withStats.stats.file_reads);
  read.push(withStats.stats.hashes, withStats.stats.parses, withStats.stats.snapshot_builds);
  read.push(withStats.stats.code_scans, withStats.stats.subprocess_spawns, withStats.stats.check_steps);
  read.push(withStats.stats.program_runs);
  try {
    await client.run("item_list");
  } catch (error) {
    if (error instanceof MaraError) {
      read.push(error.code, error.message, error.diagnostics, error.reply.error.code, error.stats?.hashes);
    }
  }
  return read;
}

/** The lines that must not compile. */
async function errors(): Promise<void> {
  // @ts-expect-error: an operation that does not exist
  await client.run("item_nope", {});
  // @ts-expect-error: an argument of the wrong type
  await client.run("item_list", { limit: "1" });
  // @ts-expect-error: a required argument left out
  await client.run("get", {});
  // @ts-expect-error: the arguments of an operation that needs one left out
  await client.run("item_create");
  const listed = await client.run("item_list");
  // @ts-expect-error: a result member that the operation does not return
  listed.results;
}

export { everyOperation, errors };
