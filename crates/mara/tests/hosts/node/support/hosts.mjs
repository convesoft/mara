// The setup of the Node and browser cases (VER-HOST-COMPARISON): the fixtures and CLI direct
// mode on copies of them, the loopback page server, Playwright's Chromium with a new profile in
// the test's directory, and a page that holds a browser workspace. `hosts/node.rs` starts each
// module with the environment that `env` reads.

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, relative, sep } from "node:path";

export { launchBrowser } from "./browser.mjs";

/** A variable that `hosts/node.rs` sets. */
function env(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is not set: run these modules through \`cargo test --test hosts\``);
  }
  return value;
}

export const testDir = env("MARA_HOSTS_DIR");
const fixturesDir = env("MARA_HOSTS_FIXTURES");
const maraBin = env("MARA_BIN");
const packageDir = env("MARA_BROWSER_PACKAGE");

/** The files below `root` but `.git`, by path relative to it with `/` between parts. */
export function filesOf(root) {
  const files = new Map();
  const walk = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.name === ".git") {
        continue;
      }
      if (entry.isDirectory()) {
        walk(path);
      } else {
        files.set(relative(root, path).split(sep).join("/"), readFileSync(path));
      }
    }
  };
  walk(root);
  return new Map([...files].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
}

/** The fixture `name`: its root, its files but `.git`, and their BLAKE3 hashes, if it has them. */
export function fixture(name) {
  const root = join(fixturesDir, name);
  let hashes = {};
  try {
    hashes = JSON.parse(readFileSync(join(fixturesDir, `${name}.hashes.json`), "utf8"));
  } catch {
    // A fixture without hashes.
  }
  return { root, files: filesOf(root), hashes };
}

/** A copy of `root`, `.git` included, in the test's directory, for CLI direct mode. */
export function copy(root) {
  const parent = join(testDir, "copies");
  mkdirSync(parent, { recursive: true });
  const target = mkdtempSync(join(parent, "project-"));
  cpSync(root, target, { recursive: true });
  return target;
}

/** The positional arguments of each document operation, in order, by MCP tool name. */
const POSITIONAL = {
  item_validate: ["id-or-mid"],
  search: ["query"],
  get: ["node"],
  relation_list: ["node"],
  schema_get: ["name"],
  item_move: ["id-or-mid", "path"],
  item_rename: ["id-or-mid", "new-id"],
  item_create: ["type", "path"],
  item_update: ["item"],
  item_delete: ["item"],
  relation_add: ["source", "relation", "target"],
  relation_remove: ["source", "relation", "target"],
  query_execute: ["name"],
  query_view: ["name"],
  query_gate: ["name"],
  query_report: ["name"],
};

let inputFiles = 0;

/**
 * The CLI's `--input` arguments for the `input` object of a `query` tool call: a string value
 * inline, rows as a JSON file of the test's directory, outside every project.
 */
function inputArguments(input) {
  const out = [];
  for (const [name, value] of Object.entries(input)) {
    if (typeof value === "string") {
      out.push("--input", `${name}=${value}`);
      continue;
    }
    const directory = join(testDir, "inputs");
    mkdirSync(directory, { recursive: true });
    const file = join(directory, `${inputFiles++}.json`);
    writeFileSync(file, JSON.stringify(value));
    out.push("--input", `${name}=@${file}`);
  }
  return out;
}

/** The CLI's arguments for the MCP tool call `name` with `args`. */
export function cliArguments(name, args = {}) {
  const words = name === "search" || name === "get" ? [name] : name.split("_");
  const positional = POSITIONAL[name] ?? [];
  const out = [...words, ...positional.map((argument) => String(args[argument]))];
  for (const [option, value] of Object.entries(args)) {
    if (positional.includes(option) || value === null || value === undefined) {
      continue;
    }
    if (option === "input" && typeof value === "object" && !Array.isArray(value)) {
      out.push(...inputArguments(value));
      continue;
    }
    for (const one of Array.isArray(value) ? value : [value]) {
      out.push(`--${option}`, String(one));
    }
  }
  return out;
}

/**
 * Runs the MCP tool call `name` with `args` as `mara --no-daemon --format json` in `root`, with
 * an environment of the test's own: `HOME`, so a per-user grammar cache that is empty, and the
 * runtime directory, so any daemon would name the test's directory. Returns the printed object
 * and the exit status.
 */
