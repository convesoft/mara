// The browser of the host tests (VER-HOST-COMPARISON): Playwright's Chromium, at the path that
// `chromium.executablePath()` gives for the pinned Playwright, launched by that path. Run as a
// script, the module prints what identifies it, for the key of `mara:test`: Playwright's version,
// the path, and the executable's size and modification time, or that it is missing. Playwright
// replaces a revision's executable only by unpacking a new download, which gives it a new
// modification time, so the three tell a removed or replaced browser without hashing its
// hundreds of megabytes on every key.

import { statSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

/** Playwright's library, or an error that names it. */
async function playwright() {
  try {
    return await import("playwright");
  } catch (error) {
    throw new Error(
      `the browser cases need Playwright's library: run \`pnpm install --frozen-lockfile\` (${error.message})`,
    );
  }
}

/** The path of the Chromium that the tests launch. */
export async function browserExecutable() {
  const { chromium } = await playwright();
  return chromium.executablePath();
}

/**
 * Chromium, launched from [`browserExecutable`] with a new profile in `TMPDIR`, never as a
 * persistent context. A missing browser fails with a message that names it.
 */
export async function launchBrowser() {
  const { chromium } = await playwright();
  const executablePath = chromium.executablePath();
  try {
    statSync(executablePath);
  } catch {
    throw new Error(
      `the browser cases need Playwright's Chromium at ${executablePath}: run \`pnpm exec playwright install chromium\``,
    );
  }
  return chromium.launch({ executablePath });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const require = createRequire(import.meta.url);
  const version = require("playwright/package.json").version;
  const path = await browserExecutable();
  let identity;
  try {
    const stat = statSync(path);
    identity = `${stat.size} ${stat.mtimeMs}`;
  } catch {
    identity = "missing";
  }
  console.log(`playwright ${version}\n${path}\n${identity}`);
}
