// What the install verification's cases of the Node library and the browser workspace use
// (VER-INSTALL-WITHOUT-TOOLCHAIN): the module that runs an operation through the addon, the
// project's files for a workspace, and the loopback server of the installed package's files.

import { lstatSync, readdirSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import { dispatcherName } from "./packages.mjs";

// A module of the installation that imports the Node library by the package's name, runs
// `project validate` of the project in its argument in this process (`noDaemon`), and prints the
// result or the failure with the stats, and the `.node` files that the process has mapped.
export const embeddedScript = `import { readFileSync } from "node:fs";
import { createClient } from "${dispatcherName}";
const client = createClient({ project: process.argv[2], noDaemon: true });
let answer;
try {
  answer = await client.run("project_validate", {}, { debugStats: true });
} catch (error) {
  answer = { error: { code: error.code, message: error.message, reply: error.reply }, stats: error.stats };
}
answer.mapped = readFileSync("/proc/self/maps", "utf8")
  .split("\\n")
  .map((line) => line.split(/\\s+/).slice(5).join(" "))
  .filter((file) => file.endsWith(".node"));
process.stdout.write(JSON.stringify(answer));
`;

// The files of the project at `root` but `.git`, by path relative to it with `/` between parts.
export function projectFiles(root) {
  const files = [];
  const walk = (relative) => {
    for (const entry of readdirSync(path.join(root, relative), { withFileTypes: true })) {
      const name = relative === "" ? entry.name : `${relative}/${entry.name}`;
      if (entry.name === ".git") {
        continue;
      }
      if (entry.isDirectory()) {
        walk(name);
      } else {
        files.push({
          path: name,
          contents: readFileSync(path.join(root, name)).toString("base64"),
        });
      }
    }
  };
  walk("");
  return files;
}

const contentTypes = {
  ".js": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
};

// A server on a loopback port that the system assigns: `/` is `page`, and `/mara/<path>` each
// regular file below `packageRoot`. `requests` holds the path of every request.
export async function startPackageServer(packageRoot, page) {
  const requests = [];
  const server = createServer((request, response) => {
    const pathname = new URL(request.url, "http://127.0.0.1").pathname;
    requests.push(pathname);
    if (pathname === "/") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(page);
      return;
    }
    const relative = pathname.startsWith("/mara/") ? pathname.slice("/mara/".length) : undefined;
    const file = relative === undefined ? undefined : path.join(packageRoot, relative);
    if (
      file === undefined ||
      relative.split("/").some((part) => part === "" || part === "." || part === "..") ||
      !lstatSync(file, { throwIfNoEntry: false })?.isFile()
    ) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, {
      "content-type": contentTypes[path.extname(file)] ?? "application/octet-stream",
    });
    response.end(readFileSync(file));
  });
  const sockets = new Set();
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    url: `http://127.0.0.1:${server.address().port}`,
    requests,
    close: () =>
      new Promise((resolve) => {
        for (const socket of sockets) {
          socket.destroy();
        }
        server.close(resolve);
      }),
  };
}

// Rejects naming `what` after `milliseconds` unless `promise` settles first.
export function within(promise, milliseconds, what) {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => {
      timer = setTimeout(
        () => reject(new Error(`${what} took more than ${milliseconds / 1000} s`)),
        milliseconds,
      );
    }),
  ]).finally(() => clearTimeout(timer));
}
