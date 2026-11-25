// The types of the Node library (DES-NODE-LIBRARY): a client of one project, whose operations
// are typed in `operations.d.ts`.

import type { Client as ClientInterface, Diagnostic, FailureReply, Stats } from "./operations.js";

export type * from "./operations.js";

/** A failed request. `stats` is set when the call asked for debug stats. */
export declare class MaraError extends Error {
  readonly code: string;
  readonly diagnostics: Diagnostic[];
  readonly reply: FailureReply;
  readonly stats?: Stats;
}

/** The options of a client. */
export interface ClientOptions {
  /** The project root, relative to the working directory when the client is created. */
  project?: string;
  /** Run every call in this process, neither reaching a daemon nor starting one. */
  noDaemon?: boolean;
}

export declare class Client implements ClientInterface {
  constructor(options?: ClientOptions);
  run: ClientInterface["run"];
}

/**
 * A client of the project `options.project`, or of the one found from the working directory of
 * each call upward.
 */
export declare function createClient(options?: ClientOptions): Client;
