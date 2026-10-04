import { test, expect } from '@playwright/test';

// @mara checks REQ-SHARED-TYPESCRIPT-CLIENT
// @mara checks DES-INTEGRATION-ARTIFACTS
test('strict compiled package consumer runs in Chromium without Node imports', async ({ page }) => {
  await page.goto('/');
  await page.addScriptTag({ type: 'importmap', content: JSON.stringify({ imports: {
    '@convesoft/mara/browser': new URL('./browser.mjs', page.url()).href,
  } }) });
  expect(await page.evaluate(async () => {
    const { typedBrowserUsage } = await import('./usage.mjs');
    await typedBrowserUsage();
    return true;
  })).toBe(true);
});
