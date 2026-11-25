#!/usr/bin/env node
// The install verification VER-INSTALL-WITHOUT-TOOLCHAIN, on the host's platform:
//
//   verify-install.mjs [--tarballs <dir>]   the packed dispatcher and native package in <dir>,
//                                           by default dist/npm of the workspace
//   verify-install.mjs --registry <version> @convesoft/mara@<version> from the npm registry
//
// It prints one line per case and exits 1 if any fails, naming the command and its output.

import { execFileSync, spawn, spawnSync } from "node:child_process";
import {
  accessSync,
  constants,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { parseArgs } from "node:util";
import {
  dispatcherContents,
  dispatcherName,
  hostPlatform,
  installedDocsProblems,
  minimumGlibc,
  nativeFiles,
  root,
  tarballName,
  tarballProblems,
  workspace,
} from "./packages.mjs";

const { values: options } = parseArgs({
  options: { tarballs: { type: "string" }, registry: { type: "string" } },
});
if (options.tarballs !== undefined && options.registry !== undefined) {
  console.error("verify-install.mjs: give --tarballs or --registry, not both");
  process.exit(2);
}

const platform = hostPlatform();
const version = options.registry ?? workspace().version;
const tarballDirectory = path.resolve(options.tarballs ?? path.join(root, "dist", "npm"));
const tarballs = {
  dispatcher: path.join(tarballDirectory, tarballName(dispatcherName, version)),
  native: path.join(tarballDirectory, tarballName(platform.name, version)),
};

const work = realpathSync(mkdtempSync(path.join(os.tmpdir(), "mara-verify-")));
// The daemon's socket lies under `XDG_RUNTIME_DIR`, so that directory sits directly in the
// temporary directory, where its path is shortest; `mkdtemp` makes it with mode 0700.
const runtimeDirectory = mkdtempSync(path.join(os.tmpdir(), "mr-"));
const failures = [];

function report(name, problem, details = "") {
  if (problem === undefined) {
    process.stdout.write(`ok      ${name}\n`);
  } else {
    process.stdout.write(`FAILED  ${name}: ${problem}\n${details}`);
    failures.push(name);
  }
}

function indent(text) {
  return text ? text.replace(/^(?=.)/gm, "    ") : "    (empty)\n";
}

function describe(command, args, result) {
  return (
    `  command: ${[command, ...args].join(" ")}\n` +
    `  status: ${result.status ?? result.signal ?? result.error?.message}\n` +
    `  stdout:\n${indent(result.stdout)}\n  stderr:\n${indent(result.stderr)}\n`
  );
}

// Runs a command of the caller's environment, such as `npm install`, and throws on failure.
function setup(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`setup failed\n${describe(command, args, result)}`);
  }
  return result.stdout;
}

function executableOnPath(name, searchPath) {
  for (const directory of searchPath.split(path.delimiter).filter(Boolean)) {
    try {
      accessSync(path.join(directory, name), constants.X_OK);
      return path.join(directory, name);
    } catch {
      // Not in this directory.
    }
  }
  return undefined;
}

// The executable of each running process, from `/proc` where it exists, else from `ps`.
function processExecutables() {
  if (existsSync("/proc/self/exe")) {
    const processes = [];
    for (const entry of readdirSync("/proc")) {
      if (/^\d+$/.test(entry)) {
        try {
          processes.push({ pid: Number(entry), executable: readlinkSync(`/proc/${entry}/exe`) });
        } catch {
          // Ended meanwhile, or not ours to read.
        }
      }
    }
    return processes;
  }
  return execFileSync("ps", ["-A", "-o", "pid=", "-o", "comm="], { encoding: "utf8" })
    .split("\n")
    .map((line) => line.trim().match(/^(\d+)\s+(.*)$/))
    .filter((match) => match !== null)
    .map((match) => ({ pid: Number(match[1]), executable: match[2] }));
}

function runsFromTheVerification(pid) {
  return processExecutables().some(
    (entry) => entry.pid === pid && entry.executable.startsWith(`${work}${path.sep}`),
  );
}

