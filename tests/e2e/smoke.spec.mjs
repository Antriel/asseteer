import { test, expect } from './asseteer.mjs';

test('fixture library scans (zips included) and processes', async ({ app }) => {
  await app.ensureLibrary();

  const [folder] = await app.folders();
  // 16 loose files + a zip holding 3 files and a nested zip holding 2.
  expect(folder.asset_count).toBe(21);

  const { pending } = await app.state();
  expect(pending.audio).toBe(0);
  expect(pending.images).toBe(0);
  await app.shot('library-after-processing');
});

test('every page mounts without console errors', async ({ app }) => {
  await app.ensureLibrary();
  for (const route of ['/library', '/processing', '/sources', '/settings']) {
    await app.navigate(route);
    await app.settle();
    await app.shot(`page-${route.slice(1)}`);
  }
});
