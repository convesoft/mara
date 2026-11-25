// What the tests of the release workflow's scripts (.github/scripts) share: loopback stubs of npm's
// registry and of GitHub's API, packed tarballs, temporary directories and an environment that
// reaches nothing but the stubs (DES-RELEASE-WORKFLOW).

import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { releasePackages, root } from "../scripts/packages.mjs";

export const scripts = path.join(root, ".github", "scripts");
// A version without a prerelease part: npm 12 refuses a dry run only of such a published version.
export const version = "0.0.1";
export const packages = releasePackages(version);
export const natives = packages.filter((entry) => entry.native);
export const others = packages.filter((entry) => !entry.native);

export const sha1 = (buffer) => createHash("sha1").update(buffer).digest("hex");
export const sha256 = (buffer) => createHash("sha256").update(buffer).digest("hex");

// A new directory under TMPDIR, removed after the test.
export function temporaryDirectory(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mara-release-"));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  return directory;
}

// The environment of a script: the caller's PATH and toolchain homes, a HOME, npm configuration,
// cache and Git configuration of the test's own, and none of the caller's npm, Git, GitHub, proxy
// or CI variables, so that nothing reads the maintainer's npm login or a runner's token.
export function environment(directory, extra = {}) {
  const home = path.join(directory, "home");
  fs.mkdirSync(home, { recursive: true });
  const realHome = os.homedir();
  const runnerTemp = path.join(directory, "runner-temp");
  fs.mkdirSync(runnerTemp, { recursive: true });
  return {
    PATH: process.env.PATH,
    LANG: "C.UTF-8",
    TMPDIR: os.tmpdir(),
    HOME: home,
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? path.join(realHome, ".rustup"),
    CARGO_HOME: process.env.CARGO_HOME ?? path.join(realHome, ".cargo"),
    XDG_CONFIG_HOME: path.join(home, ".config"),
    npm_config_userconfig: path.join(home, ".npmrc"),
    npm_config_globalconfig: path.join(home, "npmrc-global"),
    npm_config_cache: path.join(home, ".npm"),
    npm_config_update_notifier: "false",
    npm_config_audit: "false",
    npm_config_fund: "false",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_AUTHOR_NAME: "Test",
    GIT_AUTHOR_EMAIL: "test@example.invalid",
    GIT_COMMITTER_NAME: "Test",
    GIT_COMMITTER_EMAIL: "test@example.invalid",
    RUNNER_TEMP: runnerTemp,
    GITHUB_OUTPUT: path.join(directory, "github-output"),
    NPM_REGISTRY_RETRY_SECONDS: "0",
    RELEASE_POLL_SECONDS: "0",
    ...extra,
  };
}

// Runs a script of .github/scripts, or the script at an absolute path, to its end: its exit status
// and output. The script may exit before it reads its standard input: writing it then fails with
// EPIPE, which is ignored, and the result is the exit status and output. A script that cannot be
// started rejects.
export function run(script, args, { cwd, env, input = "" }) {
  return new Promise((resolve, reject) => {
    const child = spawn(path.resolve(scripts, script), args, { cwd, env });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.stdin.on("error", (error) => {
      if (error.code !== "EPIPE") {
        reject(error);
      }
    });
    child.on("error", reject);
    child.on("close", (status) => resolve({ status, stdout, stderr, output: stdout + stderr }));
    child.stdin.end(input);
  });
}

// The `key=value` lines that a script wrote to GITHUB_OUTPUT.
export function outputs(env) {
  if (!fs.existsSync(env.GITHUB_OUTPUT)) {
    return {};
  }
  return Object.fromEntries(
    fs
      .readFileSync(env.GITHUB_OUTPUT, "utf8")
      .split("\n")
      .filter(Boolean)
      .map((line) => [line.slice(0, line.indexOf("=")), line.slice(line.indexOf("=") + 1)]),
  );
}

