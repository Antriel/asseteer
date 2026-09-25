# Testing Asseteer

The harness drives the **real Tauri app** over CDP. Edge WebView2 accepts
`--remote-debugging-port` (set in `src-tauri/src/lib.rs`, debug builds only), so Playwright
attaches to the running app and everything under it is production code: real Rust commands,
real SQLite + FTS5, real zip handling, real WebView2 pixels.

Only the data is a substitute: `tests/fixtures/makeLibrary.mjs` generates a small fixture
library (WAVs, PNGs, a zip with a nested zip) into `tests/fixtures/library/` (gitignored).

Ported from scry-app's harness (`scry-app/tests/CLAUDE.md`); keep them recognisably alike.

## Isolation — why it is safe to run

A harness-launched app gets `--data-dir tests/harness/out/data` and
`--profile-dir tests/harness/out/profile`. The library DB, localStorage and settings of the
Asseteer Peter actually uses are never read or written, and the window-state plugin is off
(it would save the offscreen position into the real app). The data dir is wiped on every cold
launch, so a run starts from an empty library. Never drop these flags from `startApp`.

## The loop

**Change code → drive → look → iterate → report with images.** A change is not done until
you have shown it working.

1. `npm run check:svelte` (and `check:cargo` for Rust).
2. `npm run test:unit` for pure modules; `npm run test:e2e` if a spec covers the change.
3. Otherwise write a scratch script against `withAsseteer` (below), run it, and **Read the
   PNGs back** — a script that ran without throwing has proved almost nothing about what the
   UI looks like.
4. Report: what you ran, the console-error list, and the screenshots you looked at.

**Read `tests/harness/out/contact.png` first** — every shot of a run in one labelled grid,
the only place a before and an after sit side by side. Cells are downscaled: judge layout,
state and colour there, open the individual PNG when a detail (a number, a thin line) matters.

## Driving it yourself

```js
// scratch/check-my-change.mjs — write it, run it, delete it
import { withAsseteer } from '../tests/harness/drive.mjs';

await withAsseteer(async ({ page, app, shot }) => {
  await app.ensureLibrary();       // fixture scanned + processed, on /library
  await app.search('explosion');   // types into the real box, waits for results
  await shot('dark');
  await app.theme('light');        // the app themes on prefers-color-scheme
  await shot('light');
  await page.getByRole('button', { name: /^Images/ }).click();
});
```

`node scratch/check-my-change.mjs` — boots the stack (vite dev + debug binary, rebuilding it
if Rust sources are newer), resets UI state, runs the body, prints console errors and
screenshot paths, tears down what it started. Non-zero exit if the app logged an error.

Options: `{ keepOpen: true }` leaves the app up so the next run attaches in ~2s instead of a
cold boot (`npm run harness:stop` when done — it also stops a vite the harness started, which
would otherwise block `npm run tauri dev` on port 1421); `{ reset: false }` keeps the
previous run's UI state; `{ failOnConsoleErrors: false }` for exploring a knowingly broken
state; `{ offscreen: false }` to watch it.

`app` is the same object the specs get — `tests/harness/appApi.mjs`. Add helpers there, not
in one consumer. Current surface: `ensureLibrary`, `search`, `searchBox`, `navigate`,
`theme`, `settle`, `waitForIdle`, `state`, `folders`, `shot`, `consoleErrors`.

**Wait for transitions before judging colour.** Much of the UI uses `transition-all`; a shot
right after a theme switch or hover catches colours mid-fade and looks exactly like a styling
bug. `app.theme()` and `app.search()` already call `app.settle()`; call it yourself after
anything else that animates.

## Interactive

```
npm run harness        # stack up, fixture library ready, visible window, then idles
npm run harness:stop   # end it
```

CDP stays on `localhost:9223` (not 9222 — that is Scry's), so claude-in-chrome can attach,
or Peter can use the window while new console errors stream into the harness terminal.
`withAsseteer` and `npm run test:e2e` reuse an app already on that port.

## Specs

`tests/e2e/*.spec.mjs`, `npm run test:e2e`. Serial, one worker, one app instance.

Specs assert **hard facts, not pixels** (counts, visible rows, pending counts). No pixel
baselines — they rot and their maintenance lands on Peter. Screenshots in a spec are evidence.

A red spec leaves evidence: `test-results/<spec>/test-failed-1.png` + `trace.zip`
(`npx playwright show-trace <path>` — its frames are the real app), and
`tests/harness/out/failure-<title>.png`, included in the contact sheet.

**Every spec fails on any `console.error`, uncaught error or unhandled rejection.** This
matters here specifically: a failing search query is only ever visible as a console error
(the list just keeps its stale results). Muting means adding to `IGNORED_ERRORS` in
`appApi.mjs` with a reason — fix the cause instead where possible.

A known bug gets a `test.fixme` naming its bean (see `search.spec.mjs`), not a deleted test.

`window.asseteerTest` (`src/lib/harness/testHooks.ts`, dev builds only) is the arrangement
surface: add/remove folders without the native picker, run processing, read state.
**Hooks are for arrangement; real clicks and typing are for the thing under test.**

## Unit tests

`npm run test:unit` — Vitest over `src/**/*.test.ts`, node environment, own
`vitest.config.js` (not the app's vite config). For pure modules: query builders,
formatters, parsers. `npm run test:cargo` for the Rust side.

## What the harness cannot reach

- **OS drag-and-drop, native file dialogs** — CDP has no OS-level input. Drag-out to other
  programs (Audacity, Explorer) stays a manual test for Peter. Folder picking is bypassed with
  `asseteerTest.addFolder(path)`.
- **Audio output** — playback state is observable, sound is not.
- **CLAP / semantic search** — needs uv, a Python server and a model download; `processAll`
  deliberately skips it. Test that manually.
- **Windows only** — WebView2 CDP. macOS/Linux builds are not covered.

## Files

| | |
|---|---|
| `harness/paths.mjs` | out dir, isolated data dir + webview profile |
| `harness/launch.mjs` | spawn the debug binary (isolated, detached), attach over CDP |
| `harness/devServer.mjs` | vite dev on :1421, started only if not already up |
| `harness/stack.mjs` | binary + dev server + fixture + app; shared by all entry points |
| `harness/appApi.mjs` | the `app` helper and the error filter — shared by specs and scripts |
| `harness/drive.mjs` | `withAsseteer` — the agent's one-shot driver |
| `harness/contactSheet.mjs` | a run's shots stitched into one labelled grid PNG |
| `harness/harness.mjs` / `stop.mjs` | `npm run harness` / `harness:stop` |
| `harness/globalSetup.mjs` | Playwright global setup: stack up once per run |
| `e2e/asseteer.mjs` | the suite's `test` + the console-error gate |
| `fixtures/makeLibrary.mjs` | deterministic fixture library generator (bump `VERSION` on change) |

Logs of a quiet app and a harness-started vite: `tests/harness/out/app.log`, `vite.log`.

## Gotchas learned while porting

- Children must be spawned `detached: true`: libuv otherwise puts them in a kill-on-close job
  object, and Windows kills them the moment the script exits — `keepOpen` silently does nothing.
- A quiet app's output goes to a file, never a pipe: once the reader is gone, Rust's
  `println!` panics on the broken pipe.
- A freshly started vite may reset the first connections while optimizing deps; the webview
  then sits on a `chrome-error://` page. `attachToApp`/`gotoFrontend` retry through it.
