import fs from 'node:fs';
import path from 'node:path';
import { test, expect } from './asseteer.mjs';
import { DATA_DIR } from '../harness/paths.mjs';

test.beforeEach(async ({ page, app }) => {
  await app.ensureLibrary();
  await page.getByRole('radio', { name: 'Audio' }).click();
  await page.getByRole('button', { name: 'Folders' }).first().click();
  await page.getByText('library', { exact: true }).first().click();
  await app.waitForIdle();
});

const row = (page, filename) => page.locator(`button[title$="/${filename}"]`);

/** The `.origin.json` sidecars in the drag cache for `filename`, parsed. */
const sidecars = (filename) => {
  const cache = path.join(DATA_DIR, 'drag-cache');
  if (!fs.existsSync(cache)) return [];
  return fs
    .readdirSync(cache)
    .map((dir) => path.join(cache, dir, `${filename}.origin.json`))
    .filter((file) => fs.existsSync(file))
    .map((file) => JSON.parse(fs.readFileSync(file, 'utf8')));
};

// Scry's import reads the sidecar to record where a dragged-in sound came from; a path
// pasted by hand and one read from a sidecar must be the same string.
test('a copied zip entry gets an origin sidecar holding its Copy Path', async ({ page }) => {
  // Reading the clipboard back hangs on a permission prompt; record what is written instead
  await page.evaluate(() => {
    window.__copiedText = [];
    navigator.clipboard.writeText = async (text) => void window.__copiedText.push(text);
  });

  await row(page, 'retro_powerup.wav').click({ button: 'right' });
  await page.getByRole('button', { name: 'Copy Path' }).click();
  await expect(page.getByText('Path copied')).toBeVisible();
  const [copied] = await page.evaluate(() => window.__copiedText);
  expect(copied).toMatch(/\\Packs\\Retro Pack\.zip\\Extras\\bonus\.zip\\retro_powerup\.wav$/);

  // Ctrl+C materializes the nested entry into the drag cache, as a drag would
  await row(page, 'retro_powerup.wav').click();
  await page.keyboard.press('Control+c');
  await expect(page.getByText('File copied')).toBeVisible();

  expect(sidecars('retro_powerup.wav')).toEqual([{ path: copied }]);
});

test('a plain file is used in place, with nothing written beside it', async ({ page }) => {
  await row(page, 'explosion_big.wav').click();
  await page.keyboard.press('Control+c');
  await expect(page.getByText('File copied')).toBeVisible();

  expect(sidecars('explosion_big.wav')).toEqual([]);
  const src = path.dirname(
    (await row(page, 'explosion_big.wav').getAttribute('title')).replace(/\//g, path.sep),
  );
  expect(fs.readdirSync(src).filter((f) => f.endsWith('.origin.json'))).toEqual([]);
});
