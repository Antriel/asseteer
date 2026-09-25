/**
 * Playwright's global setup: bring the stack up once for a run, take it down after.
 *
 * Everything real happens in `stack.mjs`, shared with `withAsseteer` and
 * `npm run harness`, so a spec run and a scratch script boot the app the same way.
 * It also owns the run's shot log and stitches it into one contact sheet on the way out.
 */

import { startStack } from './stack.mjs';
import { buildContactSheetFromLog, resetShotLog } from './contactSheet.mjs';

export default async function globalSetup() {
  resetShotLog();
  const { stop } = await startStack();

  return async () => {
    // Evidence first: the sheet must survive a teardown that goes wrong.
    try {
      const sheet = await buildContactSheetFromLog();
      if (sheet) console.log(`\ncontact sheet (read this first):\n  ${sheet}\n`);
    } catch (err) {
      console.warn(`[harness] contact sheet failed: ${err.message}`);
    }
    await stop();
  };
}
