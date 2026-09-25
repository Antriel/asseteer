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

const audioState = (page) =>
  page.evaluate(() => {
    const a = document.querySelector('audio');
    const fill = document.querySelector('[aria-label="Seek audio"] .bg-accent');
    return { paused: a.paused, ended: a.ended, loop: a.loop, t: a.currentTime, fill: fill.style.width };
  });

const nowPlaying = (page) => page.getByRole('application').locator('p.font-medium').textContent();

// The playhead used to freeze at the last animation-frame sample, visibly short of the end
// on sounds of a few hundred ms.
test('a short sound that plays out leaves the progress bar full', async ({ page }) => {
  for (const f of ['footstep_grass_01.wav', 'laser_blast.wav', 'retro_coin.wav']) {
    await row(page, f).click();
    await expect.poll(async () => (await audioState(page)).ended).toBe(true);
    expect((await audioState(page)).fill).toBe('100%');
  }
});

test('"Play next" moves on to the following sound when one ends', async ({ page }) => {
  await row(page, 'explosion_big.wav').click(); // the transport, and its mode radios, need a selection
  await page.getByRole('radio', { name: 'Play next' }).click();
  const names = await page
    .locator('button[title$=".wav"]')
    .evaluateAll((rows) => rows.map((r) => r.title.split('/').pop()));
  const i = names.indexOf('footstep_grass_01.wav');
  await row(page, names[i]).click();

  await expect.poll(() => nowPlaying(page)).toBe(names[i + 1]);
  await expect.poll(async () => (await audioState(page)).paused).toBe(false);
});

test('"Repeat" loops the sound instead of stopping', async ({ page }) => {
  await row(page, 'laser_blast.wav').click(); // 0.5 s
  await page.getByRole('radio', { name: 'Repeat' }).click();
  await page.waitForTimeout(1500);

  const s = await audioState(page);
  expect(s).toMatchObject({ loop: true, paused: false, ended: false });
  expect(await nowPlaying(page)).toBe('laser_blast.wav');
});

test('"Test loop" repeats, starting 5 s before the end', async ({ page }) => {
  await row(page, 'forest_birds_loop.wav').click(); // 6 s
  await expect.poll(async () => (await audioState(page)).paused).toBe(false);
  await page.getByRole('button', { name: 'Test loop' }).click();

  await expect(page.getByRole('radio', { name: 'Repeat' })).toHaveAttribute('aria-checked', 'true');
  const start = await audioState(page);
  expect(start.t).toBeGreaterThanOrEqual(1);
  expect(start.t).toBeLessThan(2);
  expect(start.paused).toBe(false);

  // Through the loop point and back round
  await expect.poll(async () => (await audioState(page)).t, { timeout: 8000 }).toBeLessThan(1);
  expect(await audioState(page)).toMatchObject({ paused: false, ended: false });
});
