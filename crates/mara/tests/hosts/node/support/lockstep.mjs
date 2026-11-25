// The parity cases of VER-HOST-COMPARISON for several hosts at once: each request runs in CLI
// direct mode on a baseline copy of the project, then on each host's own copy, which held the same
// files before it; each host must return what direct mode printed and leave the files it left,
// once the MIDs it generated are paired with direct mode's by path and line
// (VER-CLI-MCP-PARITY). Then each host's copy is made equal to the baseline again, so that the
// next request starts from the same files everywhere. A test's own file edits change the
// baseline and reach every host the same way.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, renameSync, rmSync, rmdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import {
  applied,
  cli,
  filesOf,
  pairMids,
  replaceMids,
  testDir,
  withoutGeneratedRevisions,
} from "./hosts.mjs";

const maraBin = process.env.MARA_BIN;

/**
 * The MCP tool call `name` with `args` to `mara mcp --no-daemon` bound to `root`: what it
 * returns, as `{ status, json }` with the status that the CLI would exit with. The baseline of an
 * argument that a client rejects, whose message names the parameter as MCP does (DES-MCP-SERVER).
 */
export function mcpDirect(root, name, args) {
  const lines = [
    { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "test", version: "1" } } },
    { jsonrpc: "2.0", method: "notifications/initialized" },
    { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name, arguments: args } },
  ];
  const ran = spawnSync(maraBin, ["mcp", "--no-daemon", "--project", root], {
    cwd: root,
    env: { MARA_NO_DAEMON: "1", HOME: join(testDir, "home"), XDG_RUNTIME_DIR: join(testDir, "run") },
    input: lines.map((line) => JSON.stringify(line)).join("\n") + "\n",
    encoding: "utf8",
  });
  const response = ran.stdout
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line))
    .find((message) => message.id === 2);
  assert.ok(response?.result, `mara mcp answered ${ran.stdout}${ran.stderr}`);
  const parsed = JSON.parse(response.result.content[0].text);
  return { status: parsed.error ? 2 : 0, json: parsed };
}

/** The tool list of `mara mcp`, each tool with its input schema. */
export function toolList(root) {
  const lines = [
    { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "test", version: "1" } } },
    { jsonrpc: "2.0", method: "notifications/initialized" },
    { jsonrpc: "2.0", id: 2, method: "tools/list" },
  ];
  const ran = spawnSync(maraBin, ["mcp", "--no-daemon", "--project", root], {
    cwd: root,
    env: { MARA_NO_DAEMON: "1", HOME: join(testDir, "home"), XDG_RUNTIME_DIR: join(testDir, "run") },
    input: lines.map((line) => JSON.stringify(line)).join("\n") + "\n",
    encoding: "utf8",
  });
  const response = ran.stdout
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line))
    .find((message) => message.id === 2);
  return response.result.tools;
}

/** Makes `root` hold exactly `files`, `.git` left as it is, writing only what differs. */
export function restore(root, files) {
  const now = filesOf(root);
  for (const path of now.keys()) {
    if (!files.has(path)) {
      rmSync(join(root, path));
      // A directory goes with its last file, as in a Git checkout.
      for (let directory = dirname(join(root, path)); directory !== root; directory = dirname(directory)) {
        if (readdirSync(directory).length > 0) {
          break;
        }
        rmdirSync(directory);
      }
    }
  }
  for (const [path, bytes] of files) {
    if (!now.get(path)?.equals(bytes)) {
      mkdirSync(dirname(join(root, path)), { recursive: true });
      writeFileSync(join(root, path), bytes);
    }
  }
}

/** A host on a copy of the project on disk: `call(name, args)` answers as `startClient`'s `run`. */
export function diskHost(label, root, call, { served } = {}) {
  return {
    label,
    files: async () => filesOf(root),
    sync: async (files) => restore(root, files),
    call,
    served,
  };
}

/** A browser workspace of `page`, created from `supplied`, the files of the project but `.git`. */
export function workspaceHost(label, page, supplied) {
  const encode = (bytes) => bytes.toString("base64");
  const host = {
    label,
    files: async () => {
      const exported = await page.harness("export");
      assert.equal(exported.error, undefined, JSON.stringify(exported.error));
      return applied(supplied, exported.changes);
    },
    sync: async (files) => {
      const now = await host.files();
      for (const path of now.keys()) {
        if (!files.has(path)) {
          assert.deepEqual(await page.harness("remove", path), {});
        }
      }
      for (const [path, bytes] of files) {
        if (!now.get(path)?.equals(bytes)) {
          assert.deepEqual(await page.harness("write", path, encode(bytes)), {});
        }
      }
    },
    call: (name, args) => page.harness("run", name, args),
  };
  return host;
}

