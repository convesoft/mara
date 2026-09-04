#!/usr/bin/env node

import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
import { createInterface } from "node:readline";
import path from "node:path";

function usage() {
  console.error(
    "usage: smoke-plugin-mcp.mjs <plugin-root> <absolute-project-path>",
  );
  process.exit(2);
}

const [pluginArgument, projectArgument] = process.argv.slice(2);
if (pluginArgument === undefined || projectArgument === undefined) usage();
if (!path.isAbsolute(projectArgument)) {
  throw new Error(`project path must be absolute: ${projectArgument}`);
}

const pluginRoot = path.resolve(pluginArgument);
const project = path.normalize(projectArgument);
const manifest = JSON.parse(
  await readFile(path.join(pluginRoot, "mcp.json"), "utf8"),
);
const server = manifest?.mcpServers?.mara;

if (
  server?.type !== "stdio" ||
  typeof server.command !== "string" ||
  !Array.isArray(server.args) ||
  !server.args.every((argument) => typeof argument === "string")
) {
  throw new Error("mcp.json does not define a valid Mara stdio server");
}

const expandPluginRoot = (value) =>
  value.replaceAll("${PLUGIN_ROOT}", pluginRoot);
const child = spawn(
  expandPluginRoot(server.command),
  server.args.map(expandPluginRoot),
  {
    cwd: pluginRoot,
    env: {
      ...process.env,
      ...Object.fromEntries(
        Object.entries(server.env ?? {}).map(([name, value]) => [
          name,
          expandPluginRoot(value),
        ]),
      ),
    },
    stdio: ["pipe", "pipe", "inherit"],
  },
);

const responses = new Map();
let nextId = 1;
let processFailure;

function failPending(error) {
  processFailure = error;
  for (const { reject, timeout } of responses.values()) {
    clearTimeout(timeout);
    reject(error);
  }
  responses.clear();
}

child.once("error", failPending);
child.once("exit", (code, signal) => {
  if (processFailure !== undefined || responses.size === 0) return;
  failPending(
    new Error(
      `Mara MCP exited before the smoke test completed (${signal ?? `status ${code}`})`,
    ),
  );
});

const lines = createInterface({ input: child.stdout });
lines.on("line", (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch (error) {
    failPending(
      new Error(`Mara MCP emitted invalid JSON: ${line}`, { cause: error }),
    );
    return;
  }

  if (message.id === undefined) return;
  const pending = responses.get(message.id);
  if (pending === undefined) return;
  responses.delete(message.id);
  clearTimeout(pending.timeout);
  if (message.error !== undefined) {
    pending.reject(
      new Error(
        `Mara ${pending.method} failed: ${message.error.message ?? JSON.stringify(message.error)}`,
      ),
    );
  } else {
    pending.resolve(message.result);
  }
});

function send(message) {
  if (processFailure !== undefined) throw processFailure;
  child.stdin.write(`${JSON.stringify(message)}\n`);
}

function request(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      responses.delete(id);
      reject(new Error(`Mara ${method} timed out`));
    }, 60_000);
    responses.set(id, { method, resolve, reject, timeout });
    send({ jsonrpc: "2.0", id, method, params });
  });
}

function assertToolResult(tool, result) {
  if (result?.isError !== false) {
    throw new Error(`${tool} returned an MCP error: ${JSON.stringify(result)}`);
  }
  return result.structuredContent;
}

try {
  await request("initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "mara_plugin_smoke", version: "1" },
  });
  send({ jsonrpc: "2.0", method: "notifications/initialized" });

  const initialized = assertToolResult(
    "project_init",
    await request("tools/call", {
      name: "project_init",
      arguments: { project },
    }),
  );
  if (initialized?.project?.root !== project) {
    throw new Error(
      `project_init returned the wrong project: ${JSON.stringify(initialized)}`,
    );
  }

  const validation = assertToolResult(
    "project_validate",
    await request("tools/call", {
      name: "project_validate",
      arguments: { project },
    }),
  );
  if (validation?.valid !== true) {
    throw new Error(`project_validate failed: ${JSON.stringify(validation)}`);
  }

  process.stdout.write(`${JSON.stringify({ project, valid: true })}\n`);
} finally {
  lines.close();
  child.stdin.end();
  child.kill();
  if (child.exitCode === null && child.signalCode === null) {
    await once(child, "exit");
  }
}
