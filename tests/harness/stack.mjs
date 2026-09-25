/**
 * Bring the whole stack up: debug binary, dev server, fixture library, app.
 *
 * Shared by the Playwright runner's `globalSetup`, the one-shot `withAsseteer` driver,
 * and `npm run harness` — they differ only in what they do once it is up.
 *
 * Owns the dev server and the app — **but only the ones it had to start**. An app
 * already on the CDP port (say, `npm run harness` left up) is reused and left alone.
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { ensureLibrary } from '../fixtures/makeLibrary.mjs';
import { ensureDevServer } from './devServer.mjs';
import { APP_BINARY, isAppRunning, startApp } from './launch.mjs';
import { ROOT } from './paths.mjs';

/** Newest mtime under a path, recursively. */
function newestMtime(target) {
  const st = statSync(target);
  if (!st.isDirectory()) return st.mtimeMs;
  let newest = 0;
  for (const entry of readdirSync(target, { withFileTypes: true })) {
    newest = Math.max(newest, newestMtime(path.join(target, entry.name)));
  }
  return newest;
}

/**
 * Build the debug binary when it is missing or older than the Rust sources.
 *
 * Frontend-only iterations never reach `cargo`, because the binary loads its UI from
 * the dev server. A Rust change is the one case where an unattended run would otherwise
 * test yesterday's backend and report a mystery failure.
 */
function ensureBinary(log) {
  const sources = ['src-tauri/src', 'src-tauri/Cargo.toml', 'src-tauri/tauri.conf.json']
    .map((p) => path.join(ROOT, p))
    .filter(existsSync);

  let reason = null;
  if (!existsSync(APP_BINARY)) {
    reason = 'no debug binary yet';
  } else {
    const built = statSync(APP_BINARY).mtimeMs;
    if (sources.some((s) => newestMtime(s) > built)) reason = 'Rust sources are newer';
  }
  if (!reason) return;

  if (process.env.ASSETEER_NO_BUILD) {
    log(`WARNING: ${reason}, and ASSETEER_NO_BUILD is set — running against a stale binary`);
    return;
  }
  log(`cargo build (${reason}) — the slow path, only Rust changes hit it`);
  execFileSync('cargo', ['build'], { cwd: path.join(ROOT, 'src-tauri'), stdio: 'inherit' });
}

/**
 * @param {object} [opts]
 * @param {boolean} [opts.offscreen=true]  `npm run harness` passes false — the point there is to look.
 * @param {boolean} [opts.quiet=true]  Swallow the app's stdout/stderr.
 * @param {(msg: string) => void} [opts.log]
 * @returns {Promise<{ reusedApp: boolean, stop: () => Promise<void>, detach: () => void }>}
 */
export async function startStack({
  offscreen = true,
  quiet = true,
  log = (msg) => console.log(`[harness] ${msg}`),
} = {}) {
  const started = Date.now();

  // Decided first: reusing an app already on the CDP port is what lets a scratch script
  // or a spec run attach to `npm run harness` in ~2s.
  const reusedApp = await isAppRunning();

  const dev = await ensureDevServer();
  log(dev.started ? 'started vite dev on :1421' : 'vite dev already up on :1421');

  await ensureLibrary();

  if (reusedApp) {
    log('attached to an app already on the CDP port — skipped build');
  } else {
    ensureBinary(log);
  }

  const app = reusedApp ? null : await startApp({ offscreen, quiet });
  if (!reusedApp) log(`app launched (${offscreen ? 'offscreen' : 'visible'}, empty library)`);

  log(`ready in ${((Date.now() - started) / 1000).toFixed(1)}s`);

  return {
    reusedApp,
    stop: async () => {
      await app?.stop();
      await dev.stop();
      log('torn down');
    },
    /** Leave everything running and let this process exit anyway (the keepOpen path). */
    detach: () => {
      app?.detach();
      dev.detach();
    },
  };
}
