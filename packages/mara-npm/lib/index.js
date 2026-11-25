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

/** A refusal of the caller's arguments: `invalid-argument`, nothing run. */
function invalidArgument(message) {
  return new MaraError({
    format_version: 1,
    error: { code: "invalid-argument", message, diagnostics: [] },
  });
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
    if (this.#refused !== undefined) {
      throw invalidArgument(this.#refused);
    }
    let text;
    try {
      text = JSON.stringify(args ?? null);
    } catch (error) {
      throw invalidArgument(`the arguments are no JSON: ${error.message}`);
    }
    const answer = JSON.parse(
      await addon().call(String(name), text ?? "null", this.#project, this.#noDaemon),
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
 * one found from the working directory of each call upward. With `options.noDaemon` every call
 * runs in this process, as with the CLI's `--no-daemon`.
 */
export function createClient(options) {
  return new Client(options);
}
