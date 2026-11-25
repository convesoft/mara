// The Node library of `@convesoft/mara` (REQ-NODE-LIBRARY, DES-NODE-LIBRARY): the document
// operations of one project, run by the native addon of the installed native package, embedded in
// this process or through the daemon of that package's `mara`, without starting the CLI. Every
// call is asynchronous and throws nothing: a failure rejects its promise.

import { createRequire } from "node:module";
import path from "node:path";

const require = createRequire(import.meta.url);

/** A failed request, with the failure object that `--format json` prints. */
export class MaraError extends Error {
  constructor(reply, stats) {
    super(reply.error.message);
    this.name = "MaraError";
    this.code = reply.error.code;
    this.diagnostics = reply.error.diagnostics;
    this.reply = reply;
    if (stats !== undefined) {
      this.stats = stats;
    }
  }
}

/** The work of a call that ran nothing, as its stats line would hold it (DES-PERFORMANCE-HARNESS). */
const NO_WORK = {
  format_version: 1,
  file_reads: 0,
  hashes: 0,
  parses: 0,
  snapshot_builds: 0,
  code_scans: 0,
  subprocess_spawns: 0,
  check_steps: 0,
  program_runs: 0,
};

/**
 * A refusal of the caller's arguments: `invalid-argument`, nothing run, with no work when the
 * call asked for debug stats.
 */
function invalidArgument(message, stats) {
  const reply = {
    format_version: 1,
    error: { code: "invalid-argument", message, diagnostics: [] },
  };
  return new MaraError(reply, stats ? { ...NO_WORK } : undefined);
}

/**
 * Whether this thread's environment asks for direct mode, as the CLI reads its own: `CI` set to
 * a non-empty value or `MARA_NO_DAEMON` set to `1`. A worker thread has an environment of its
 * own, which the addon cannot see, so the library reads it here.
 */
function directByEnvironment() {
  return Boolean(process.env.CI) || process.env.MARA_NO_DAEMON === "1";
}

/** The addon, or the error that keeps it from loading, once per process. */
let loaded;

/**
 * The addon `bin/mara.node` of the native package that the dispatcher runs `bin/mara` of: the
 * package for this platform installed for this one at its version. Throws why it cannot load.
 */
function addon() {
  if (loaded === undefined) {
    try {
      const { nativePackage } = require("../bin/mara.cjs");
      const module = { exports: {} };
      process.dlopen(module, path.join(nativePackage(), "bin", "mara.node"));
      loaded = { addon: module.exports };
    } catch (error) {
      loaded = { error };
    }
  }
  if (loaded.error !== undefined) {
    throw loaded.error;
  }
  return loaded.addon;
}

export class Client {
  #project;
  #noDaemon;
  #refused;

  constructor(options = {}) {
    const { project, noDaemon } = options ?? {};
    if (project !== undefined && typeof project !== "string") {
      this.#refused = "`project` is no string";
    } else if (noDaemon !== undefined && typeof noDaemon !== "boolean") {
      this.#refused = "`noDaemon` is no boolean";
    }
    this.#project = typeof project === "string" ? path.resolve(project) : null;
    this.#noDaemon = noDaemon === true;
  }

  /**
   * Runs the document operation whose MCP tool is `name` with the tool's `args`. Resolves with
   * its result, or with `{ result, stats }` when `options.debugStats` is true.
   */
  async run(name, args = {}, options = {}) {
    const stats = options?.debugStats === true;
    // Taken when the call is made, before the first `await`: a later change of the working
    // directory or the environment does not reach a call already made.
    const cwd = process.cwd();
    const noDaemon = this.#noDaemon || directByEnvironment();
    if (this.#refused !== undefined) {
      throw invalidArgument(this.#refused, stats);
    }
    let text;
    try {
      text = JSON.stringify(args ?? null);
    } catch (error) {
      throw invalidArgument(`the arguments are no JSON: ${error.message}`, stats);
    }
    const answer = JSON.parse(
      await addon().call(String(name), text ?? "null", this.#project, cwd, noDaemon),
    );
    const reply = JSON.parse(answer.text);
    if (answer.failed) {
      throw new MaraError(reply, stats ? answer.stats : undefined);
    }
    return stats ? { result: reply, stats: answer.stats } : reply;
  }
}

/**
 * A client of the project `options.project`, relative to the working directory now, or of the
 * one found upward from the working directory when each call is made. With `options.noDaemon`,
 * or `CI` or `MARA_NO_DAEMON=1` in `process.env`, every call runs in this process, as with the
 * CLI's `--no-daemon`.
 */
export function createClient(options) {
  return new Client(options);
}