// Packs a package of the version with `npm pack`, with `bin/mara` printing `printed` when given,
// and returns the tarball's bytes. `salt` makes another tarball of the same name and version.
export function pack(directory, name, { printed, salt = "" } = {}) {
  const source = fs.mkdtempSync(path.join(directory, "package-"));
  const manifest = { name, version, description: `test ${salt}`, license: "MIT" };
  if (printed !== undefined) {
    manifest.bin = { mara: "bin/mara.cjs" };
    fs.mkdirSync(path.join(source, "bin"));
    fs.writeFileSync(
      path.join(source, "bin", "mara.cjs"),
      `#!/usr/bin/env node\nconsole.log(${JSON.stringify(printed)});\n`,
      { mode: 0o755 },
    );
  }
  fs.writeFileSync(path.join(source, "package.json"), JSON.stringify(manifest));
  const output = execFileSync(
    "npm",
    ["pack", "--ignore-scripts", "--json", "--pack-destination", source],
    {
      cwd: source,
      env: environment(fs.mkdtempSync(path.join(directory, "npm-"))),
      encoding: "utf8",
    },
  );
  // npm 11 prints a list of packages, npm 12 an object keyed by the package's name.
  const { filename } = Object.values(JSON.parse(output))[0];
  return fs.readFileSync(path.join(source, filename));
}

// A directory holding a tarball of every package of the version, named as `npm pack` names it.
export function tarballDirectory(directory, { printed } = {}) {
  const tarballs = path.join(directory, "tarballs");
  fs.mkdirSync(tarballs, { recursive: true });
  for (const entry of packages) {
    fs.writeFileSync(
      path.join(tarballs, entry.tarball),
      pack(directory, entry.name, entry.native ? {} : { printed }),
    );
  }
  return tarballs;
}

function readBody(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => resolve(Buffer.concat(chunks)));
    request.on("error", reject);
  });
}

function listen(server) {
  return new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => resolve(server.address().port)),
  );
}

function json(response, status, value) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(typeof value === "string" ? value : JSON.stringify(value));
}