async function processEnds(isRunning, milliseconds) {
  const deadline = Date.now() + milliseconds;
  while (isRunning()) {
    if (Date.now() > deadline) {
      return false;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return true;
}

// What the running case started and has to be gone when it ends: children it still has to
// reap, and daemons by the process ID that their status reports. A child of `spawnSync` has
// ended and been reaped when the call returns, so it is never signalled: its process ID may
// already belong to another process.
let started;

// Runs one case and then checks that nothing it started still runs. A leftover child or daemon
// is ended by its process ID with SIGTERM, awaited, and fails the case. Any other process whose
// executable lies in the verification's directory fails the case and is reported, never signalled.
async function verifyCase(name, body) {
  started = { children: [], daemons: [] };
  let outcome;
  try {
    outcome = (await body()) ?? {};
  } catch (error) {
    outcome = { problem: error.message };
  }
  // Everything found is reported: the case's own failure, each leftover that it ended, and each
  // other process of the verification's directory.
  const problems = outcome.problem === undefined ? [] : [outcome.problem];
  const tracked = new Set();
  const state = (ended) => (ended ? "ended by SIGTERM" : "still runs after SIGTERM");
  for (const child of started.children) {
    if (child.exitCode === null && child.signalCode === null) {
      tracked.add(child.pid);
      child.kill("SIGTERM");
      const ended = await processEnds(
        () => child.exitCode === null && child.signalCode === null,
        5_000,
      );
      problems.push(`child ${child.pid} (${child.spawnfile}) ran after the case, ${state(ended)}`);
    }
  }
  for (const pid of started.daemons) {
    const executable = processExecutables().find((entry) => entry.pid === pid)?.executable;
    if (runsFromTheVerification(pid)) {
      tracked.add(pid);
      process.kill(pid, "SIGTERM");
      const ended = await processEnds(() => runsFromTheVerification(pid), 5_000);
      problems.push(`daemon ${pid} (${executable}) ran after the case, ${state(ended)}`);
    }
  }
  for (const entry of processExecutables()) {
    if (entry.executable.startsWith(`${work}${path.sep}`) && !tracked.has(entry.pid)) {
      problems.push(`process ${entry.pid} (${entry.executable}) runs from the installed packages`);
    }
  }
  const problem = problems.length > 0 ? problems.join("; ") : undefined;
  report(name, problem, outcome.details ?? "");
}

// Installs the dispatcher into a new empty directory with lifecycle scripts off, with its native
// package or without optional dependencies. Tarballs install with npm's offline mode and an empty
// cache of the installation's own, so that npm fetches nothing from the registry.
function install(name, { native }) {
  const directory = path.join(work, name);
  mkdirSync(directory);
  const specs =
    options.registry === undefined
      ? ["--offline", tarballs.dispatcher, ...(native ? [tarballs.native] : [])]
      : [`${dispatcherName}@${version}`];
  const cache = path.join(work, `npm-cache-${name}`);
  const flags = [
    "--ignore-scripts",
    "--no-audit",
    "--no-fund",
    "--no-update-notifier",
    "--cache",
    cache,
  ];
  setup("npm", ["install", ...flags, ...(native ? [] : ["--omit=optional"]), ...specs], directory);
  return directory;
}

function compareVersions(left, right) {
  const a = left.split(".").map(Number);
  const b = right.split(".").map(Number);
  for (let index = 0; index < Math.max(a.length, b.length); index += 1) {
    if ((a[index] ?? 0) !== (b[index] ?? 0)) {
      return (a[index] ?? 0) - (b[index] ?? 0);
    }
  }
  return 0;
}

// Starts `mara --no-daemon mcp`, sends `initialize`, closes its standard input and waits for it to
// end. Settles on the child's `close`, or on its `error` when it cannot start.
function mcpInitialize(mara, cwd, env) {
  const args = ["--no-daemon", "mcp"];
  return new Promise((resolve) => {
    const output = { stdout: "", stderr: "" };
    let problem;
    let answered = false;
    let closedAt;
    let settled = false;
    const settle = (result) => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        resolve({ problem, details: describe(mara, args, { ...output, ...result }) });
      }
    };
    const child = spawn(mara, args, { cwd, env, stdio: ["pipe", "pipe", "pipe"] });
    started.children.push(child);
    const timer = setTimeout(() => {
      problem ??= answered
        ? "did not exit within 5 s of its input closing"
        : "no answer within 60 s";
      child.kill("SIGTERM");
    }, 60_000);
    child.on("error", (error) => {
      problem ??= `could not run: ${error.message}`;
      if (child.pid === undefined) {
        settle({ error });
      }
    });
    child.on("close", (status, signal) => {
      if (problem === undefined && !answered) {
        problem = "no answer to initialize";
      } else if (problem === undefined && status !== 0) {
        problem = `exited with ${status ?? signal}`;
      } else if (problem === undefined && Date.now() - closedAt > 5_000) {
        problem = "did not exit within 5 s of its input closing";
      }
      settle({ status, signal });
    });
    child.stdin.on("error", () => {
      // Reported through `close` or `error`.
    });
    child.stderr.on("data", (chunk) => {
      output.stderr += chunk;
    });
    child.stdout.on("data", (chunk) => {
      output.stdout += chunk;
      if (answered || !output.stdout.includes("\n")) {
        return;
      }
      answered = true;
      let serverInfo;
      try {
        serverInfo = JSON.parse(output.stdout.split("\n")[0]).result?.serverInfo;
      } catch {
        // Reported below.
      }
      if (serverInfo?.name !== "mara" || serverInfo?.version !== version) {
        problem = `expected serverInfo mara ${version}`;
      }
      closedAt = Date.now();
      child.stdin.end();
      clearTimeout(timer);
      setTimeout(() => {
        if (child.exitCode === null && child.signalCode === null) {
          problem ??= "did not exit within 5 s of its input closing";
          child.kill("SIGTERM");
        }
      }, 5_000).unref();
    });
    child.stdin.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: {
          protocolVersion: "2025-06-18",
          capabilities: {},
          clientInfo: { name: "mara-verify-install", version },
        },
      })}\n`,
    );
  });
}

async function main() {
  if (options.registry === undefined) {
    for (const tarball of Object.values(tarballs)) {
      try {
        accessSync(tarball, constants.R_OK);
      } catch {
        throw new Error(
          `no tarball ${tarball}: run nx run mara-npm:pack-dispatcher and ` +
            "nx run mara-npm:pack-native:release first",
        );
      }
    }
  }

  // The runtime environment: a private PATH with links to node and npm only, a new HOME and the
  // private XDG_RUNTIME_DIR.
  const bin = path.join(work, "path");
  const home = path.join(work, "home");
  mkdirSync(bin);
  mkdirSync(home);
  const npm = executableOnPath("npm", process.env.PATH ?? "");
  if (npm === undefined) {
    throw new Error("npm is not on PATH");
  }
  symlinkSync(realpathSync(process.execPath), path.join(bin, "node"));
  symlinkSync(realpathSync(npm), path.join(bin, "npm"));
  const environment = { PATH: bin, HOME: home, XDG_RUNTIME_DIR: runtimeDirectory };
  const socket = path.join(runtimeDirectory, "mara", "0123456789abcdef.sock");
  if (Buffer.byteLength(socket) >= 108) {
    throw new Error(`the daemon's socket path ${socket} is too long; use a shorter TMPDIR`);
  }

  const found = ["cargo", "rustc", "cc", "c++", "g++", "clang", "souffle"].filter((tool) =>
    executableOnPath(tool, environment.PATH),
  );
  if (found.length > 0) {
    report("no toolchain on the runtime PATH", `found ${found.join(", ")} on ${environment.PATH}`);
    return;
  }
  report("no toolchain on the runtime PATH");

  const installed = install("install", { native: true });
  const withoutOptional = install("without-optional", { native: false });
  const mara = (directory) => path.join(directory, "node_modules", ".bin", "mara");
  const run = (args, { cwd = work, env = environment, directory = installed } = {}) => {
    const result = spawnSync(mara(directory), args, {
      cwd,
      env,
      encoding: "utf8",
      timeout: 120_000,
    });
    return { result, details: describe(mara(directory), args, result) };
  };

  await verifyCase("mara --version", () => {
    const { result, details } = run(["--version"]);
    return {
      problem:
        result.status === 0 && result.stdout === `mara ${version}\n`
          ? undefined
          : `expected "mara ${version}" and exit 0`,
      details,
    };
  });

  const project = path.join(work, "project");
  mkdirSync(project);
  let directDiagnostics;
  await verifyCase("project validate evaluates an invariant with the bundled souffle", () => {
    const init = run(["--no-daemon", "project", "init"], { cwd: project });
    const create = run(
      [
        "--no-daemon",
        "item",
        "create",
        "requirement",
        "docs/requirements.mara.md",
        "--title",
        "Run anywhere",
        "--id",
        "REQ-RUN-ANYWHERE",
        "--field",
        "Status=Accepted",
        "--body",
        "Mara runs on every platform.",
      ],
      { cwd: project },
    );
    const at = create.result.stdout.match(/ at (docs\/requirements\.mara\.md:\d+),/)?.[1];
    const validate = run(["--no-daemon", "project", "validate", "--debug-stats"], {
      cwd: project,
    });
    const stats = validate.result.stderr
      .split("\n")
      .map((line) => {
        try {
          return JSON.parse(line);
        } catch {
          return undefined;
        }
      })
      .find((value) => value?.program_runs !== undefined);
    directDiagnostics = validate.result.stdout;
    return {
      problem:
        init.result.status !== 0
          ? "project init failed"
          : create.result.status !== 0 || at === undefined
            ? "item create failed"
            : validate.result.status !== 1
              ? "expected exit 1"
              : !validate.result.stdout.includes(`${at}: error[requirement-has-basis]`)
                ? `no error of requirement-has-basis at ${at}`
                : !(stats?.program_runs >= 1)
                  ? "the stats line counts no program run"
                  : undefined,
      details: init.details + create.details + validate.details,
    };
  });

  await verifyCase("mara mcp answers initialize", () =>
    mcpInitialize(mara(installed), project, environment),
  );

  await verifyCase("the daemon serves a validation", async () => {
    const start = run(["daemon", "start"], { cwd: project });
    const startedPid = Number(start.result.stdout.match(/as process (\d+)/)?.[1]);
    if (startedPid > 0) {
      started.daemons.push(startedPid);
    }
    const validate = run(["project", "validate"], { cwd: project });
    const status = run(["--format", "json", "daemon", "status"], { cwd: project });
    let pid;
    let served;
    try {
      ({ pid, served } = JSON.parse(status.result.stdout));
    } catch {
      // Reported below.
    }
    if (pid > 0 && pid !== startedPid) {
      started.daemons.push(pid);
    }
    const stop = run(["daemon", "stop"], { cwd: project });
    const ended =
      pid !== undefined && (await processEnds(() => runsFromTheVerification(pid), 5_000));
    return {
      problem:
        start.result.status !== 0
          ? "daemon start failed"
          : validate.result.status !== 1 || validate.result.stdout !== directDiagnostics
            ? "the validation's result differs from direct mode's"
            : served !== 1
              ? `served is ${served}, not 1`
              : stop.result.status !== 0
                ? "daemon stop failed"
                : !ended
                  ? `the daemon's process ${pid} still runs 5 s after the stop`
                  : undefined,
      details: start.details + validate.details + status.details + stop.details,
    };
  });

  const refused = (env, description) => {
    const { result, details } = run(["--version"], { directory: withoutOptional, env });
    return {
      problem:
        result.status === 1 &&
        result.stdout === "" &&
        result.stderr.includes(platform.name) &&
        result.stderr.includes("optional dependencies")
          ? undefined
          : "expected exit 1 and a message naming the native package and optional dependencies",
      details: description + details,
    };
  };
  await verifyCase("the dispatcher without its native package", () => refused(environment, ""));
  await verifyCase("the dispatcher refuses a native package found only through NODE_PATH", () => {
    const nodePath = path.join(installed, "node_modules");
    return refused({ ...environment, NODE_PATH: nodePath }, `  NODE_PATH: ${nodePath}\n`);
  });

  if (platform.libc === "glibc") {
    for (const [reported, expected] of [
      ["2.34", [minimumGlibc, "2.34"]],
      [undefined, [`glibc ${minimumGlibc} or later`]],
    ]) {
      await verifyCase(
        `the dispatcher on ${reported === undefined ? "no glibc" : `glibc ${reported}`}`,
        () => {
          const preload = path.join(work, `glibc-${reported ?? "none"}.cjs`);
          writeFileSync(
            preload,
            "const getReport = process.report.getReport.bind(process.report);\n" +
              "process.report.getReport = (...args) => {\n" +
              "  const value = getReport(...args);\n" +
              (reported === undefined
                ? "  delete value.header.glibcVersionRuntime;\n"
                : `  value.header.glibcVersionRuntime = ${JSON.stringify(reported)};\n`) +
              "  return value;\n};\n",
          );
          // NODE_OPTIONS splits at spaces outside double quotes, and reads backslash escapes
          // inside them, as JSON writes a string.
          const nodeOptions = `--require=${JSON.stringify(preload)}`;
          const { result, details } = run(["--version"], {
            env: { ...environment, NODE_OPTIONS: nodeOptions },
          });
          return {
            problem:
              result.status === 1 &&
              result.stdout === "" &&
              expected.every((text) => result.stderr.includes(text))
                ? undefined
                : `expected exit 1, no output of mara and a message naming ${expected.join(" and ")}`,
            details: `  NODE_OPTIONS: ${nodeOptions}\n${details}`,
          };
        },
      );
    }
  }

  // The dispatcher's `docs/` against the files that Git tracks under `docs/` at the checkout's HEAD,
  // which in the release workflow's Build and Registry check jobs is the recorded commit.
  let dispatcher;
  await verifyCase("the installed dispatcher's docs/ equals the checkout's", () => {
    dispatcher = dispatcherContents();
    const docs = path.join(installed, "node_modules", ...dispatcherName.split("/"));
    const problems = installedDocsProblems(docs, dispatcher.blobs);
    return {
      problem: problems.length > 0 ? problems.join("; ") : undefined,
      details: `  compared: ${path.join(docs, "docs")} with docs/ at HEAD of ${root}\n`,
    };
  });

  if (options.registry === undefined) {
    for (const [tarball, files, blobs] of [
      [tarballs.dispatcher, dispatcher?.files, dispatcher?.blobs],
      [tarballs.native, nativeFiles],
    ]) {
      await verifyCase(`the file list of ${path.basename(tarball)}`, () => {
        if (files === undefined) {
          throw new Error("the checkout's docs/ could not be read; see the case above");
        }
        const { problems, command, output } = tarballProblems(tarball, files, blobs);
        return {
          problem: problems.length > 0 ? problems.join("; ") : undefined,
          details: `  command: ${command}\n  stdout:\n${indent(output)}`,
        };
      });
    }
  }

  if (platform.libc === "glibc") {
    const directory = path.join(installed, "node_modules", ...platform.name.split("/"), "bin");
    for (const binary of ["mara", "souffle"]) {
      await verifyCase(`bin/${binary} needs no GLIBC_ version above ${minimumGlibc}`, () => {
        const file = path.join(directory, binary);
        const symbols = execFileSync("objdump", ["-T", file], {
          encoding: "utf8",
          maxBuffer: 256 * 1024 * 1024,
        });
        const newer = (line) =>
          (line.match(/GLIBC_\d+(\.\d+)*/g) ?? []).some(
            (name) => compareVersions(name.slice(6), minimumGlibc) > 0,
          );
        const lines = symbols.split("\n").filter(newer);
        return {
          problem:
            lines.length > 0
              ? `${lines.length} symbols need a GLIBC_ version above ${minimumGlibc}`
              : undefined,
          details:
            `  command: objdump -T ${file}\n` +
            `  stdout, the lines that name a GLIBC_ version above ${minimumGlibc}:\n` +
            indent(`${lines.join("\n")}\n`),
        };
      });
    }
  }
}

try {
  await main();
} catch (error) {
  report("setup", error.message);
}
if (failures.length > 0) {
  process.stdout.write(`\n${failures.length} failed; the files are in ${work}\n`);
  process.exitCode = 1;
} else {
  rmSync(work, { recursive: true, force: true });
  process.stdout.write("\nevery case passed\n");
}
rmSync(runtimeDirectory, { recursive: true, force: true });
