// The document operations of every Mara client (DES-NODE-LIBRARY in `docs/hosts/index.mara.md`):
// each operation's MCP tool name, its arguments as the tool takes them (DES-MCP-SERVER) and its
// result as `--format json` prints it (DES-OPERATIONS, DES-READ-RESULTS, DES-AUTHORING-REQUESTS,
// DES-QUERY-RESULTS, DES-DIAGNOSTICS, DES-FIX-REPAIRS). No type holds `any`. The Node library
// and the browser package share this file.

/** A JSON value, such as a field's value or a value of a supplied input's row. */
export type Json = string | number | boolean | null | Json[] | { [member: string]: Json };

/** An item, as a result names it. `id` is null for an item without an `ID` row. */
export interface ItemNode {
  kind: "item";
  path: string;
  line: number;
  id: string | null;
  mid: string;
  type: string;
  title: string;
}

/** A narrative unit: the text under a heading, or before the first heading. */
export interface NarrativeNode {
  kind: "narrative";
  path: string;
  line: number;
  heading: string | null;
}

/** A whole document. */
export interface DocumentNode {
  kind: "document";
  path: string;
}

/** A code location, or a whole code file with `line` null; `declaration` is null at the basic level. */
export interface CodeNode {
  kind: "code";
  path: string;
  line: number | null;
  declaration: string | null;
}

/** A node that a read names (DES-READ-RESULTS). */
export type Node = ItemNode | NarrativeNode | DocumentNode | CodeNode;

/** A problem in the project's files (DES-DIAGNOSTICS); `id` and `mid` are null outside an item. */
export interface Diagnostic {
  code: string;
  severity: "error" | "warning";
  message: string;
  path: string;
  line: number;
  id: string | null;
  mid: string | null;
}

/** A place in a file. */
export interface Location {
  path: string;
  line: number;
}

/** The members of every page of a read (DES-READ-RESULTS). */
export interface Page {
  format_version: 1;
  diagnostics: Diagnostic[];
  diagnostics_total: number;
  /** The cursor of the next page; null on the last. */
  next_cursor: string | null;
}

/** The result of `project validate`, `item validate` and `schema validate` (DES-DIAGNOSTICS). */
export interface ValidationResult {
  format_version: 1;
  valid: boolean;
  errors: number;
  warnings: number;
  diagnostics: Diagnostic[];
  next_cursor: string | null;
}

/** A search result: its node, with the line that matched. */
export type SearchResult = Node & { excerpt: { line: number; text: string } };

/** A metadata row of a node that `get` returns. */
export interface Row {
  key: string;
  value: string;
  line: number;
  /** Whether this page holds only part of the value. */
  partial: boolean;
}

/** A relation or mention of a node (DES-READ-RESULTS). */
export interface Connection {
  kind: "relation" | "mention";
  /** The relation's name as shown; null for a mention. */
  relation: string | null;
  direction: "outgoing" | "incoming";
  neighbour: Node | { kind: "unresolved"; mid: string };
  source: Location;
}

/** A type, as `schema list` lists it. */
export interface TypeSummary {
  kind: "type";
  name: string;
  id_prefix: string | null;
  description: string;
}

/** A relation, as `schema list` lists it. */
export interface RelationSummary {
  kind: "relation";
  name: string;
  inverse: string | null;
  description: string;
}

/** A field of a type, as loaded. */
export interface Field {
  name: string;
  kind: "text" | "enum";
  /** The allowed values of an enum; null for text. */
  values: string[] | null;
  required: boolean;
  repeatable: boolean;
}

/** A type, as `schema get` returns it. */
export interface TypeDeclaration extends TypeSummary {
  statuses: string[];
  fields: Field[];
}

/** A relation, as `schema get` returns it. */
export interface RelationDeclaration extends RelationSummary {
  /** The target types of each source type. */
  targets: { [source: string]: string[] };
  same_type: boolean;
  acyclic: boolean;
}

/** A row's values: symbols as strings, numbers as numbers. */
export type Values = (string | number)[];

/** A step of a proof (DES-QUERY-RESULTS). */
export type ProofStep =
  | { relation: string; atom: string; rule: string; children: ProofStep[] }
  | { relation: string; values: Values; path: string; line: number }
  | { input: string; values: Values }
  | { absent: string };

