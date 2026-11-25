// The Node library's setup (VER-NODE-LIBRARY, VER-HOST-COMPARISON): `@convesoft/mara` and the
// native package of this platform installed into a folder of the test, as npm installs them, with
// the build's `mara`, Soufflé and the addon; a client of that installation in a Node process of
// its own; and the daemons of the test, which run in its runtime directory (VER-DAEMON-START).

import assert from "node:assert/strict";
import { execFileSync, fork, spawnSync } from "node:child_process";
import {
  chmodSync,
  constants,
  copyFileSync,
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { testDir } from "./hosts.mjs";

/** A variable that `hosts/node.rs` sets. */
function env(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is not set: run these modules through \`cargo test --test hosts\``);
  }
  return value;
}

const maraBin = env("MARA_BIN");
const dispatcherDir = env("MARA_DISPATCHER");
const souffle = env("MARA_SOUFFLE");
const addon = process.env.MARA_NODE_ADDON;

/** The runtime directory of the test's daemons: `XDG_RUNTIME_DIR` of every client. */
export const runtime = join(testDir, "run");
mkdirSync(runtime, { recursive: true, mode: 0o700 });

/** A `HOME` of the test's own, so a per-user cache that is empty. */
export const home = join(testDir, "home");
mkdirSync(home, { recursive: true });

const require = createRequire(import.meta.url);
const { platforms } = require(join(dispatcherDir, "bin", "mara.cjs"));
/** The files of the Node library in the dispatcher, and its manifest's members for them. */
const { libraryFiles, libraryManifest } = await import(
  join(dispatcherDir, "scripts", "packages.mjs")
);

/** The version that the installed packages carry, the build's own. */
const version = execFileSync(maraBin, ["--version"], { encoding: "utf8" }).trim().split(" ")[1];

/** The native package's name for this platform. */
const nativeName = platforms.find(
  (platform) => platform.os === process.platform && platform.cpu === process.arch,
)?.name;

/** Links `from` to `to`, or copies it where no link can be made, such as across filesystems. */
function linkOrCopy(from, to) {
  try {
    linkSync(from, to);
  } catch {
    copyFileSync(from, to);
  }
}

/**
 * Installs `@convesoft/mara` with the Node library and the native package of this platform into
 * `dir/node_modules`, as npm installs them, at the build's version. The native package holds the
 * addon, Soufflé and, unless `binary` is false, a copy of the build's `mara` of its own, whose
 * mode and place the test may change without touching the build's. Returns the installation's
 * folders and its `mara`.
 */
export function install(dir, { binary = true } = {}) {
  if (addon === undefined || !existsSync(addon)) {
    throw new Error(
      `the Node cases need the built addon at ${addon}: build it with \`nx run mara-node:build\``,
    );
  }
  assert.ok(nativeName, `no native package for ${process.platform} ${process.arch}`);
  const modules = join(dir, "node_modules", "@convesoft");
  const dispatcher = join(modules, "mara");
  const native = join(modules, nativeName.split("/")[1]);
  for (const folder of [join(dispatcher, "bin"), join(native, "bin")]) {
    mkdirSync(folder, { recursive: true });
  }
  writeFileSync(
    join(dispatcher, "package.json"),
    JSON.stringify({
      name: "@convesoft/mara",
      version,
      bin: { mara: "bin/mara.cjs" },
      engines: { node: ">=24" },
      optionalDependencies: { [nativeName]: version },
      ...libraryManifest,
    }),
  );
  copyFileSync(join(dispatcherDir, "bin", "mara.cjs"), join(dispatcher, "bin", "mara.cjs"));
  for (const file of Object.keys(libraryFiles)) {
    mkdirSync(dirname(join(dispatcher, file)), { recursive: true });
    copyFileSync(join(dispatcherDir, file), join(dispatcher, file));
  }
  writeFileSync(join(native, "package.json"), JSON.stringify({ name: nativeName, version }));
  linkOrCopy(addon, join(native, "bin", "mara.node"));
  linkOrCopy(souffle, join(native, "bin", "souffle"));
  const mara = join(native, "bin", "mara");
  if (binary) {
    // A file of its own, so that a mode the test sets leaves the build's binary as it is.
    copyFileSync(maraBin, mara, constants.COPYFILE_FICLONE);
    chmodSync(mara, 0o755);
  }
  // A module of the test that imports the library by its name, from the installation.
  writeFileSync(join(dir, "library.mjs"), 'export * from "@convesoft/mara";\n');
  return { dir, dispatcher, native, mara };
}