export function cli(root, name, args = {}) {
  const home = join(testDir, "home");
  const runtime = join(testDir, "run");
  mkdirSync(home, { recursive: true });
  mkdirSync(runtime, { recursive: true, mode: 0o700 });
  const argv = ["--no-daemon", "--format", "json", ...cliArguments(name, args)];
  let stdout;
  let status = 0;
  try {
    stdout = execFileSync(maraBin, argv, {
      cwd: root,
      env: { MARA_NO_DAEMON: "1", HOME: home, XDG_RUNTIME_DIR: runtime },
      encoding: "utf8",
    });
  } catch (error) {
    if (typeof error.status !== "number") {
      throw error;
    }
    ({ stdout, status } = error);
  }
  return { status, json: JSON.parse(stdout) };
}

/** The MID of each anchor line, by `<path>:<line>`. */
function anchors(files) {
  const out = new Map();
  for (const [path, bytes] of files) {
    const lines = bytes.toString("utf8").split("\n");
    lines.forEach((line, index) => {
      const mid = /^<a id="([0-9A-Z]{26})"><\/a>$/.exec(line.trim())?.[1];
      if (mid) {
        out.set(`${path}:${index + 1}`, mid);
      }
    });
  }
  return out;
}

/**
 * The MIDs that a run on the files `before` generated, each paired with the one that the run
 * which left `expected` generated at the same path and line (VER-CLI-MCP-PARITY): a map from
 * each of `files`' new MIDs to `expected`'s. Both runs must have generated MIDs at the same
 * places.
 */
export function pairMids(before, expected, files) {
  const known = new Set(anchors(before).values());
  const generated = (of) => new Map([...anchors(of)].filter(([, mid]) => !known.has(mid)));
  const [theirs, ours] = [generated(expected), generated(files)];
  const places = (map) => [...map.keys()].sort().join(", ");
  if (places(theirs) !== places(ours)) {
    throw new Error(`MIDs generated at other places: ${places(ours)} against ${places(theirs)}`);
  }
  const pairs = new Map();
  for (const [place, mid] of ours) {
    pairs.set(mid, theirs.get(place));
  }
  return pairs;
}

/** `text` with each MID that `pairs` holds replaced by its pair. */
export function replaceMids(text, pairs) {
  let out = text;
  for (const [mid, paired] of pairs) {
    out = out.split(mid).join(paired);
  }
  return out;
}

/** `value` without the `revision` of each object that holds one of `mids` (VER-CLI-MCP-PARITY). */
export function withoutGeneratedRevisions(value, mids) {
  if (Array.isArray(value)) {
    return value.map((element) => withoutGeneratedRevisions(element, mids));
  }
  if (value && typeof value === "object") {
    const generated = Object.values(value).some((member) => mids.has(member));
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => !(generated && key === "revision"))
        .map(([key, member]) => [key, withoutGeneratedRevisions(member, mids)]),
    );
  }
  return value;
}

/** The files `supplied` with the change set `changes` applied. */
export function applied(supplied, changes) {
  const files = new Map(supplied);
  for (const { path, contents } of changes) {
    if (contents === null) {
      files.delete(path);
    } else {
      files.set(path, Buffer.from(contents, "base64"));
    }
  }
  return new Map([...files].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
}

/** The change set that turns `before` into `after`, with `hashes` of `before`'s files. */
export function changeSet(before, after, hashes) {
  const paths = [...new Set([...before.keys(), ...after.keys()])].sort();
  return paths
    .filter((path) => !(before.has(path) && after.has(path) && before.get(path).equals(after.get(path))))
    .map((path) => ({
      path,
      base: before.has(path) ? hashes[path] : null,
      contents: after.has(path) ? after.get(path).toString("base64") : null,
    }));
}

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".ts": "text/plain; charset=utf-8",
  ".wasm": "application/wasm",
};

/**
 * The files of the browser package: those its manifest lists, and Soufflé's module, which
 * `mara-browser:build` copies there and mara-865a2e packs.
 */
const SOUFFLE_MODULE = ["souffle.js", "souffle.wasm"];
const packageFiles = [
  ...JSON.parse(readFileSync(join(packageDir, "package.json"), "utf8")).files,
  ...SOUFFLE_MODULE,
];