/** The role of a gate's row. */
export type Role = "subject" | "condition" | "unavailable";

/** A row of a gate's item. */
export interface GateRow {
  relation: string;
  role: Role;
  values: Values;
}

/** A row of a report: explained by `proof`, or not, for the reason `unexplained`. */
export type ReportRow = { relation: string; role?: Role; values: Values } & (
  | { proof: ProofStep }
  | { unexplained: string }
);

/** A query or view, with its columns. */
export interface QueryEntry {
  name: string;
  kind: "query" | "view";
  relation: string;
  columns: { name: string; type: "symbol" | "number" }[];
}

/** The result of `query execute` and `query view`. */
export interface QueryResult extends Page {
  query: QueryEntry;
  rows: { item: ItemNode | null; values: Values }[];
}

/** The result of an edit of an item (DES-AUTHORING-REQUESTS). */
export interface ItemWritten {
  format_version: 1;
  item: ItemNode;
  files: string[];
}

/** The result of `relation add` and `relation remove`. */
export interface RelationWritten extends ItemWritten {
  relation: string;
  target: ItemNode;
  line: number;
}

/** The inputs of a `query` request: a value as a string, or rows (DES-REQUEST-INPUTS). */
export type Inputs = { [input: string]: string | Json[][] };

/** The options of a paged read. */
interface Paged {
  limit?: number;
  cursor?: string;
}

/** The filters of `search` and `item list` (DES-SEARCH-REQUESTS). */
interface Filters extends Paged {
  type?: string[];
  field?: string[];
  relation?: string[];
  path?: string[];
  id?: string[];
}

/** The options of a request that evaluates a program. */
interface Evaluated extends Paged {
  input?: Inputs;
  complete?: string[];
}

/** Each document operation by its MCP tool name: its arguments and its result. */
export interface Operations {
  project_validate: {
    args: Paged & { path?: string[] };
    result: ValidationResult;
  };
  project_init: {
    args: { template?: string };
    result: { format_version: 1; created: string[] };
  };
  project_fix: {
    args: Record<string, never>;
    result: {
      format_version: 1;
      repairs: { code: string; path: string; line: number; message: string }[];
      repairs_total: number;
    };
  };
  project_convert: {
    args: { status?: string };
    result: {
      format_version: 1;
      files: string[];
      entries: { path: string; line: number; message: string }[];
    };
  };
  item_validate: {
    args: Paged & { "id-or-mid": string };
    result: ValidationResult;
  };
  schema_validate: {
    args: Paged;
    result: ValidationResult;
  };
  search: {
    args: Filters & { query: string };
    result: Page & { results: SearchResult[] };
  };
  get: {
    args: { node: string; cursor?: string };
    result: Page & {
      /** The node, an item with its `revision`: the BLAKE3 hash of its region. */
      node: NarrativeNode | DocumentNode | CodeNode | (ItemNode & { revision: string });
      rows: Row[];
      content: string;
    };
  };
  relation_list: {
    args: Paged & { node: string; relation?: string[] };
    result: Page & { node: Node; connections: Connection[] };
  };
  item_list: {
    args: Filters;
    result: Page & { items: ItemNode[] };
  };
  schema_list: {
    args: Paged;
    result: Page & { declarations: (TypeSummary | RelationSummary)[] };
  };
  schema_get: {
    args: Paged & { name: string };
    result: Page & { declarations: (TypeDeclaration | RelationDeclaration)[] };
  };
  item_move: {
    args: { "id-or-mid": string; path: string; line?: number };
    result: ItemWritten & { from: Location; diagnostics: Diagnostic[] };
  };
  item_rename: {
    args: { "id-or-mid": string; "new-id": string };
    result: ItemWritten & { old_id: string | null; diagnostics: Diagnostic[] };
  };
  item_create: {
    args: {
      type: string;
      path: string;
      title: string;
      body: string;
      id?: string;
      field?: string[];
      line?: number;
    };
    result: ItemWritten;
  };
  item_update: {
    args: {
      item: string;
      title?: string;
      body?: string;
      field?: string[];
      "clear-field"?: string[];
    };
    result: ItemWritten;
  };
  item_delete: {
    args: { item: string };
    result: ItemWritten;
  };
  relation_add: {
    args: { source: string; relation: string; target: string };
    result: RelationWritten;
  };
  relation_remove: {
    args: { source: string; relation: string; target: string };
    result: RelationWritten;
  };
  query_list: {
    args: Paged;
    result: Page & {
      entries: {
        name: string;
        kind: "query" | "view" | "invariant" | "gate";
        description: string;
        inputs: string[];
      }[];
    };
  };
  query_execute: {
    args: Evaluated & { name: string; id?: string[] };
    result: QueryResult;
  };
  query_view: {
    args: Evaluated & { name: string; id?: string[] };
    result: QueryResult;
  };
  query_gate: {
    args: Evaluated & { name: string; id: string[] };
    result: Page & {
      gate: { name: string; outcome: "passed" | "failed" | "unavailable" };
      items: {
        item: ItemNode;
        outcome: "passed" | "failed" | "unavailable" | "not-applicable";
        rows: GateRow[];
      }[];
    };
  };
  query_report: {
    args: Evaluated & { name: string; id: string };
    result: Page & {
      report: {
        name: string;
        kind: "invariant" | "gate";
        item: ItemNode;
        outcome: "passed" | "failed" | "unavailable" | "not-applicable";
      };
      rows: ReportRow[];
    };
  };
}

