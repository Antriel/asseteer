/**
 * The suite's `test` — a Playwright test bound to the real app, plus the console-error gate.
 *
 * Every spec gets:
 *  1. **`app`** — arrangement helpers from `tests/harness/appApi.mjs`, shared with the
 *     one-shot driver agents use, so the two never drift apart. Hooks are for
 *     arrangement; **real clicks and typing are for the thing under test**.
 *  2. **The console-error gate** — a spec fails on any `console.error`, uncaught error or
 *     unhandled rejection. Search failures, for one, only ever surface in the console.
 *     If it goes red for a reason nobody cares about, fix the cause or add it to
 *     `IGNORED_ERRORS` in `appApi.mjs` with a reason — do not mute the gate.
 */

import { test as base, expect } from '@playwright/test';
import path from 'node:path';
import { attachToApp } from '../harness/launch.mjs';
import { createApp, filterErrors, formatErrors, resetApp } from '../harness/appApi.mjs';
import { recordShot } from '../harness/contactSheet.mjs';

export const test = base.extend({
  // Worker-scoped: one CDP attachment for the whole run. globalSetup owns the app.
  attached: [
    async ({}, use) => {
      const attached = await attachToApp();
      await use(attached);
      await attached.browser.close();
    },
    { scope: 'worker' },
  ],

  // Specs drive the app's existing webview, not a fresh context (which under CDP would
  // be a blank tab with no Tauri IPC at all).
  page: async ({ attached }, use) => {
    await use(attached.page);
  },

  app: async ({ page }, use, testInfo) => {
    await resetApp(page);

    const app = createApp({
      page,
      onShot: (file, name) => {
        recordShot({ file, label: name, sub: testInfo.title });
        return testInfo.attach(path.basename(file), { path: file, contentType: 'image/png' });
      },
    });
    await app.clearConsoleErrors();

    await use(app);

    // The failing screen, caught while it still is the failing screen. Lands in
    // `tests/harness/out/`, where the run's contact sheet picks it up.
    const slug = testInfo.title.replace(/\W+/g, '-').slice(0, 60);
    if (testInfo.status !== testInfo.expectedStatus) {
      await app.shot(`failure-${slug}`).catch(() => {});
    }

    // ── the gate ─────────────────────────────────────────────────────────────────
    let errors = [];
    try {
      errors = await app.consoleErrors();
    } catch {
      // The page went away — nothing to read.
    }
    const real = filterErrors(errors);
    if (real.length) await app.shot(`console-errors-${slug}`).catch(() => {});

    expect(formatErrors(real), 'console-error gate: the app logged errors during this spec').toEqual([]);
  },
});

export { expect };
