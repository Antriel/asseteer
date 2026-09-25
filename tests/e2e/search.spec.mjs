import { test, expect } from './asseteer.mjs';

test.beforeEach(async ({ app }) => {
  await app.ensureLibrary();
});

test('typing a term filters the audio list', async ({ page, app }) => {
  await app.search('gun');
  expect((await app.state()).assetCount).toBe(2);
  await expect(page.getByText('gun_shot_01.wav')).toBeVisible();
  await expect(page.getByText('explosion_big.wav')).toHaveCount(0);
  await app.shot('search-gun');
});

test('finds audio inside a nested zip', async ({ page, app }) => {
  await app.search('powerup');
  expect((await app.state()).assetCount).toBe(1);
  await expect(page.getByText('retro_powerup.wav')).toBeVisible();
});

test('images tab searches images', async ({ page, app }) => {
  await page.getByRole('button', { name: /^Images/ }).click();
  await app.search('tile');
  // grass_tile + stone_tile by filename, water_wide by its `Tiles/` folder path.
  expect((await app.state()).assetCount).toBe(3);
  await app.shot('search-images-tile');
});

// Known bug: raw input is parsed as FTS5 syntax, so `sci-fi` → "no such column: fi".
// Remove `fixme` when asseteer-no3w lands.
test.fixme('a hyphenated term does not break search (asseteer-no3w)', async ({ app }) => {
  await app.search('sci-fi');
  expect((await app.state()).assetCount).toBe(1);
});
