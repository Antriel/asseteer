/**
 * Launch the real Asseteer app and attach Playwright to it over CDP.
 *
 * Launches the already-built debug binary rather than going through `npm run tauri dev`:
 * that command's watcher restarts the app on any Rust change, which would kill the CDP
 * connection mid-run. The binary still loads its frontend from the vite dev server
 * (`devUrl` is compiled in), so frontend edits are picked up on reload with no rebuild.
 *
 * Starting and attaching are separate exports because the Playwright suite splits them
 * across processes: global setup owns the app for the whole run, and each worker
 * attaches on its own.
 *
 * Ported from scry-app's harness (`scry-app/tests/harness/launch.mjs`).
 */

import { spawn } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, openSync, rmSync } from 'node:fs';
import { setTimeout as sleep } from 'node:timers/promises';
import path from 'node:path';
import { DATA_DIR, OUT_DIR, PROFILE_DIR, ROOT } from './paths.mjs';

/** A quiet app's stdout/stderr — read it when the app misbehaves. */
export const APP_LOG = path.join(OUT_DIR, 'app.log');

export const APP_BINARY = path.join(
  ROOT,
  'src-tauri/target/debug',
  process.platform === 'win32' ? 'Asseteer.exe' : 'Asseteer',
);

// 9223, not 9222: Scry's harness owns 9222, and both can be up at once.
const CDP_PORT = Number(process.env.ASSETEER_CDP_PORT ?? 9223);
export const CDP_URL = `http://localhost:${CDP_PORT}`;
export const FRONTEND_URL = 'http://localhost:1421';

/** Resolve after `fn()` returns truthy, or throw after `timeoutMs`. */
export async function waitFor(label, fn, { timeoutMs = 60_000, everyMs = 250 } = {}) {
  const deadline = Date.now() + timeoutMs;
  let lastErr;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (err) {
      lastErr = err;
    }
    await sleep(everyMs);
  }
  throw new Error(
    `Timed out after ${timeoutMs}ms waiting for ${label}` +
      (lastErr ? `\n  last error: ${lastErr.message}` : ''),
  );
}

async function assertFrontendServing() {
  try {
    await fetch(FRONTEND_URL, { signal: AbortSignal.timeout(2000) });
  } catch {
    throw new Error(
      `Nothing is serving ${FRONTEND_URL}.\n` +
        `The debug binary loads its frontend from the vite dev server.\n` +
        `Start it first:  npm run dev   (or let the harness start it)`,
    );
  }
}

/** True once something answers on the CDP port — i.e. an app is already up. */
export async function isAppRunning() {
  try {
    const res = await fetch(`${CDP_URL}/json/version`, { signal: AbortSignal.timeout(1000) });
    return res.ok;
  } catch {
    return false;
  }
}

/**
 * Spawn the app and wait until its CDP endpoint answers. Does not attach.
 *
 * Always isolated: `--data-dir` and `--profile-dir` are not optional, because the
 * alternative is a harness run scanning fixtures into Peter's real library.
 *
 * @param {object} [opts]
 * @param {boolean} [opts.offscreen=true]  Park the window off the visible desktop.
 * @param {string} [opts.size="1400x900"]  Window size (logical px). Under CDP the
 *   viewport *is* the window, and the 800x600 default is cramped.
 * @param {boolean} [opts.freshData=true]  Wipe the data dir first — an empty library.
 * @param {boolean} [opts.quiet=false]  Send the app's stdout/stderr to `APP_LOG` instead
 *   of this terminal. Required for an app that should outlive this process.
 */