/**
 * The project of the baseline, at `root`, and the hosts that each hold a copy of it. `sent`
 * holds the tool of every request sent.
 */
export class Lockstep {
  constructor(root, hosts) {
    this.root = root;
    this.hosts = hosts;
    this.sent = new Set();
    /** Each request's tool and what direct mode printed for it. */
    this.printed = [];
  }

  /** The baseline's file `path` as text. */
  read(path) {
    return filesOf(this.root).get(path).toString("utf8");
  }

  /** Writes `contents` at `path` of every host's project, as another program would. */
  async write(path, contents) {
    mkdirSync(dirname(join(this.root, path)), { recursive: true });
    writeFileSync(join(this.root, path), contents);
    await this.#sync();
  }

  /** Renames `from` to `to` in every host's project, as another program would. */
  async rename(from, to) {
    mkdirSync(dirname(join(this.root, to)), { recursive: true });
    renameSync(join(this.root, from), join(this.root, to));
    await this.#sync();
  }

  async #sync() {
    const files = filesOf(this.root);
    for (const host of this.hosts) {
      await host.sync(files);
    }
  }

  /**
   * The tool call `name` with `args` on every host, compared with CLI direct mode, or with
   * `mara mcp --no-daemon` when the client rejects the arguments before it sends them
   * (`rejected`), and the baseline's printed object.
   */
  async run(name, args = {}, { rejected = false } = {}) {
    const before = filesOf(this.root);
    const expected = rejected ? mcpDirect(this.root, name, args) : cli(this.root, name, args);
    const expectedFiles = filesOf(this.root);
    const shown = `${name} ${JSON.stringify(args)}`;
    for (const host of this.hosts) {
      const served = host.served?.before();
      const answer = await host.call(name, args);
      host.served?.after(served, rejected ? 0 : 1, shown);
      if (answer.promised !== undefined) {
        assert.equal(answer.promised, true, `${host.label}: ${shown} did not return a promise`);
      }
      const output = answer.error ? answer.error.reply : answer.result;
      assert.ok(output, `${host.label}: ${shown} answered ${JSON.stringify(answer)}`);
      const files = await host.files();
      const pairs = pairMids(before, expectedFiles, files);
      const mids = new Set(pairs.values());
      const got = withoutGeneratedRevisions(JSON.parse(replaceMids(JSON.stringify(output), pairs)), mids);
      assert.deepEqual(
        got,
        withoutGeneratedRevisions(expected.json, mids),
        `${host.label}: ${shown} differs from direct mode`,
      );
      assert.equal(
        answer.error !== undefined,
        expected.json.error !== undefined,
        `${host.label}: ${shown} fails only where direct mode fails`,
      );
      const paired = new Map([...files].map(([path, bytes]) => [path, Buffer.from(replaceMids(bytes.toString("latin1"), pairs), "latin1")]));
      const differ = [...new Set([...paired.keys(), ...expectedFiles.keys()])].filter(
        (path) => !paired.get(path)?.equals(expectedFiles.get(path) ?? Buffer.alloc(0)) || !expectedFiles.has(path),
      );
      assert.deepEqual(differ, [], `${host.label}: ${shown} left other files than direct mode`);
      await host.sync(expectedFiles);
    }
    this.sent.add(name);
    this.printed.push({ name, json: expected.json });
    return expected.json;
  }
}

/**
 * A TypeScript module that declares each object of `printed`, `{ name, json }`, as the result of
 * its operation, or as a failure, with the library's types (VER-NODE-LIBRARY): it compiles only
 * when the types hold every member that the operations printed, and no other.
 */
export function printedModule(printed) {
  const lines = ['import type { FailureReply, Result } from "@convesoft/mara";', ""];
  printed.forEach(({ name, json }, index) => {
    const type = json.error === undefined ? `Result<${JSON.stringify(name)}>` : "FailureReply";
    lines.push(`export const printed${index}: ${type} = ${JSON.stringify(json)};`);
  });
  return `${lines.join("\n")}\n`;
}
