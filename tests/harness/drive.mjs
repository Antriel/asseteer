/**
 * `withAsseteer` — drive the real app from a one-shot script, and look at the result.
 *
 * The agent loop: change code, drive, look, iterate, report with images.
 *
 *   // scratch/check-my-change.mjs
 *   import { withAsseteer } from '../tests/harness/drive.mjs';
 *
 *   await withAsseteer(async ({ page, app, shot }) => {
 *     await app.ensureLibrary();            // fixture scanned + processed, on /library
 *     await shot('library');
 *     await page.getByPlaceholder(/search/i).fill('gun');
 *     await shot('search-gun');
 *   });
 *
 * A JS API rather than a CLI grammar: agents write code well, and a rigid flag
 * language would need extending for every new thing anyone wanted to drive.
 *
 * The screenshots are **real WebView2 output**, pixel-identical to what Peter sees.
 */

import { attachToApp } from './launch.mjs';
import { startStack } from './stack.mjs';
import { createApp, filterErrors, formatErrors, resetApp } from './appApi.mjs';
import { buildContactSheet } from './contactSheet.mjs';
import { SHOT_DIR } from './paths.mjs';

const log = (msg) => console.log(`[drive] ${msg}`);

/**
 * Bring the stack up, hand `{ page, app, shot }` to `body`, then report and tear down.
 *
 * @param {(ctx: { page: any, app: any, shot: (name: string, target?: any) => Promise<string> }) => Promise<any>} body
 * @param {object} [opts]
 * @param {boolean} [opts.reset=true]  Clear persisted UI state and reload before `body`.
 * @param {boolean} [opts.failOnConsoleErrors=true]  Non-zero exit if the app logged errors.
 * @param {boolean} [opts.keepOpen=false]  Leave the app running; the next run reuses it
 *   (~2s instead of a cold boot). `npm run harness:stop` ends it.
 * @param {boolean} [opts.offscreen=true]  Park the window off the visible desktop.
 */
export async function withAsseteer(
  body,
  { reset = true, failOnConsoleErrors = true, keepOpen = false, offscreen = true } = {},
) {
  const stack = await startStack({ offscreen, log });
  let attached;
  try {
    attached = await attachToApp();
  } catch (err) {
    // Nothing to report on yet, but don't leave an app and a dev server behind.
    await stack.stop();
    throw err;
  }
  const { browser, page } = attached;

  const shots = [];
  const app = createApp({
    page,
    onShot: (file, name) => {
      shots.push({ file, label: name });
      log(`shot ${file}`);
    },
  });

  let result;
  let thrown;
  try {
    if (reset) await resetApp(page);
    await app.clearConsoleErrors();
    result = await body({ page, app, shot: app.shot });
  } catch (err) {
    thrown = err;
    // A screenshot of the moment it broke is usually worth more than the stack trace.
    await app.shot('drive-failure').catch(() => {});
  }

  let errors = [];
  try {
    errors = filterErrors(await app.consoleErrors());
  } catch {
    // The page went away — nothing left to read.
  }

  let sheet = null;
  if (shots.length > 1) {
    sheet = await buildContactSheet(shots, { title: `withAsseteer — ${shots.length} shots` }).catch((err) => {
      log(`contact sheet failed: ${err.message}`);
      return null;
    });
  }

  // ── the report ─────────────────────────────────────────────────────────────────
  console.log('');
  if (errors.length) {
    console.log(`console errors (${errors.length}):`);
    for (const line of formatErrors(errors)) console.log(`  ${line}`);
  } else {
    console.log('console errors: none');
  }
  if (sheet) console.log(`\ncontact sheet (read this first):\n  ${sheet}`);
  if (shots.length) {
    console.log(`\nscreenshots in ${SHOT_DIR} — read them, do not just list them:`);
    for (const { file } of shots) console.log(`  ${file}`);
  }
  console.log('');

  try {
    await browser.close();
  } catch {
    // the app may already be gone
  }
  if (keepOpen) {
    stack.detach();
    log('app left running (keepOpen) — the next withAsseteer will reuse it');
  } else {
    await stack.stop();
  }

  if (thrown) throw thrown;
  if (failOnConsoleErrors && errors.length) process.exitCode = 1;

  return { result, errors, shots: shots.map((s) => s.file), sheet };
}
