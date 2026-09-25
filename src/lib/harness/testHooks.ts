/**
 * `window.asseteerTest` — the harness's arrangement surface. Dev builds only.
 *
 * **Hooks are for arrangement; real clicks are for the thing under test.** These call
 * the same code the buttons call, so a script can reach "fixture library scanned and
 * processed, library page open" without driving a native folder picker (which CDP
 * cannot reach). If the clicking *is* the behaviour under test, click.
 *
 * See `tests/CLAUDE.md`.
 */

import { goto } from '$app/navigation';
import { page } from '$app/state';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { assetsState } from '$lib/state/assets.svelte';
import { exploreState } from '$lib/state/explore.svelte';
import { isAnyRunning, processingState } from '$lib/state/tasks.svelte';
import { uiState } from '$lib/state/ui.svelte';
import { viewState } from '$lib/state/view.svelte';
import type { ProcessingCategory, SourceFolder } from '$lib/types';

export interface AsseteerTestState {
  path: string;
  activeTab: string;
  /** Assets currently loaded into the list. */
  assetCount: number;
  totalCount: number;
  isLoading: boolean;
  searchText: string;
  scanning: boolean;
  processing: boolean;
  pending: { images: number; audio: number; clap: number; total: number };
}

export interface AsseteerTestError {
  source: 'console.error' | 'window.onerror' | 'unhandledrejection';
  message: string;
  stack?: string;
}

export interface AsseteerTestHooks {
  state(): AsseteerTestState;
  navigate(path: string): Promise<void>;
  folders(): Promise<SourceFolder[]>;
  /** Add and scan a source folder — the Sources page's "Add folder", minus the picker. */
  addFolder(path: string): Promise<void>;
  removeFolder(id: number): Promise<void>;
  /** Run image + audio processing to completion. CLAP is deliberately excluded: it needs
   *  a Python server and a model download, which a harness run must not depend on. */
  processAll(categories?: ProcessingCategory[]): Promise<void>;
  consoleErrors(): AsseteerTestError[];
  clearConsoleErrors(): void;
}

const errors: AsseteerTestError[] = [];

/** One console argument as text — `String(obj)` would give "[object Object]". */
function describe(arg: unknown): string {
  if (arg instanceof Error) return arg.message || arg.stack?.split('\n')[0] || arg.name;
  if (typeof arg === 'object' && arg !== null) {
    try {
      return JSON.stringify(arg);
    } catch {
      return Object.prototype.toString.call(arg);
    }
  }
  return String(arg);
}

/**
 * Collect anything that looks like a failure. The console-error gate built on this is
 * the cheapest broad check the harness has — e.g. a search that throws an FTS syntax
 * error is only ever visible in the console.
 */
function captureErrors(): void {
  const originalError = console.error.bind(console);
  console.error = (...args: unknown[]) => {
    errors.push({
      source: 'console.error',
      message: args.map(describe).join(' ').trim() || '(console.error with no message)',
      stack: args.find((a): a is Error => a instanceof Error)?.stack,
    });
    originalError(...args);
  };

  window.addEventListener('error', (event) => {
    errors.push({
      source: 'window.onerror',
      message: event.message,
      stack: event.error instanceof Error ? event.error.stack : undefined,
    });
  });

  window.addEventListener('unhandledrejection', (event) => {
    const reason: unknown = event.reason;
    errors.push({
      source: 'unhandledrejection',
      message: reason instanceof Error ? reason.message : String(reason),
      stack: reason instanceof Error ? reason.stack : undefined,
    });
  });
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function processCategory(category: ProcessingCategory, timeoutMs = 120_000) {
  await processingState.refreshPendingCount();
  if (processingState.getPendingCountForCategory(category) === 0) return;

  await processingState.startProcessing(category);

  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    await sleep(250);
    if (isAnyRunning(processingState)) continue;
    await processingState.refreshPendingCount();
    if (processingState.getPendingCountForCategory(category) === 0) return;
  }
  throw new Error(`processing ${category} did not finish within ${timeoutMs}ms`);
}

/**
 * Install the hooks. Call once from the root layout's `onMount`, guarded by
 * `import.meta.env.DEV` at the call site so this module is not in release builds.
 */
export function installTestHooks(): void {
  if (window.asseteerTest) return; // HMR re-run — keep the collected errors

  captureErrors();

  const hooks: AsseteerTestHooks = {
    state: () => ({
      path: page.url.pathname,
      activeTab: viewState.activeTab,
      assetCount: assetsState.assets.length,
      totalCount: assetsState.totalCount,
      isLoading: assetsState.isLoading,
      searchText: assetsState.searchText,
      scanning: uiState.isScanning,
      processing: isAnyRunning(processingState),
      pending: { ...processingState.pendingCount },
    }),

    navigate: (path) => goto(path),

    folders: () => invoke<SourceFolder[]>('list_folders'),

    async addFolder(path) {
      const normalized = path.replace(/\\/g, '/');
      uiState.startScan(normalized);
      try {
        await invoke('add_folder', { path });
      } finally {
        uiState.endScan(normalized);
      }
      exploreState.clearCache();
      await exploreState.loadRoots(true);
      await processingState.refreshPendingCount();
      await emit('scan-complete');
    },

    async removeFolder(id) {
      await invoke('remove_folder', { folderId: id });
      exploreState.clearCache();
      await exploreState.loadRoots(true);
      await processingState.refreshPendingCount();
      await emit('scan-complete');
    },

    async processAll(categories = ['image', 'audio']) {
      for (const category of categories) await processCategory(category);
    },

    consoleErrors: () => [...errors],
    clearConsoleErrors: () => {
      errors.length = 0;
    },
  };

  window.asseteerTest = hooks;
  console.info('[asseteer] test hooks installed (window.asseteerTest)');
}
