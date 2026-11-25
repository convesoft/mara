// The types of the browser package (DES-BROWSER-WORKSPACE, DES-NODE-LIBRARY): a workspace and a
// client of the daemon's listener, each a client whose operations are typed in
// `operations.d.ts`, which `nx run mara-browser:build` copies from the Node library.

import type { Client, Diagnostic, FailureReply, Stats } from "./operations.js";

export type * from "./operations.js";

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

export declare class Workspace implements Client {
  /** `project_init` and `project_convert` fail with `unavailable-operation`. */
  run: Client["run"];
  write(path: string, contents: Uint8Array | ArrayBuffer): Promise<void>;
  remove(path: string): Promise<void>;
  export(): Promise<Change[]>;
  close(): void;
  static create(files: Iterable<SuppliedFile>): Promise<Workspace>;
}

/** Starts a workspace of `files` in a dedicated worker of its own. */
export declare function createWorkspace(files: Iterable<SuppliedFile>): Promise<Workspace>;

/** The options of a client of the daemon's listener. */
export interface DaemonOptions {
  /** The listener's origin, such as `http://127.0.0.1:<port>` with the port that `mara daemon start --http` printed. */
  url: string;
  /** The token that `mara daemon start --http` printed. */
  token: string;
  /** The project root, an absolute path that the daemon allows. */
  project: string;
}

/** A client of one project through the daemon's listener. */
export declare class DaemonClient implements Client {
  constructor(options: DaemonOptions);
  /** `project_init` and `project_convert` fail with `unavailable-operation`. */
  run: Client["run"];
}

/** A client of `options.project` through the daemon's listener. */
export declare function connectDaemon(options: DaemonOptions): DaemonClient;
