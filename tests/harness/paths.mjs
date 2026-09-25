/**
 * Where everything the harness owns lives. All under `tests/harness/out/` (gitignored).
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export const OUT_DIR = path.join(ROOT, 'tests/harness/out');

/** Screenshots. Agents read these back with the Read tool. */
export const SHOT_DIR = OUT_DIR;

/**
 * The app's data directory under the harness (`--data-dir`): the SQLite library, CLAP
 * logs. Wiped on every cold launch, so a run starts from an empty library and never
 * sees — or touches — the real `asseteer.db`.
 */
export const DATA_DIR = path.join(OUT_DIR, 'data');

/**
 * The webview profile (`--profile-dir`): localStorage holds settings and view state.
 * Every build of Asseteer otherwise shares one profile derived from the bundle
 * identifier, so `resetApp()` clearing localStorage would clear the real app's settings.
 * Kept between runs — a cold WebView2 profile only costs startup time.
 */
export const PROFILE_DIR = path.join(OUT_DIR, 'profile');