/**
 * Checks that the package holds Soufflé's module, which every case that runs a program needs
 * (VER-HOST-COMPARISON); without it each evaluation would be a fault.
 */
export function requireSouffle() {
  for (const name of SOUFFLE_MODULE) {
    if (!existsSync(join(packageDir, name))) {
      throw new Error(
        `the browser package has no ${name} of Soufflé's module: build it with ` +
          "scripts/build-souffle-wasm.sh, which `nx run mara-browser:build` runs and copies",
      );
    }
  }
}

/**
 * An HTTP server on a loopback port that the operating system assigns: `/` is the test's page,
 * and `/package/<file>` each file of the browser package. `requests` holds the path of every
 * request it received; `answer(path, handler)` makes `handler(response)` answer `path`, and
 * `answer(path, null)` serves it again.
 */
export async function startServer() {
  const requests = [];
  const handlers = new Map();
  const server = createServer((request, response) => {
    const path = new URL(request.url, "http://localhost").pathname;
    requests.push(path);
    if (handlers.has(path)) {
      handlers.get(path)(response);
      return;
    }
    if (path === "/") {
      response.writeHead(200, { "content-type": TYPES[".html"] });
      response.end(PAGE);
      return;
    }
    const name = path.startsWith("/package/") ? path.slice("/package/".length) : null;
    if (name && packageFiles.includes(name)) {
      let bytes;
      try {
        bytes = readFileSync(join(packageDir, name));
      } catch {
        response.writeHead(404).end();
        return;
      }
      response.writeHead(200, { "content-type": TYPES[extname(name)] ?? "application/octet-stream" });
      response.end(bytes);
      return;
    }
    response.writeHead(404).end();
  });
  // Connections are closed with the server, so that `close` ends.
  const sockets = new Set();
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    url: `http://127.0.0.1:${port}`,
    requests,
    answer: (path, handler) => (handler ? handlers.set(path, handler) : handlers.delete(path)),
    close: () =>
      new Promise((resolve) => {
        for (const socket of sockets) {
          socket.destroy();
        }
        server.close(resolve);
      }),
  };
}

/** Whether `path`, a path the server received, names a file of the browser package. */
export function isPackageFile(path) {
  return path.startsWith("/package/") && packageFiles.includes(path.slice("/package/".length));
}

/**
 * A page of the test, in a new context of `browser`, that loads the browser package from
 * `server` after it wrapped `WebAssembly`'s compiling and instantiating functions to count their
 * calls. `harness(name, ...args)` calls a function of the page's harness. `workers` and
 * `requests` hold what Playwright reports; `souffleWorkers()` each Soufflé worker that a
 * workspace started, `{ closed }`, in order.
 */
export async function openPage(browser, server) {
  const context = await browser.newContext();
  const page = await context.newPage();
  const workers = [];
  const states = [];
  const requests = [];
  page.on("worker", (worker) => {
    workers.push(worker.url());
    const state = { url: worker.url(), closed: false };
    states.push(state);
    worker.on("close", () => {
      state.closed = true;
    });
  });
  context.on("request", (request) => requests.push(request.url()));
  await page.goto(server.url);
  await page.waitForFunction(() => window.harness !== undefined || window.harnessError !== undefined);
  const failed = await page.evaluate(() => window.harnessError);
  if (failed) {
    throw new Error(`the page could not load the browser package: ${failed}`);
  }
  return {
    page,
    workers,
    souffleWorkers: () => states.filter((state) => state.url.endsWith("/package/souffle-worker.js")),
    requests,
    harness: (name, ...args) => page.evaluate(([name, args]) => window.harness[name](...args), [name, args]),
    close: () => context.close(),
  };
}

/** Bytes as base64, for a call of the page's harness. */
export function encode(files) {
  return [...files].map(([path, bytes]) => ({ path, contents: bytes.toString("base64") }));
}

