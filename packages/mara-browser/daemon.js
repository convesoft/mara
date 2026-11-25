// A browser's client of the daemon's listener (DES-NODE-LIBRARY, DES-BROWSER-PROTOCOL): the
// document operations of one project of the daemon, sent as MCP tool calls are written and
// answered as `--format json` prints them. Like every client it checks the arguments before it
// sends them (DES-OPERATIONS): their names, presence and types as `mara mcp` checks a tool call;
// the daemon checks their values, and its refusal of them is the call's `invalid-argument`.

import { MaraError } from "./errors.js";

/** The version of the listener's requests. */
const PROTOCOL_VERSION = 1;

// Each document operation's parameters by MCP tool name: its positional arguments in order,
// then each option with its kind, as `mara mcp`'s tool list declares them. `strings` and
// `integers` repeat; `input` holds the inputs of a `query` request.
const PAGED = { limit: "integer", cursor: "string" };
const FILTERS = {
  ...PAGED,
  type: "strings",
  field: "strings",
  relation: "strings",
  path: "strings",
  id: "strings",
};
const EVALUATED = { ...PAGED, input: "inputs", complete: "strings" };
const TOOLS = {
  project_validate: { arguments: [], options: { ...PAGED, path: "strings" } },
  project_init: { arguments: [], options: { template: "string" } },
  project_fix: { arguments: [], options: {} },
  project_convert: { arguments: [], options: { status: "string" } },
  item_validate: { arguments: ["id-or-mid"], options: PAGED },
  schema_validate: { arguments: [], options: PAGED },
  search: { arguments: ["query"], options: FILTERS },
  get: { arguments: ["node"], options: { cursor: "string" } },
  relation_list: { arguments: ["node"], options: { ...PAGED, relation: "strings" } },
  item_list: { arguments: [], options: FILTERS },
  schema_list: { arguments: [], options: PAGED },
  schema_get: { arguments: ["name"], options: PAGED },
  item_move: { arguments: ["id-or-mid", "path"], options: { line: "integer" } },
  item_rename: { arguments: ["id-or-mid", "new-id"], options: {} },
  item_create: {
    arguments: ["type", "path"],
    options: { title: "string", body: "string", id: "string", field: "strings", line: "integer" },
  },
  item_update: {
    arguments: ["item"],
    options: { title: "string", body: "string", field: "strings", "clear-field": "strings" },
  },
  item_delete: { arguments: ["item"], options: {} },
  relation_add: { arguments: ["source", "relation", "target"], options: {} },
  relation_remove: { arguments: ["source", "relation", "target"], options: {} },
  query_list: { arguments: [], options: PAGED },
  query_execute: { arguments: ["name"], options: { ...EVALUATED, id: "strings" } },
  query_view: { arguments: ["name"], options: { ...EVALUATED, id: "strings" } },
  query_gate: { arguments: ["name"], options: { ...EVALUATED, id: "strings" } },
  query_report: { arguments: ["name"], options: { ...EVALUATED, id: "string" } },
};

/** The parameters of each tool, for the test that compares them with `mara mcp`'s tool list. */
export const tools = TOOLS;

/** A failure with the code `code`, as `--format json` prints one; nothing ran. */
function failure(code, message) {
  return new MaraError({ format_version: 1, error: { code, message, diagnostics: [] } });
}

/** Rejected arguments, in the words of `mara mcp`. */
const rejected = (message) => failure("invalid-argument", message);

/** One value of an option of `kind`, as the CLI reads it. */
function scalar(name, kind, value) {
  if (kind === "integer" || kind === "integers") {
    if (typeof value !== "number" || !Number.isInteger(value)) {
      throw rejected(`\`${name}\` is a whole number`);
    }
    return String(value);
  }
  if (typeof value !== "string") {
    throw rejected(`\`${name}\` is a string`);
  }
  return value;
}

/**
 * The listener's request for the tool call `name` with `args` in `project`: the operation's
 * words, positional arguments, options and inputs (DES-BROWSER-PROTOCOL). Throws the refusal of
 * arguments that `mara mcp` would reject before the operation runs.
 */
