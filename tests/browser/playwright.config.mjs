import { defineConfig } from '@playwright/test';
import { statSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const require = createRequire(import.meta.url);

if (!process.env.BROWSER_ASSETS_DIR) throw new Error('Set BROWSER_ASSETS_DIR to built disposable assets');
const browserTmp = process.env.BROWSER_RUNTIME_TMPDIR;
if (!browserTmp || !path.isAbsolute(browserTmp) || !statSync(browserTmp, { throwIfNoEntry: false })?.isDirectory()) {
  throw new Error('Set BROWSER_RUNTIME_TMPDIR to an existing private, session-owned runtime directory');
}
if (Buffer.byteLength(path.join(browserTmp, '.org.chromium.Chromium.XXXXXX/SingletonSocket')) > 107) {
  throw new Error('BROWSER_RUNTIME_TMPDIR is too long for Chromium Unix sockets; use a shorter runtime path');
}
const port = Number(process.env.BROWSER_TEST_PORT ?? process.env.BROWSER_PORT ?? '18364');
if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('Invalid browser test port');
const origin = `http://127.0.0.1:${port}`;
export default defineConfig({
  testDir: '.', testMatch: process.env.BROWSER_TEST_MATCH ?? '*.spec.mjs', timeout: 120_000, workers: 1,
  reporter: [['list'], ['json', { outputFile: process.env.BROWSER_REPORT }]],
  outputDir: process.env.BROWSER_RESULTS,
  use: {
    browserName: 'chromium', headless: true, baseURL: origin,
    launchOptions: { executablePath: process.env.CHROMIUM_PATH ?? '/usr/bin/chromium',
      env: { ...process.env, TMPDIR: browserTmp },
      args: ['--disable-dev-shm-usage'] },
  },
  webServer: {
    command: `"${process.execPath}" "${require.resolve('serve/build/main.js')}" --no-clipboard --listen tcp://127.0.0.1:${port} "${process.env.BROWSER_ASSETS_DIR}"`,
    env: { NO_UPDATE_CHECK: '1' },
    stdout: 'pipe', stderr: 'pipe',
    url: `${origin}/`, reuseExistingServer: false, timeout: 30_000,
  },
});
