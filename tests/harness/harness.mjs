/**
 * `npm run harness` — the stack up with the fixture library scanned and processed, a
 * visible window, then it idles with CDP open.
 *
 * For a session where Peter is present: he uses the window while new console errors
 * stream here, or the agent points claude-in-chrome at the CDP port. It is also the fast
 * path for a series of scratch scripts — `withAsseteer` reuses an app already on the
 * port, so each `node scratch/foo.mjs` starts in about two seconds.
 *
 * Flags:
 *   --offscreen    park the window off the visible desktop after all
 *   --empty        skip adding the fixture library (test the first-run paths)
 *
 * Ctrl+C shuts down what this started.
 */

import { attachToApp, CDP_URL, isAppRunning } from './launch.mjs';
import { startStack } from './stack.mjs';
import { createApp, filterErrors, formatErrors, resetApp } from './appApi.mjs';
import { SHOT_DIR } from './paths.mjs';

const argv = process.argv.slice(2);
const has = (flag) => argv.includes(flag);
const log = (msg) => console.log(`[harness] ${msg}`);

const stack = await startStack({ offscreen: has('--offscreen'), quiet: false, log });
const { page } = await attachToApp();

const app = createApp({ page, onShot: (f) => log(`shot ${f}`) });
await resetApp(page);
await app.clearConsoleErrors();

if (!has('--empty')) {
  await app.ensureLibrary();
  log(`fixture library ready: ${app.library}`);
}

console.log(`
──────────────────────────────────────────────────────────────────────
  Idling. The stack stays up until Ctrl+C.

  CDP        ${CDP_URL}          (claude-in-chrome, or connectOverCDP)
  scratch    node scratch/foo.mjs   — withAsseteer() attaches to this app
  shots      ${SHOT_DIR}

  New console errors print below as they happen.
──────────────────────────────────────────────────────────────────────
`);

// Poll the app's own collected list rather than the CDP console: it is the same source
// the gate reads, so what prints here is exactly what would fail a spec.
let closing = false;
let poll;

const shutdown = async () => {
  if (closing) return;
  closing = true;
  clearInterval(poll);
  console.log('');
  await stack.stop();
  process.exit(0);
};

let reported = 0;
poll = setInterval(async () => {
  try {
    const errors = filterErrors(await app.consoleErrors());
    for (const line of formatErrors(errors.slice(reported))) console.log(`  ERROR ${line}`);
    reported = errors.length;
  } catch {
    // Usually a reload mid-poll. But it is also how `npm run harness:stop` (or Peter
    // closing the window) reaches us, so confirm against the port before deciding.
    if (!(await isAppRunning())) {
      log('the app is gone — shutting down');
      await shutdown();
    }
  }
}, 1000);

process.on('SIGINT', () => void shutdown());
process.on('SIGTERM', () => void shutdown());
