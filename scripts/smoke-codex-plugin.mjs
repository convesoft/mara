#!/usr/bin/env node

import { spawn } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import path from "node:path";

function usage() {
  console.error(
    "usage: smoke-codex-plugin.mjs <absolute-project-path> <codex-command> [codex-arguments...]",
  );
  process.exit(2);
}

const [projectArgument, command, ...commandArguments] = process.argv.slice(2);
if (projectArgument === undefined || command === undefined) usage();

if (!path.isAbsolute(projectArgument)) {
  throw new Error(`project path must be absolute: ${projectArgument}`);
}
const project = path.normalize(projectArgument);

const appServer = spawn(command, [...commandArguments, "app-server", "--stdio"], {
  env: process.env,
  stdio: ["pipe", "pipe", "inherit"],
});
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

appServer.once("error", (error) => {
  failPending(error);
});
appServer.once("exit", (code, signal) => {
  if (processFailure !== undefined || responses.size === 0) return;
  failPending(
    new Error(
      `Codex app server exited before the smoke test completed (${signal ?? `status ${code}`})`,
    ),
  );
});

const lines = createInterface({ input: appServer.stdout });
lines.on("line", (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch (error) {
    failPending(
      new Error(`Codex app server emitted invalid JSON: ${line}`, {
        cause: error,
      }),
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
        `Codex ${pending.method} failed: ${message.error.message ?? JSON.stringify(message.error)}`,
      ),
    );
  } else {
    pending.resolve(message.result);
  }
});

function send(message) {
  if (processFailure !== undefined) throw processFailure;
  appServer.stdin.write(`${JSON.stringify(message)}\n`);
}

function request(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      responses.delete(id);
      reject(new Error(`Codex ${method} timed out`));
    }, 60_000);
    responses.set(id, { method, resolve, reject, timeout });
    send({ method, id, params });
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
    clientInfo: {
      name: "mara_codex_marketplace_smoke",
      title: "Mara Codex marketplace smoke test",
      version: "1",
    },
  });
  send({ method: "notifications/initialized" });

  const started = await request("thread/start", {
    cwd: process.cwd(),
    approvalPolicy: "never",
    sandbox: "read-only",
    ephemeral: true,
  });
  const threadId = started?.thread?.id;
  if (typeof threadId !== "string") {
    throw new Error(`Codex did not return a thread id: ${JSON.stringify(started)}`);
  }

  const initialized = assertToolResult(
    "project_init",
    await request("mcpServer/tool/call", {
      threadId,
      server: "mara",
      tool: "project_init",
      arguments: { project },
    }),
  );
  if (initialized?.project?.root !== project) {
    throw new Error(`project_init returned the wrong project: ${JSON.stringify(initialized)}`);
  }

  const validation = assertToolResult(
    "project_validate",
    await request("mcpServer/tool/call", {
      threadId,
      server: "mara",
      tool: "project_validate",
      arguments: { project },
    }),
  );
  if (validation?.valid !== true) {
    throw new Error(`project_validate failed: ${JSON.stringify(validation)}`);
  }

  process.stdout.write(`${JSON.stringify({ project, valid: true })}\n`);
} finally {
  lines.close();
  appServer.stdin.end();
  appServer.kill();
  if (appServer.exitCode === null && appServer.signalCode === null) {
    await once(appServer, "exit");
  }
}
