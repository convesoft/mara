#!/usr/bin/env node

"use strict";

const path = require("node:path");
const { spawn } = require("node:child_process");

const packages = new Map([
  ["linux:x64", "@convesoft/mara-linux-x64-gnu"],
  ["linux:arm64", "@convesoft/mara-linux-arm64-gnu"],
  ["darwin:x64", "@convesoft/mara-darwin-x64"],
  ["darwin:arm64", "@convesoft/mara-darwin-arm64"],
]);

const manifest = require("../package.json");
const packageName = packages.get(`${process.platform}:${process.arch}`);
let hasLocalRuntime = false;

if (packageName !== undefined) {
  try {
    require.resolve(`${packageName}/package.json`);
    hasLocalRuntime = true;
  } catch {
    // Codex extracts npm plugin packages without installing their dependencies.
  }
}

const command = hasLocalRuntime ? process.execPath : "npx";
const args = hasLocalRuntime
  ? [path.join(__dirname, "mara.cjs"), ...process.argv.slice(2)]
  : ["--yes", `${manifest.name}@${manifest.version}`, ...process.argv.slice(2)];
const child = spawn(command, args, {
  cwd: hasLocalRuntime ? undefined : path.parse(__dirname).root,
  stdio: "inherit",
});
const signals = ["SIGINT", "SIGTERM", "SIGHUP"];
const forward = new Map();

for (const signal of signals) {
  const handler = () => child.kill(signal);
  forward.set(signal, handler);
  process.on(signal, handler);
}

child.once("error", (error) => {
  console.error(`Could not start Mara from the Agent Plugin: ${error.message}`);
  process.exitCode = 1;
});

child.once("exit", (code, signal) => {
  for (const [name, handler] of forward) {
    process.off(name, handler);
  }

  if (signal !== null) {
    process.kill(process.pid, signal);
  } else {
    process.exitCode = code ?? 1;
  }
});
