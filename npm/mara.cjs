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

const platform = `${process.platform}:${process.arch}`;
const packageName = packages.get(platform);

if (packageName === undefined) {
  console.error(
    `Mara does not provide a binary for ${process.platform}/${process.arch}. ` +
      "Supported targets are glibc Linux and macOS on x64 or arm64.",
  );
  process.exitCode = 1;
} else {
  let binary;
  try {
    const manifest = require.resolve(`${packageName}/package.json`);
    binary = path.join(path.dirname(manifest), "bin", "mara");
  } catch {
    console.error(
      `The optional package ${packageName} is missing. ` +
        "Reinstall @convesoft/mara with optional dependencies enabled, and " +
        "confirm that the host uses glibc when running Linux.",
    );
    process.exitCode = 1;
  }

  if (binary !== undefined) {
    const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });
    const signals = ["SIGINT", "SIGTERM", "SIGHUP"];
    const forward = new Map();

    for (const signal of signals) {
      const handler = () => child.kill(signal);
      forward.set(signal, handler);
      process.on(signal, handler);
    }

    child.once("error", (error) => {
      console.error(`Could not start ${packageName}: ${error.message}`);
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
  }
}
