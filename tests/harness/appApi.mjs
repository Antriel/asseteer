/**
 * The `app` helper object — arrangement, screenshots, and the console-error list.
 *
 * Shared by the Playwright suite (`tests/e2e/asseteer.mjs`) and the one-shot driver
 * agents write scratch scripts against (`drive.mjs`), so a helper added for one is
 * immediately available to the other.
 *
 * **Hooks are for arrangement; real clicks are for the thing under test.**
 */

import path from 'node:path';
import { mkdirSync } from 'node:fs';
import { LIBRARY } from '../fixtures/makeLibrary.mjs';
import { gotoFrontend } from './launch.mjs';
import { SHOT_DIR } from './paths.mjs';

/**
 * Console errors that are known noise rather than a defect under test.
 *
 * Every entry is a hole in the gate, so each needs a reason and, ideally, a bean.
 */
const IGNORED_ERRORS = [];

/** Drop known-noise entries from a `consoleErrors()` list. */
export function filterErrors(errors) {
  return errors.filter((e) => !IGNORED_ERRORS.some((rx) => rx.test(e.message)));
}

/** Format an error list for a terminal or a failure message. */
export function formatErrors(errors) {
  return errors.map((e) => `[${e.source}] ${e.message}`);
}

const samePath = (a, b) => a.replace(/\\/g, '/').toLowerCase() === b.replace(/\\/g, '/').toLowerCase();

/**
 * Evaluate, tolerating one reload underneath us. Vite re-optimizes deps the first time
 * a run reaches a page that imports a new one and forces a full reload, which presents
 * as "Execution context was destroyed". That is the dev server working as designed.
 */
export async function evaluateThroughReload(page, fn, arg) {
  try {
    return await page.evaluate(fn, arg);
  } catch (err) {
    if (!/Execution context was destroyed|frame was detached/i.test(String(err))) throw err;
    await page.waitForLoadState('domcontentloaded');
    await page.waitForFunction(() => Boolean(window.asseteerTest));
    return page.evaluate(fn, arg);
  }
}

/**
 * Put the UI back to a known state: no persisted settings/view state, fresh module
 * state, back at the root route (which redirects to /library or /sources).
 *
 * The library (the SQLite data) is *not* reset here — it is wiped on every cold launch
 * instead (`startApp({ freshData })`). `localStorage.clear()` is only safe because the
 * harness app runs on its own webview profile (`PROFILE_DIR`).
 */
export async function resetApp(page) {
  await evaluateThroughReload(page, () => {
    localStorage.clear();
    sessionStorage.clear();
  });
  await gotoFrontend(page, '/');
  await page.waitForFunction(() => Boolean(window.asseteerTest), undefined, { timeout: 30_000 });
  await page.emulateMedia({ colorScheme: null });
}

/**
 * Build the `app` helper bound to one page.
 *
 * @param {object} args
 * @param {import('playwright').Page} args.page  The app's main webview.
 * @param {(file: string, name: string) => void | Promise<void>} [args.onShot]
 */
export function createApp({ page, onShot }) {
  const app = {
    /** Absolute path of the generated fixture library. */
    library: LIBRARY,

    state: () => page.evaluate(() => window.asseteerTest.state()),
    consoleErrors: () => page.evaluate(() => window.asseteerTest.consoleErrors()),
    clearConsoleErrors: () => page.evaluate(() => window.asseteerTest.clearConsoleErrors()),
    folders: () => page.evaluate(() => window.asseteerTest.folders()),

    /** Client-side navigation (`/library`, `/processing`, `/sources`, `/settings`). */
    async navigate(route) {
      await evaluateThroughReload(page, (r) => window.asseteerTest.navigate(r), route);
      await page.waitForFunction((r) => window.asseteerTest.state().path.startsWith(r), route);
    },

    /**
     * Make sure the fixture library is a source folder, scanned and processed (image +
     * audio; never CLAP), and land on /library. Idempotent: cheap when already done,
     * which is what lets every spec call it.
     */
    async ensureLibrary({ process = true } = {}) {
      const folders = await app.folders();
      if (!folders.some((f) => samePath(f.path, LIBRARY))) {
        await page.evaluate((p) => window.asseteerTest.addFolder(p), LIBRARY);
      }
      if (process) {
        await page.evaluate(() => window.asseteerTest.processAll());
      }
      await app.navigate('/library');
      await app.waitForIdle();
    },

    /** The library's search box — a real input, for real typing. */
    searchBox: () => page.getByRole('textbox', { name: 'Search' }),

    /**
     * Replace the query in the real search box and wait until the (debounced) search has run and
     * its results are in. `searchText` is only set when the debounce fires, so it is the
     * signal that the query actually started.
     */
    async search(text) {
      // The input holds only the alternative being typed; earlier ones are chips. Clear
      // them so `text` replaces the whole query.
      const clear = page.getByTitle('Clear search');
      if (await clear.isVisible()) await clear.click();
      await app.searchBox().fill(text);
      await page.waitForFunction(
        (t) => {
          const s = window.asseteerTest.state();
          return s.searchText === t && !s.isLoading;
        },
        text,
        { timeout: 15_000, polling: 50 },
      );
      await app.settle();
    },

    /** No asset load in flight. */
    async waitForIdle() {
      await page.waitForFunction(() => !window.asseteerTest.state().isLoading, undefined, {
        timeout: 30_000,
        polling: 100,
      });
    },

    /** `'dark' | 'light' | null` — the app themes on `prefers-color-scheme`. */
    async theme(scheme) {
      await page.emulateMedia({ colorScheme: scheme });
      await app.settle();
    },

    /**
     * Wait until no CSS transition or animation is running. Many elements here use
     * `transition-all`, so a shot taken right after a theme switch or a hover catches
     * colours halfway between the two states — which looks exactly like a styling bug.
     */
    async settle({ timeout = 3000 } = {}) {
      await page
        .waitForFunction(
          () => document.getAnimations().every((a) => a.playState !== 'running' || a.effect?.getTiming().iterations === Infinity),
          undefined,
          { timeout, polling: 50 },
        )
        .catch(() => {}); // A spinner that never stops must not fail a run.
    },

    /** Screenshot into `tests/harness/out/`. Returns the absolute path — the path to
     *  hand to the Read tool. `target` can be a locator to shoot one element. */
    async shot(name, target = page) {
      mkdirSync(SHOT_DIR, { recursive: true });
      const safe = name.replace(/[<>:"/\\|?*]/g, '-');
      const file = path.join(SHOT_DIR, safe.endsWith('.png') ? safe : `${safe}.png`);
      await target.screenshot({ path: file });
      await onShot?.(file, name);
      return file;
    },
  };

  return app;
}
