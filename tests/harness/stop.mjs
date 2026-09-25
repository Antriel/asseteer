/**
 * Kill an app the harness left running — `npm run harness:stop`.
 *
 * `npm run harness` and `withAsseteer(..., { keepOpen: true })` leave the app up so the
 * next run attaches in ~2s. This is how that ends. It kills the process tree that owns
 * the CDP port — never by image name, which would also take down an Asseteer Peter has
 * open for real use.
 *
 * Also ends a vite dev server the harness started and left running (recorded in
 * `vite.pid`). One the user started has no such record and is left alone.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { VITE_PID_FILE } from './devServer.mjs';
import { CDP_URL, isAppRunning } from './launch.mjs';

function stopHarnessVite() {
  if (!existsSync(VITE_PID_FILE)) return;
  const pid = readFileSync(VITE_PID_FILE, 'utf8').trim();
  rmSync(VITE_PID_FILE, { force: true });
  const res = spawnSync('taskkill', ['/F', '/T', '/PID', pid], { encoding: 'utf8' });
  if (res.status === 0) console.log(`stopped the harness's vite dev server (PID ${pid})`);
}

if (!(await isAppRunning())) {
  console.log('Nothing listening on the CDP port — no app to stop.');
  stopHarnessVite();
  process.exit(0);
}

// The port is owned by msedgewebview2.exe; its parent is the Asseteer process.
const port = new URL(CDP_URL).port;
const ps = spawnSync(
  'powershell',
  [
    '-NoProfile',
    '-Command',
    `$c = Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1;` +
      `if ($c) { $p = Get-CimInstance Win32_Process -Filter "ProcessId=$($c.OwningProcess)";` +
      `while ($p -and $p.Name -ne 'Asseteer.exe') { $p = Get-CimInstance Win32_Process -Filter "ProcessId=$($p.ParentProcessId)" };` +
      `if ($p) { $p.ProcessId } }`,
  ],
  { encoding: 'utf8' },
);
const pid = ps.stdout.trim();
if (!pid) {
  console.log(`Could not find the Asseteer process behind port ${port}.\n${ps.stderr}`);
  process.exit(1);
}

const res = spawnSync('taskkill', ['/F', '/T', '/PID', pid], { encoding: 'utf8' });
console.log(res.status === 0 ? `stopped Asseteer (PID ${pid}) and its webview` : res.stderr?.trim());
stopHarnessVite();
