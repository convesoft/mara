// Mara in the browser: a workspace (REQ-BROWSER-LOCAL-WORKSPACE, DES-BROWSER-WORKSPACE), the
// document operations on supplied files, run by Mara's WebAssembly module in a dedicated worker,
// with the change set of what they and the caller changed; and a client of the daemon's listener
// (`daemon.js`, DES-NODE-LIBRARY). Every method is asynchronous and throws nothing: a failure
// rejects its promise.

import { MaraError } from "./errors.js";

export { MaraError };
export { DaemonClient, connectDaemon } from "./daemon.js";

/** A refusal of the caller's arguments, as the module refuses them: `invalid-argument`. */
function invalidArgument(message) {
  return new MaraError({
    format_version: 1,
    error: { code: "invalid-argument", message, diagnostics: [] },
  });
}

/**
 * A copy of `contents`, which the caller keeps as it was. A `Uint8Array` or `ArrayBuffer` is
 * told by its kind, so that one from another realm, such as a same-origin frame, is taken too.
 */
function bytes(contents, what) {
  const kind = Object.prototype.toString.call(contents);
  if (kind === "[object Uint8Array]" || kind === "[object ArrayBuffer]") {
    const view = kind === "[object ArrayBuffer]" ? new Uint8Array(contents) : contents;
    const copy = new Uint8Array(view.length);
    copy.set(view);
    return copy;
  }
  throw invalidArgument(`${what} is no Uint8Array or ArrayBuffer`);
}

export class Workspace {
  #worker;
  #next = 0;
  #pending = new Map();
  #closed;

  constructor(worker) {
    this.#worker = worker;
    worker.onmessage = ({ data }) => {
      const pending = this.#pending.get(data.id);
      this.#pending.delete(data.id);
      if (data.error !== undefined) {
        pending.reject(new Error(data.error));
      } else {
        pending.resolve(data.answer);
      }
    };
    worker.onerror = (event) => {
      event.preventDefault();
      this.#stop(new Error(`Mara's worker failed: ${event.message}`));
    };
  }

  #stop(error) {
    this.#closed ??= error;
    for (const pending of this.#pending.values()) {
      pending.reject(error);
    }
    this.#pending.clear();
  }

  async #call(message, stats = false) {
    if (this.#closed) {
      throw this.#closed;
    }
    const id = this.#next++;
    const answer = await new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      try {
        this.#worker.postMessage({ id, ...message });
      } catch (error) {
        // An argument that cannot be sent, such as a function.
        this.#pending.delete(id);
        reject(invalidArgument(`the call cannot be sent: ${error.message}`));
      }
    });
    if (answer.failed) {
      throw new MaraError(answer.reply, stats ? answer.stats : undefined);
    }
    return answer;
  }

  /**
   * Runs the document operation whose MCP tool is `name` with the tool's `args`. Resolves with
   * its result, or with `{ result, stats }` when `options.debugStats` is true.
   */
  async run(name, args = {}, options = {}) {
    const stats = options.debugStats === true;
    const answer = await this.#call({ call: "run", name, arguments: args }, stats);
    return stats ? { result: answer.reply, stats: answer.stats } : answer.reply;
  }

  /** Gives the file at `path`, relative to the project root, the bytes `contents`. */
  async write(path, contents) {
    await this.#call({ call: "write", path, contents: bytes(contents, "contents") });
  }

  /** Removes the file at `path`, relative to the project root. */
  async remove(path) {
    await this.#call({ call: "remove", path });
  }

  /** The change set: `{ path, contents, base }` for each file that differs from its supplied bytes. */
  async export() {
    return (await this.#call({ call: "export" })).changes;
  }

  /** Ends the worker; every later call rejects. */
  close() {
    this.#stop(new Error("the workspace is closed"));
    this.#worker.terminate();
  }

  /** Starts a workspace of `files`, each `{ path, contents }`, in a worker of its own. */
  static async create(files) {
    const supplied = Array.from(files, ({ path, contents }) => ({
      path,
      contents: bytes(contents, `the contents of ${path}`),
    }));
    const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
    const workspace = new Workspace(worker);
    try {
      await workspace.#call({ call: "create", files: supplied });
    } catch (error) {
      workspace.close();
      throw error;
    }
    return workspace;
  }
}

/** Starts a workspace of `files`, each `{ path, contents }` with the path relative to the project root. */
export function createWorkspace(files) {
  return Workspace.create(files);
}
