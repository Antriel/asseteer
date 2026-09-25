/**
 * Make sure something is serving the frontend on :1421.
 *
 * The debug binary has `devUrl` compiled in, so it loads its UI from the vite dev server.
 * A cold agent session has none running, so start one — and stop only what we started.
 *
 * Spawns `node node_modules/vite/bin/vite.js` rather than the `vite` shim: Node 24
 * refuses to spawn a `.cmd` without `shell: true`, and a `cmd.exe` in between survives
 * `kill()` and keeps holding the port.
 */

import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { OUT_DIR, ROOT } from './paths.mjs';
import { FRONTEND_URL } from './launch.mjs';

const VITE_LOG = path.join(OUT_DIR, 'vite.log');

/** PID of a dev server the harness started and left running (`keepOpen`), so
 *  `npm run harness:stop` can end it — a leftover would hold :1421 and make Peter's
 *  next `npm run tauri dev` fail on the strict port. Absent for a user-started server. */
export const VITE_PID_FILE = path.join(OUT_DIR, 'vite.pid');

// Via `package.json`: vite's `exports` map does not expose its bin path.
const VITE_BIN = path.join(
  path.dirname(createRequire(import.meta.url).resolve('vite/package.json')),
  'bin/vite.js',
);

async function isServing() {
  try {
    await fetch(FRONTEND_URL, { signal: AbortSignal.timeout(1000) });
    return true;
  } catch {
    return false;
  }
}

/**
 * @returns {Promise<{ started: boolean, stop: () => Promise<void>, detach: () => void }>}
 *   `started` is false when a dev server was already up — then `stop` is a no-op,
 *   because it is the user's terminal and not ours to kill.
 */
export async function ensureDevServer() {
  if (await isServing()) return { started: false, stop: async () => {}, detach: () => {} };

  // Output to a file, never a pipe: a kept-open dev server outlives this process, and
  // writing to a pipe with no reader would take it down.
  mkdirSync(OUT_DIR, { recursive: true });
  const logFd = openSync(VITE_LOG, 'w');
  const proc = spawn(process.execPath, [VITE_BIN, 'dev'], {
    cwd: ROOT,
    stdio: ['ignore', logFd, logFd],
    // See launch.mjs: not detached = killed when this script exits.
    detached: true,
    windowsHide: true,
  });
  closeSync(logFd);
  const log = () => (existsSync(VITE_LOG) ? readFileSync(VITE_LOG, 'utf8') : '');

  let exited = null;
  proc.on('exit', (code) => (exited = code));

  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (exited !== null) throw new Error(`vite dev exited with ${exited}\n${log()}`);
    if (await isServing()) {
      return {
        started: true,
        detach: () => {
          writeFileSync(VITE_PID_FILE, String(proc.pid));
          proc.unref();
        },
        stop: async () => {
          if (exited !== null) return;
          if (process.platform === 'win32') {
            await new Promise((resolve) => {
              spawn('taskkill', ['/F', '/T', '/PID', String(proc.pid)], { stdio: 'ignore' })
                .on('exit', resolve)
                .on('error', resolve);
            });
          }
          proc.kill();
        },
      };
    }
    await sleep(250);
  }
  throw new Error(`Timed out waiting for ${FRONTEND_URL}\n${log()}`);
}