// The test's page. Its module wraps `WebAssembly`'s functions before it imports the package by
// its path, then keeps the workspace and the supplied buffers, with copies to compare them with.
// Bytes cross `page.evaluate` as base64.
const PAGE = `<!doctype html>
<html>
<head><meta charset="utf-8"><link rel="icon" href="data:,"><title>Mara browser workspace</title></head>
<body>
<script type="module">
const calls = {};
for (const name of ["compile", "compileStreaming", "instantiate", "instantiateStreaming"]) {
  const original = WebAssembly[name];
  calls[name] = 0;
  WebAssembly[name] = function (...args) {
    calls[name] += 1;
    return original.apply(this, args);
  };
}
for (const name of ["Module", "Instance"]) {
  const original = WebAssembly[name];
  calls[name] = 0;
  WebAssembly[name] = new Proxy(original, {
    construct(target, args, newTarget) {
      calls[name] += 1;
      return Reflect.construct(target, args, newTarget);
    },
  });
}
const decode = (text) => Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
const encode = (bytes) => {
  let text = "";
  for (let at = 0; at < bytes.length; at += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(at, at + 0x8000));
  }
  return btoa(text);
};
const failure = (error) => ({
  error: { name: error.name, code: error.code, message: error.message, reply: error.reply, stats: error.stats },
});
try {
  const { createWorkspace } = await import("/package/index.js");
  let workspace;
  const supplied = [];
  const call = async (run) => {
    try {
      return await run();
    } catch (error) {
      return failure(error);
    }
  };
  window.harness = {
    wasmCalls: () => ({ ...calls }),
    create: (files) =>
      call(async () => {
        workspace?.close();
        const buffers = files.map(({ path, contents }) => ({ path, contents: decode(contents) }));
        supplied.push(...buffers.map(({ contents }) => ({ buffer: contents, copy: contents.slice() })));
        workspace = await createWorkspace(buffers);
        return {};
      }),
    run: (name, args, options) => call(() => workspace.run(name, args, options).then((result) => (options?.debugStats ? result : { result }))),
    write: (path, contents) => call(() => workspace.write(path, decode(contents)).then(() => ({}))),
    remove: (path) => call(() => workspace.remove(path).then(() => ({}))),
    export: () =>
      call(async () => ({
        changes: (await workspace.export()).map(({ path, contents, base }) => ({
          path,
          base,
          contents: contents === null ? null : encode(contents),
        })),
      })),
    // A call with a BigInt argument, which no JSON holds.
    runWithBigInt: (name) => call(() => workspace.run(name, { limit: 10n }).then((result) => ({ result }))),
    // Writes bytes made in a same-origin frame, another realm, as a Uint8Array and as an
    // ArrayBuffer.
    writeFromFrame: (path, contents) =>
      call(async () => {
        const frame = document.createElement("iframe");
        document.body.append(frame);
        const bytes = new frame.contentWindow.Uint8Array(decode(contents));
        await workspace.write(path, bytes);
        await workspace.write(path + ".copy", bytes.buffer);
        frame.remove();
        return {};
      }),
    // Sends two calls \`create\` to one worker of the package, and returns each answer.
    secondCreate: (files) =>
      new Promise((resolve) => {
        const worker = new Worker("/package/worker.js", { type: "module" });
        const supplied = files.map(({ path, contents }) => ({ path, contents: decode(contents) }));
        const answers = [];
        worker.onmessage = ({ data }) => {
          answers.push(data.answer ?? data);
          if (answers.length === 2) {
            worker.terminate();
            resolve(answers);
          }
        };
        worker.postMessage({ id: 0, call: "create", files: supplied });
        worker.postMessage({ id: 1, call: "create", files: supplied });
      }),
    close: () => {
      workspace?.close();
      workspace = undefined;
      return {};
    },
    suppliedIntact: () =>
      supplied.every(
        ({ buffer, copy }) => buffer.length === copy.length && buffer.every((byte, at) => byte === copy[at]),
      ),
    storage: async () => {
      const opfs = [];
      for await (const name of (await navigator.storage.getDirectory()).keys()) {
        opfs.push(name);
      }
      return {
        localStorage: localStorage.length,
        sessionStorage: sessionStorage.length,
        indexedDB: (await indexedDB.databases()).map((database) => database.name),
        caches: await caches.keys(),
        opfs,
      };
    },
  };
} catch (error) {
  window.harnessError = String(error.stack ?? error);
}
</script>
</body>
</html>
`;

/** `value` as JSON reads it back: without members that are `undefined`. */
export function json(value) {
  return JSON.parse(JSON.stringify(value));
}