export async function startApp({
  offscreen = true,
  size = '1400x900',
  freshData = true,
  quiet = false,
} = {}) {
  if (!existsSync(APP_BINARY)) {
    throw new Error(`Debug binary not found at ${APP_BINARY}\nBuild it first:  cd src-tauri && cargo build`);
  }
  await assertFrontendServing();

  if (freshData) rmSync(DATA_DIR, { recursive: true, force: true });
  mkdirSync(DATA_DIR, { recursive: true });
  mkdirSync(PROFILE_DIR, { recursive: true });

  const args = ['--data-dir', DATA_DIR, '--profile-dir', PROFILE_DIR];
  if (offscreen) args.push('--offscreen');
  if (size) args.push('--window-size', size);

  // Quiet output goes to a file, never a pipe: a kept-open app outlives this process,
  // and once the pipe's reader is gone Rust's `println!` panics on the broken pipe.
  mkdirSync(OUT_DIR, { recursive: true });
  const logFd = quiet ? openSync(APP_LOG, 'w') : null;
  const proc = spawn(APP_BINARY, args, {
    cwd: ROOT,
    stdio: quiet ? ['ignore', logFd, logFd] : ['ignore', 'pipe', 'pipe'],
    // Without this, libuv puts the child in a kill-on-close job object and Windows ends
    // it the moment this script exits — `keepOpen` would silently do nothing.
    detached: true,
    windowsHide: true,
    env: {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${CDP_PORT}`,
    },
  });
  if (logFd !== null) closeSync(logFd);

  if (!quiet) {
    proc.stdout.on('data', (d) => process.stdout.write(`[app] ${d}`));
    proc.stderr.on('data', (d) => process.stderr.write(`[app] ${d}`));
  }

  let exited = null;
  proc.on('exit', (code) => {
    exited = code;
  });

  // The CDP endpoint answering is the readiness signal — cheaper and more reliable
  // than sleeping.
  await waitFor(`CDP on ${CDP_URL}`, async () => {
    if (exited !== null) {
      throw new Error(`app exited with code ${exited} before CDP came up` + (quiet ? ` — see ${APP_LOG}` : ''));
    }
    return isAppRunning();
  });

  const stop = async () => {
    if (exited !== null) return;
    // Windows: the webview host outlives a plain kill, and with its stdio still piped
    // it would keep this process alive. `/T` takes the whole tree.
    if (process.platform === 'win32') {
      await new Promise((resolve) => {
        spawn('taskkill', ['/F', '/T', '/PID', String(proc.pid)], { stdio: 'ignore' })
          .on('exit', resolve)
          .on('error', resolve);
      });
    }
    proc.kill();
  };

  /** Let this process exit while the app keeps running (`keepOpen`). Only safe for a
   *  quiet app — a piped one would die on its next log line. */
  const detach = () => {
    proc.stdout?.unref?.();
    proc.stderr?.unref?.();
    proc.unref();
  };

  return { proc, stop, detach };
}

/**
 * Attach Playwright to an app that is already up, and find its main webview.
 * @returns {Promise<{ browser, page }>}
 */
export async function attachToApp() {
  const { chromium } = await import('playwright');
  const browser = await chromium.connectOverCDP(CDP_URL);

  // Asseteer has one window. Right after launch it is `about:blank`; if its first load
  // raced a dev server still optimizing deps, it sits on a `chrome-error://` page for
  // good and would never match a frontend URL — so take the page, not a URL match.
  const page = await waitFor('the main webview target', () =>
    browser
      .contexts()
      .flatMap((c) => c.pages())
      .find((p) => p.url() !== 'about:blank'),
  );
  if (!page.url().startsWith(FRONTEND_URL)) await gotoFrontend(page, '/');

  return { browser, page };
}

/**
 * Navigate the webview to a frontend route, riding out a dev server that drops the
 * first connection or two while it optimizes deps (ERR_CONNECTION_RESET). That is the
 * dev server working as designed, not the app's fault.
 */
export async function gotoFrontend(page, route = '/') {
  for (let attempt = 1; ; attempt++) {
    try {
      await page.goto(FRONTEND_URL + route);
      return;
    } catch (err) {
      if (attempt >= 8 || !/net::ERR_/.test(String(err))) throw err;
      await sleep(1000);
    }
  }
}
