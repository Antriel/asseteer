/**
 * E2E against the real Tauri app over CDP. `npm run test:e2e`. See `tests/CLAUDE.md`.
 *
 * Serial and single-worker on purpose: there is exactly one app instance, launched by
 * `globalSetup`, and specs share it. That is the cost of testing the real thing rather
 * than a headless re-implementation of it; extra workers would fight over one window.
 */

import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: '**/*.spec.mjs',
  globalSetup: './tests/harness/globalSetup.mjs',

  fullyParallel: false,
  workers: 1,
  // A retry on a single shared app instance would only hide a state leak between specs.
  retries: 0,
  forbidOnly: true,

  // Scanning + processing the fixture library happens inside the first spec.
  timeout: 60_000,
  expect: { timeout: 10_000 },

  reporter: [['list']],

  use: {
    // No browser launch options: `page` is overridden in `tests/e2e/asseteer.mjs` to be
    // the app's own webview, so Playwright never opens a browser of its own.
    // A red spec leaves `test-results/<spec>/test-failed-1.png` and a `trace.zip` whose
    // screencast frames are the real app (`npx playwright show-trace <path>`).
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
});
