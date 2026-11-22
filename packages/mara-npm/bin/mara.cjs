#!/usr/bin/env node
// The dispatcher of `@convesoft/mara` (DES-PACKAGES): it runs the `mara` of the native package of
// this platform, installed at exactly this package's version.

"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");

const minimumGlibc = "2.35";

// One row per platform that has a native package. The packaging script reads this table for the
// dispatcher's optional dependencies and for each native package's `os`, `cpu` and `libc`.
const platforms = [
  {
    os: "linux",
    cpu: "x64",
    libc: "glibc",
    name: "@convesoft/mara-linux-x64-gnu",
    label: `Linux x64 with glibc ${minimumGlibc} or later`,
  },
  {
    os: "linux",
    cpu: "arm64",
    libc: "glibc",
    name: "@convesoft/mara-linux-arm64-gnu",
    label: `Linux arm64 with glibc ${minimumGlibc} or later`,
  },
];

// The signals that the dispatcher passes on to `mara` while it runs.
const forwardedSignals = ["SIGINT", "SIGTERM", "SIGHUP"];

function supported() {
  return `Supported platforms: ${platforms.map((platform) => platform.label).join("; ")}.`;
}

function fail(message) {
  process.stderr.write(`mara: ${message}\n`);
  process.exitCode = 1;
}

// Compares two dotted version strings, such as glibc's `2.35`, part by part as numbers.
function compareVersions(left, right) {
  const a = left.split(".").map(Number);
  const b = right.split(".").map(Number);
  for (let index = 0; index < Math.max(a.length, b.length); index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference !== 0) {
      return difference;
    }
  }
  return 0;
}

// The `bin/mara` of the native package for this platform, or undefined after reporting why none
// can run.
function nativeBinary() {
  const platform = platforms.find(
    (candidate) => candidate.os === process.platform && candidate.cpu === process.arch,
  );
  if (platform === undefined) {
    fail(`Mara has no native package for ${process.platform} ${process.arch}. ${supported()}`);
    return undefined;
  }

  if (platform.libc === "glibc") {
    const found = process.report.getReport().header.glibcVersionRuntime;
    if (typeof found !== "string") {
      fail(
        `Mara requires glibc ${minimumGlibc} or later, and Node reports no glibc on this system, ` +
          `as on musl. ${supported()}`,
      );
      return undefined;
    }
    if (!/^\d+(\.\d+)*$/.test(found) || compareVersions(found, minimumGlibc) < 0) {
      fail(`Mara requires glibc ${minimumGlibc} or later; this system has glibc ${found}.`);
      return undefined;
    }
  }

  const version = JSON.parse(
    fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf8"),
  ).version;
  const reinstall =
    `Reinstall @convesoft/mara@${version} with optional dependencies enabled ` +
    "(without --omit=optional or --no-optional).";
  const directory = installedPackage(platform.name);
  if (directory === undefined) {
    fail(`The native package ${platform.name}@${version} is not installed. ${reinstall}`);
    return undefined;
  }
  const installed = JSON.parse(
    fs.readFileSync(path.join(directory, "package.json"), "utf8"),
  ).version;
  if (installed !== version) {
    fail(
      `The native package ${platform.name} at ${directory} has version ${installed}, and ` +
        `@convesoft/mara ${version} needs exactly ${version}. ${reinstall}`,
    );
    return undefined;
  }
  return path.join(directory, "bin", "mara");
}

// The folder of the package `name` installed for this dispatcher, links resolved: the first
// `node_modules` folder on the dispatcher's own ancestor path that holds it, in the order of
// Node's resolution, as npm, pnpm and npx install it. Node's global folders and `NODE_PATH` are
// left out, so a package found only there counts as not installed.
function installedPackage(name) {
  const ancestors = new Set();
  for (let folder = __dirname; ; folder = path.dirname(folder)) {
    ancestors.add(path.join(folder, "node_modules"));
    if (path.dirname(folder) === folder) {
      break;
    }
  }
  for (const folder of require.resolve.paths(name) ?? []) {
    if (ancestors.has(folder) && fs.existsSync(path.join(folder, name, "package.json"))) {
      return fs.realpathSync(path.join(folder, name));
    }
  }
  return undefined;
}

function run(binary) {
  // No shell: the arguments reach `mara` as they are, and the standard streams are this
  // process's own.
  const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });
  const handlers = forwardedSignals.map((signal) => {
    const handler = () => child.kill(signal);
    process.on(signal, handler);
    return [signal, handler];
  });
  const stopForwarding = () => {
    for (const [signal, handler] of handlers) {
      process.off(signal, handler);
    }
  };

  child.once("error", (error) => {
    stopForwarding();
    fail(`could not start ${binary}: ${error.message}`);
  });

  child.once("exit", (code, signal) => {
    stopForwarding();
    if (signal === null) {
      process.exitCode = code;
      return;
    }
    // End by the same signal. Node ignores some signals, such as SIGPIPE, and starts its
    // inspector on SIGUSR1; for those the shell's convention, 128 plus the signal's number, is
    // the exit status.
    if (signal !== "SIGUSR1") {
      process.kill(process.pid, signal);
    }
    process.exitCode = 128 + (os.constants.signals[signal] ?? 0);
  });
}

if (require.main === module) {
  const binary = nativeBinary();
  if (binary !== undefined) {
    run(binary);
  }
} else {
  module.exports = { minimumGlibc, platforms };
}