/** `PATH` without the folders that hold a `mara`, so that none is on it. */
export function pathWithoutMara() {
  return (process.env.PATH ?? "")
    .split(delimiter)
    .filter((folder) => folder && !existsSync(join(folder, "mara")))
    .join(delimiter);
}

/** The environment of a client of the test: its runtime directory and `HOME`, and `extra`. */
export function clientEnv(extra = {}) {
  const base = {
    PATH: process.env.PATH ?? "",
    HOME: home,
    XDG_RUNTIME_DIR: runtime,
    TMPDIR: process.env.TMPDIR,
  };
  return Object.fromEntries(
    Object.entries({ ...base, ...extra }).filter(([, value]) => value !== undefined),
  );
}

const childScript = fileURLToPath(new URL("./library-child.mjs", import.meta.url));

/**
 * A client of the installation `installation`, created with `options`, in a Node process of its
 * own that runs in `cwd` with the environment `env`: `run(name, args, options, { timed })`
 * resolves with `{ result }`, or `{ error }` with the `MaraError`'s members, `promised`, whether
 * the call returned a promise without throwing, and for a timed call `times`
 * (`library-child.mjs`). `close()` ends the process.
 */
export function startClient(installation, { options = {}, cwd = installation.dir, env = clientEnv() } = {}) {
  const child = fork(childScript, [join(installation.dir, "library.mjs"), JSON.stringify(options)], {
    cwd,
    env,
    stdio: ["ignore", "inherit", "inherit", "ipc"],
  });
  let next = 0;
  const pending = new Map();
  child.on("message", (message) => {
    const resolve = pending.get(message.id);
    pending.delete(message.id);
    resolve(message);
  });
  child.on("exit", (code, signal) => {
    for (const resolve of pending.values()) {
      resolve({ error: { exited: `the client's process ended (${code ?? signal})` } });
    }
    pending.clear();
  });
  return {
    pid: child.pid,
    run(name, args, options, { timed = false } = {}) {
      const id = next++;
      return new Promise((resolve) => {
        pending.set(id, resolve);
        child.send({ id, name, args, options, timed });
      });
    },
    close: () =>
      new Promise((resolve) => {
        if (child.exitCode !== null || child.signalCode !== null) {
          resolve();
          return;
        }
        child.once("exit", () => resolve());
        child.disconnect();
      }),
  };
}

/** `mara daemon <args>` of the binary `mara` with the test's runtime directory, which must succeed. */
export function daemon(mara, args, extra = {}) {
  const ran = spawnSync(mara, ["--format", "json", "daemon", ...args], {
    cwd: testDir,
    env: clientEnv(extra),
    encoding: "utf8",
  });
  assert.equal(ran.status, 0, `mara daemon ${args.join(" ")}: ${ran.stderr}${ran.stdout}`);
  return JSON.parse(ran.stdout);
}

/** `mara daemon status` of the binary `mara`. */
export const status = (mara) => daemon(mara, ["status"]);

/**
 * The daemons of the test that run: processes whose environment holds its runtime directory
 * and whose executable is a `mara` (VER-DAEMON-START). Linux only: it reads `/proc`.
 */
export function daemonsOfTest() {
  const entry = `XDG_RUNTIME_DIR=${runtime}`;
  const found = [];
  for (const name of readdirSync("/proc")) {
    if (!/^\d+$/.test(name)) {
      continue;
    }
    let executable;
    let environment;
    try {
      executable = readlinkSync(`/proc/${name}/exe`).replace(/ \(deleted\)$/, "");
      environment = readFileSync(`/proc/${name}/environ`, "latin1");
    } catch {
      continue;
    }
    if (executable.endsWith("/mara") && environment.split("\0").includes(entry)) {
      found.push(Number(name));
    }
  }
  return found;
}

/** Waits until `check()` returns a value other than undefined, for at most 10 s, polling. */
export async function waitFor(what, check) {
  const deadline = Date.now() + 10_000;
  for (;;) {
    const value = check();
    if (value !== undefined) {
      return value;
    }
    assert.ok(Date.now() < deadline, `waited 10 s for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

/** Stops every daemon of the test, by `mara daemon stop` of each binary of `binaries`. */
export async function stopDaemons(binaries) {
  for (const mara of binaries) {
    if (existsSync(mara)) {
      daemon(mara, ["stop"]);
    }
  }
  await waitFor("no daemon of the test", () => (daemonsOfTest().length === 0 ? true : undefined));
}