export function request(name, args, project) {
  const tool = Object.hasOwn(TOOLS, name) ? TOOLS[name] : undefined;
  if (tool === undefined) {
    throw rejected(`unknown operation \`${name}\``);
  }
  if (args === null || args === undefined) {
    args = {};
  } else if (typeof args !== "object" || Array.isArray(args)) {
    throw rejected("the arguments are no object");
  }
  const positional = Array.from({ length: tool.arguments.length }, () => undefined);
  const options = [];
  const supplied = [];
  for (const [parameter, value] of Object.entries(args)) {
    if (value === null || value === undefined) {
      continue;
    }
    const index = tool.arguments.indexOf(parameter);
    const kind = Object.hasOwn(tool.options, parameter) ? tool.options[parameter] : undefined;
    if (index !== -1) {
      if (typeof value !== "string") {
        throw rejected(`\`${parameter}\` is a string`);
      }
      positional[index] = value;
    } else if (kind === "inputs") {
      const refuse = () =>
        rejected(
          `\`${parameter}\` is an object from each input's name to a string or to a list of rows`,
        );
      if (typeof value !== "object" || Array.isArray(value)) {
        throw refuse();
      }
      for (const [input, given] of Object.entries(value)) {
        if (typeof given === "string") {
          supplied.push([input, { inline: given }]);
        } else if (Array.isArray(given)) {
          supplied.push([input, { rows: given }]);
        } else {
          throw refuse();
        }
      }
    } else if (kind === "strings" || kind === "integers") {
      if (!Array.isArray(value)) {
        throw rejected(
          `\`${parameter}\` is an array of ${kind === "integers" ? "integers" : "strings"}`,
        );
      }
      for (const one of value) {
        options.push([parameter, scalar(parameter, kind, one)]);
      }
    } else if (kind !== undefined) {
      options.push([parameter, scalar(parameter, kind, value)]);
    } else if (parameter === "project") {
      throw rejected(
        "`project` is no parameter of a client, which holds the project it was created with",
      );
    } else {
      throw rejected(`unknown parameter \`${parameter}\` of \`${name}\``);
    }
  }
  const missing = tool.arguments.find((_, index) => positional[index] === undefined);
  if (missing !== undefined) {
    throw rejected(`\`${name}\` needs \`${missing}\``);
  }
  const words = name === "search" || name === "get" ? [name] : name.split("_");
  return {
    version: PROTOCOL_VERSION,
    request: "operation",
    project,
    words,
    arguments: positional,
    options,
    supplied,
    json: true,
  };
}

/** A client of one project that the daemon's listener at `url` serves. */
export class DaemonClient {
  #url;
  #token;
  #project;

  constructor({ url, token, project }) {
    this.#url = url;
    this.#token = token;
    this.#project = project;
  }

  /**
   * Runs the document operation whose MCP tool is `name` with the tool's `args`. Resolves with
   * its result, or with `{ result, stats }` when `options.debugStats` is true.
   */
  async run(name, args = {}, options = {}) {
    const stats = options?.debugStats === true;
    const body = JSON.stringify(request(name, args, this.#project));
    const response = await fetch(new URL("/operation", this.#url), {
      method: "POST",
      headers: { Authorization: `Bearer ${this.#token}`, "Content-Type": "application/json" },
      body,
      cache: "no-store",
      credentials: "omit",
    });
    const answer = await response.json();
    if (response.status !== 200) {
      // The listener checks what the client sent as the daemon checks a native request: what
      // it refuses as malformed is an argument's value, since the client wrote the rest.
      const code = answer.error === "malformed-request" ? "invalid-argument" : answer.error;
      throw failure(code, answer.message);
    }
    const reply = JSON.parse(answer.text);
    const work = { format_version: 1, ...answer.work };
    if (answer.failed) {
      throw new MaraError(reply, stats ? work : undefined);
    }
    return stats ? { result: reply, stats: work } : reply;
  }
}

/**
 * A client of the project `options.project`, an absolute path that the daemon allows, through
 * the daemon's listener at `options.url`, such as `http://127.0.0.1:<port>`, with the token that
 * `mara daemon start --http` printed.
 */
export function connectDaemon(options) {
  return new DaemonClient(options ?? {});
}
