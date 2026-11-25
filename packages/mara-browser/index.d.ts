// The types of a browser workspace (DES-BROWSER-WORKSPACE). Operation names, arguments and
// results are those of the MCP tools and of `--format json` (DES-OPERATIONS, DES-MCP-SERVER).

/** The MCP tool name of each document operation. */
export type OperationName =
  | "project_validate"
  | "project_init"
  | "project_fix"
  | "project_convert"
  | "item_validate"
  | "schema_validate"
  | "search"
  | "get"
  | "relation_list"
  | "item_list"
  | "schema_list"
  | "schema_get"
  | "item_move"
  | "item_rename"
  | "item_create"
  | "item_update"
  | "item_delete"
  | "relation_add"
  | "relation_remove"
  | "query_list"
  | "query_execute"
  | "query_view"
  | "query_gate"
  | "query_report";

/** An operation's arguments, as its MCP tool takes them; `null` stands for one left out. */
export interface Arguments {
  [name: string]:
    | string
    | number
    | string[]
    | number[]
    | { [input: string]: string | unknown[][] }
    | null
    | undefined;
}

/** A diagnostic of a result or failure (DES-DIAGNOSTICS). */
export interface Diagnostic {
  [member: string]: unknown;
}

/** The JSON object that `--format json` prints for a result. */
export interface Result {
  format_version: 1;
  [member: string]: unknown;
}

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

/** A file supplied to a workspace: its path relative to the project root, and its bytes. */
export interface SuppliedFile {
  path: string;
  contents: Uint8Array | ArrayBuffer;
}

/** A file whose final contents differ from its supplied contents. */
export interface Change {
  path: string;
  /** The final bytes; `null` for a removed file. */
  contents: Uint8Array | null;
  /** The lowercase hexadecimal BLAKE3 hash of the supplied bytes; `null` for a file not supplied. */
  base: string | null;
}

/** A failed request. `stats` is set when the call asked for debug stats. */
export declare class MaraError extends Error {
  readonly code: string;
  readonly diagnostics: Diagnostic[];
  readonly reply: FailureReply;
  readonly stats?: Stats;
}

export declare class Workspace {
  run(name: OperationName, args?: Arguments, options?: { debugStats?: false }): Promise<Result>;
  run(
    name: OperationName,
    args: Arguments | undefined,
    options: { debugStats: true },
  ): Promise<{ result: Result; stats: Stats }>;
  write(path: string, contents: Uint8Array | ArrayBuffer): Promise<void>;
  remove(path: string): Promise<void>;
  export(): Promise<Change[]>;
  close(): void;
  static create(files: Iterable<SuppliedFile>): Promise<Workspace>;
}

/** Starts a workspace of `files` in a dedicated worker of its own. */
export declare function createWorkspace(files: Iterable<SuppliedFile>): Promise<Workspace>;
