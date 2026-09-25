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
  await page.getByRole('radio', { name: 'Images' }).click();
  await app.search('tile');
  // grass_tile + stone_tile by filename, water_wide by its `Tiles/` folder path.
  expect((await app.state()).assetCount).toBe(3);
  await app.shot('search-images-tile');
});

test('a hyphenated term does not break search', async ({ app }) => {
  // Used to be parsed as FTS5 syntax ("no such column: fi"). Matches the file and, by its
  // Sci-Fi/ folder, laser_blast too.
  await app.search('sci-fi');
  expect((await app.state()).assetCount).toBe(2);
});

test('commas find any of the alternatives, shown as chips', async ({ page, app }) => {
  await app.search('pistol, laser');
  expect((await app.state()).assetCount).toBe(2);
  await expect(page.getByText('reload_pistol.wav')).toBeVisible();
  await expect(page.getByText('laser_blast.wav')).toBeVisible();
  // "pistol" became a chip; the input edits only the alternative after the comma
  await expect(page.getByRole('button', { name: 'Remove “pistol”' })).toBeVisible();
  await expect(app.searchBox()).toHaveValue('laser');
  await app.shot('search-or-chips');

  // Clicking a chip edits it: it swaps places with the text in the input
  await page.getByTitle('Edit “pistol”').click();
  await expect(app.searchBox()).toHaveValue('pistol');
  await expect(page.getByRole('button', { name: 'Remove “laser”' })).toBeVisible();
  expect((await app.state()).assetCount).toBe(2);

  // Backspace in the emptied input brings the last chip back for editing
  await app.searchBox().fill('');
  await app.searchBox().press('Backspace');
  await expect(app.searchBox()).toHaveValue('laser');
  await expect(page.getByRole('button', { name: /^Remove / })).toHaveCount(0);
});

test('a multi-word search with no results offers the words as alternatives', async ({
  page,
  app,
}) => {
  await app.search('gun explosion');
  expect((await app.state()).assetCount).toBe(0);
  const suggestion = page.getByRole('button', { name: /gun\s*or\s*explosion\s*4 audio/ });
  await expect(suggestion).toBeVisible();
  await app.shot('search-or-suggestion');

  await suggestion.click();
  await page.waitForFunction(() => {
    const s = window.asseteerTest.state();
    return s.searchText === 'gun, explosion' && !s.isLoading;
  });
  expect((await app.state()).assetCount).toBe(4);
  await expect(app.searchBox()).toHaveValue('explosion');
});
