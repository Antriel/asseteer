import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

// Unit tests for pure modules (`src/**/*.test.ts`). Deliberately not vite.config.js: the
// app config pulls in SvelteKit and Tailwind, none of which a pure helper test needs.
// End-to-end behaviour is Playwright's job (`npm run test:e2e`, tests/CLAUDE.md).
export default defineConfig({
  resolve: { alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) } },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
