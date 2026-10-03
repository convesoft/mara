import { defineConfig } from '@playwright/test';

if (!process.env.BROWSER_ASSETS_DIR) throw new Error('Set BROWSER_ASSETS_DIR to built disposable assets');
export default defineConfig({
  testDir: '.', testMatch: 'workspace.spec.mjs', timeout: 120_000, workers: 1,
  reporter: [['list'], ['json', { outputFile: process.env.BROWSER_REPORT }]],
  outputDir: process.env.BROWSER_RESULTS,
  use: {
    browserName: 'chromium', headless: true,
    launchOptions: { executablePath: process.env.CHROMIUM_PATH ?? '/usr/bin/chromium',
      args: ['--disable-dev-shm-usage'] },
  },
  webServer: {
    command: `python -m http.server 18364 --bind 127.0.0.1 --directory "${process.env.BROWSER_ASSETS_DIR}"`,
    url: 'http://127.0.0.1:18364/', reuseExistingServer: false, timeout: 30_000,
  },
});
