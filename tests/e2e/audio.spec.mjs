import { test, expect } from './asseteer.mjs';

test.beforeEach(async ({ page, app }) => {
  await app.ensureLibrary();
  await page.getByRole('radio', { name: 'Audio' }).click();
  await page.getByRole('button', { name: 'Folders' }).first().click();
  await page.getByText('library', { exact: true }).first().click();
  await app.waitForIdle();
});

const row = (page, filename) => page.locator(`button[title$="/${filename}"]`);

test('rows are single lines and keyboard navigation moves the selection', async ({ page }) => {
  const box = await row(page, 'explosion_big.wav').boundingBox();
  expect(box.height).toBe(32);

  await row(page, 'explosion_big.wav').click();
  await page.keyboard.press('ArrowDown');
  await expect(
    page.getByRole('application').locator('p', { hasText: 'explosion_small.wav' }),
  ).toBeVisible();
});

// A zip entry loads through an async blob; a later, plain file must not be overwritten
// by it when the blob resolves late.
test('a slow zip load does not replace the sound picked after it', async ({ page }) => {
  // Three zip entries in a row, then a plain file — several blob loads in flight at once
  await row(page, 'retro_coin.wav').click();
  for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowDown'); // → sci-fi_door_open.wav
  // Waiting for something *not* to happen: give the zip's blob time to arrive.
  await page.waitForTimeout(1000);

  const src = await page.locator('audio').getAttribute('src');
  expect(src).not.toMatch(/^blob:/);
  expect(decodeURIComponent(src)).toContain('sci-fi_door_open.wav');
});

test('end-of-track mode is a persisted three-way choice', async ({ page }) => {
  await row(page, 'forest_birds_loop.wav').click();
  const repeat = page.getByRole('radio', { name: 'Repeat' });
  const stop = page.getByRole('radio', { name: 'Stop at end' });

  await expect(stop).toHaveAttribute('aria-checked', 'true');
  await repeat.click();
  await expect(repeat).toHaveAttribute('aria-checked', 'true');
  await expect(stop).toHaveAttribute('aria-checked', 'false');

  const stored = await page.evaluate(() => localStorage.getItem('asseteer-settings'));
  expect(JSON.parse(stored).audioEndMode).toBe('repeat');
});