/** The MCP tool name of each document operation. */
export type OperationName = keyof Operations;

/** `T` whose optional members may also be `null`, which counts as left out (DES-MCP-SERVER). */
export type LeftOut<T> = { [K in keyof T]: undefined extends T[K] ? T[K] | null : T[K] };

/** The arguments of the operation `N`; a member set to `null` counts as left out. */
export type Arguments<N extends OperationName> = LeftOut<Operations[N]["args"]>;

/** The result of the operation `N`: the JSON object that `--format json` prints. */
export type Result<N extends OperationName> = Operations[N]["result"];

/** The JSON object that `--format json` prints for a failure. */
export interface FailureReply {
  format_version: 1;
  error: { code: string; message: string; diagnostics: Diagnostic[] };
}

/** The work of one call, as the stats line of `--debug-stats` holds it (DES-PERFORMANCE-HARNESS). */
export interface Stats {
  format_version: 1;
  file_reads: number;
  hashes: number;
  parses: number;
  snapshot_builds: number;
  code_scans: number;
  subprocess_spawns: number;
  check_steps: number;
  program_runs: number;
}

/** The options of a call. */
export interface CallOptions {
  /** Resolve with `{ result, stats }`: the call's work beside its result, never inside it. */
  debugStats?: boolean;
}

/**
 * The parameters of `run` after the name: the arguments, which may be left out only when the
 * operation needs none, then the options `O`.
 */
export type RunParameters<N extends OperationName, O> =
  {} extends Arguments<N> ? [args?: Arguments<N>, options?: O] : [args: Arguments<N>, options?: O];

/** The parameters of `run` after the name for a call with debug stats. */
export type RunParametersWithStats<N extends OperationName> =
  {} extends Arguments<N>
    ? [args: Arguments<N> | undefined, options: CallOptions & { debugStats: true }]
    : [args: Arguments<N>, options: CallOptions & { debugStats: true }];

/**
 * A client of one project: its embedded host or the daemon in Node, the daemon's listener or a
 * workspace in a browser. `run` rejects with a `MaraError` when the request fails.
 */
export interface Client {
  run<N extends OperationName>(
    name: N,
    ...rest: RunParameters<N, CallOptions & { debugStats?: false }>
  ): Promise<Result<N>>;
  run<N extends OperationName>(
    name: N,
    ...rest: RunParametersWithStats<N>
  ): Promise<{ result: Result<N>; stats: Stats }>;
  /** A call whose `debugStats` is known only when it runs. */
  run<N extends OperationName>(
    name: N,
    ...rest: RunParameters<N, CallOptions>
  ): Promise<Result<N> | { result: Result<N>; stats: Stats }>;
}