// A loopback stub of npm's registry: package documents, tarballs and `npm publish`'s PUT, which
// needs the token `test-token`. Each request is logged as `METHOD name`.
export async function startRegistry(t) {
  const documents = new Map();
  const queued = new Map();
  const afterPublish = new Map();
  const log = [];
  let base;
  const server = http.createServer(async (request, response) => {
    const body = await readBody(request);
    const url = new URL(request.url, base);
    const tarballPath = url.pathname.match(/^\/(.+)\/-\/([^/]+\.tgz)$/);
    const name = decodeURIComponent(tarballPath ? tarballPath[1] : url.pathname.slice(1));
    log.push(`${request.method} ${name}${tarballPath ? ` ${tarballPath[2]}` : ""}`);
    const document = documents.get(name);
    if (tarballPath) {
      const entry =
        document &&
        Object.values(document.versions).find((candidate) => candidate.file === tarballPath[2]);
      if (entry === undefined) {
        return json(response, 404, { error: "Not found" });
      }
      response.writeHead(200, { "content-type": "application/octet-stream" });
      return response.end(entry.tarball);
    }
    if (request.method === "GET") {
      const answers = queued.get(name) ?? [];
      if (answers.length > 0) {
        const answer = answers.shift();
        return json(response, answer.status, answer.body);
      }
      if (document === undefined) {
        return json(response, 404, { error: "Not found" });
      }
      const versions = {};
      for (const [key, entry] of Object.entries(document.versions)) {
        if (entry.hiddenReads > 0) {
          entry.hiddenReads -= 1;
          continue;
        }
        versions[key] = {
          ...entry.manifest,
          dist: {
            shasum: entry.shasum,
            integrity: entry.integrity,
            tarball: `${base}/${name}/-/${entry.file}`,
          },
        };
      }
      const tags = document.taggedReads > 0 ? document.oldTags : document.tags;
      document.taggedReads = Math.max(0, (document.taggedReads ?? 0) - 1);
      return json(response, 200, { name, "dist-tags": tags, versions });
    }
    if (request.method === "PUT") {
      if (request.headers.authorization !== "Bearer test-token") {
        return json(response, 401, { error: "unauthorized" });
      }
      const published = JSON.parse(body.toString("utf8"));
      const [key] = Object.keys(published.versions);
      if (document?.versions[key] !== undefined) {
        return json(response, 403, {
          error: "You cannot publish over the previously published versions",
        });
      }
      const [file, attachment] = Object.entries(published._attachments)[0];
      const tarball = Buffer.from(attachment.data, "base64");
      registry.add(name, published.versions[key], tarball, { hiddenReads: registry.hiddenReads });
      const stored = documents.get(name);
      stored.versions[key].file = file.replace(/^.*\//, "");
      Object.assign(stored.tags, published["dist-tags"]);
      registry.answer(name, ...(afterPublish.get(name) ?? []));
      return json(response, 200, { ok: true });
    }
    return json(response, 405, { error: "method not allowed" });
  });
  const port = await listen(server);
  base = `http://127.0.0.1:${port}`;
  t.after(() => server.close());
  const registry = {
    url: base,
    log,
    // How many reads a version that `npm publish` sends stays absent, as on npm's replicas.
    hiddenReads: 0,
    // Adds a version, as published before the test, with `shasum` in place of the tarball's.
    add(name, manifest, tarball, { shasum, hiddenReads = 0 } = {}) {
      const document = documents.get(name) ?? { versions: {}, tags: {} };
      documents.set(name, document);
      document.versions[manifest.version] = {
        manifest: {
          name,
          version: manifest.version,
          ...(manifest.bin ? { bin: manifest.bin } : {}),
        },
        tarball,
        file: `${name.replace(/^.*\//, "")}-${manifest.version}.tgz`,
        shasum: shasum ?? sha1(tarball),
        integrity: `sha512-${createHash("sha512").update(tarball).digest("base64")}`,
        hiddenReads,
      };
    },
    // Sets a package's dist-tags; with `reads`, the next that many reads see `old` instead.
    tags(name, tags, { old, reads = 0 } = {}) {
      const document = documents.get(name) ?? { versions: {}, tags: {} };
      documents.set(name, document);
      document.tags = tags;
      document.oldTags = old;
      document.taggedReads = reads;
    },
    // Answers the next reads of a package with these {status, body}, in order.
    answer(name, ...answers) {
      queued.set(name, [...(queued.get(name) ?? []), ...answers]);
    },
    // Answers the reads of a package that follow its publication with these, in order.
    answerAfterPublish(name, ...answers) {
      afterPublish.set(name, answers);
    },
    shasum(name, key) {
      return documents.get(name)?.versions[key]?.shasum;
    },
    puts() {
      return log.filter((line) => line.startsWith("PUT ")).map((line) => line.slice(4));
    },
  };
  return registry;
}

// The npm configuration of a test that publishes to the registry stub: its token alone.
export function npmLogin(env, registry) {
  const host = registry.url.replace(/^http:/, "");
  fs.writeFileSync(env.npm_config_userconfig, `${host}/:_authToken=test-token\n`);
  return { ...env, NPM_REGISTRY: registry.url };
}

// A loopback stub of GitHub's REST API for the repository `o/r`, reached as gh reaches
// `github.localhost`: through an HTTP proxy, which is the stub, at `api.github.localhost` and
// `uploads.github.localhost`. It refuses every CONNECT, which an HTTPS request through the proxy
// would send, and logs it. Each request is logged as `METHOD path`.
export async function startGitHub(t) {
  const releases = new Map();
  const log = [];
  let nextId = 1;
  let lookups = [];
  const releaseJson = (release) => ({
    id: release.id,
    tag_name: release.tag,
    name: release.tag,
    prerelease: release.prerelease,
    upload_url: `http://uploads.github.localhost/repos/o/r/releases/${release.id}/assets{?name,label}`,
    assets: release.assets.map((asset) => ({
      id: asset.id,
      name: asset.name,
      state: asset.state,
      size: asset.content.length,
      ...(asset.digest === null
        ? {}
        : { digest: asset.digest ?? `sha256:${sha256(asset.content)}` }),
      url: `http://api.github.localhost/repos/o/r/releases/assets/${asset.id}`,
      browser_download_url: `http://github.localhost/o/r/releases/download/${release.tag}/${asset.name}`,
    })),
  });
  const findAsset = (id) => {
    for (const release of releases.values()) {
      const asset = release.assets.find((candidate) => candidate.id === id);
      if (asset !== undefined) {
        return { release, asset };
      }
    }
    return undefined;
  };
  const server = http.createServer(async (request, response) => {
    const body = await readBody(request);
    const url = new URL(request.url, "http://api.github.localhost");
    log.push(`${request.method} ${url.host}${url.pathname}`);
    let match;
    if (request.method === "POST" && url.pathname === "/graphql") {
      return json(response, 404, { message: "Not Found" });
    }
    if ((match = url.pathname.match(/^\/repos\/o\/r\/releases\/tags\/(.+)$/))) {
      const status = lookups.shift();
      if (status !== undefined) {
        return json(response, status, { message: "stubbed" });
      }
      const release = releases.get(decodeURIComponent(match[1]));
      return release === undefined
        ? json(response, 404, { message: "Not Found" })
        : json(response, 200, releaseJson(release));
    }
    if (request.method === "POST" && url.pathname === "/repos/o/r/releases") {
      const { tag_name: tag, prerelease } = JSON.parse(body.toString("utf8"));
      github.release(tag, { prerelease });
      return json(response, 201, releaseJson(releases.get(tag)));
    }
    if ((match = url.pathname.match(/^\/repos\/o\/r\/releases\/(\d+)\/assets$/))) {
      const release = [...releases.values()].find((candidate) => candidate.id === Number(match[1]));
      const name = url.searchParams.get("name");
      const asset = { id: nextId++, name, state: "uploaded", content: body };
      release.assets.push(asset);
      return json(response, 201, releaseJson(release).assets.at(-1));
    }
    if ((match = url.pathname.match(/^\/repos\/o\/r\/releases\/assets\/(\d+)$/))) {
      const found = findAsset(Number(match[1]));
      if (found === undefined) {
        return json(response, 404, { message: "Not Found" });
      }
      if (request.method === "DELETE") {
        found.release.assets = found.release.assets.filter((asset) => asset !== found.asset);
        response.writeHead(204);
        return response.end();
      }
      response.writeHead(200, { "content-type": "application/octet-stream" });
      return response.end(found.asset.content);
    }
    return json(response, 404, { message: "Not Found" });
  });
  server.on("connect", (request, socket) => {
    log.push(`CONNECT ${request.url}`);
    socket.end("HTTP/1.1 403 Forbidden\r\n\r\n");
  });
  const port = await listen(server);
  t.after(() => server.close());
  const proxy = `http://127.0.0.1:${port}`;
  const github = {
    log,
    // The environment that sends gh and curl to the stub: the Tag job's variables, with the API at
    // `api.github.localhost`, and every proxy variable naming the stub.
    env: {
      GH_TOKEN: "test-token",
      GH_HOST: "github.localhost",
      GH_REPO: "o/r",
      GH_NO_UPDATE_NOTIFIER: "1",
      GH_PROMPT_DISABLED: "1",
      // gh sends telemetry from a detached process to https://cafe.github.com, which would reach
      // the stub as a CONNECT, after or during the test's assertions.
      GH_TELEMETRY: "false",
      GITHUB_API_URL: "http://api.github.localhost",
      http_proxy: proxy,
      HTTP_PROXY: proxy,
      https_proxy: proxy,
      HTTPS_PROXY: proxy,
    },
    // Adds a release of the tag, with assets of {name, content, state, digest}; a digest of null
    // leaves it out, as GitHub did for assets uploaded before it computed them.
    release(tag, { prerelease = false, assets = [] } = {}) {
      releases.set(tag, {
        id: nextId++,
        tag,
        prerelease,
        assets: assets.map((asset) => ({ id: nextId++, state: "uploaded", ...asset })),
      });
    },
    assets(tag) {
      return releases.get(tag)?.assets ?? [];
    },
    // Answers the next lookups of a release by tag with these statuses, in order.
    lookups(...statuses) {
      lookups = statuses;
    },
  };
  return github;
}

// Runs git in a directory with the test's environment.
export function git(cwd, env, ...args) {
  return execFileSync("git", args, { cwd, env, encoding: "utf8" }).trim();
}
