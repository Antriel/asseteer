<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onDestroy } from 'svelte';
  import { uiState } from '$lib/state/ui.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { processingState } from '$lib/state/tasks.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  interface ScanProgress {
    phase: 'discovering' | 'inserting' | 'complete';
    files_found: number;
    files_inserted: number;
    files_total: number;
    zips_scanned: number;
    current_path: string | null;
  }

  let unlisten: UnlistenFn | null = null;
  let scanPhase = $state<'idle' | 'discovering' | 'inserting' | 'complete'>('idle');
  let filesFound = $state(0);
  let filesInserted = $state(0);
  let filesTotal = $state(0);
  let zipsScanned = $state(0);
  let currentPath = $state<string | null>(null);

  onDestroy(() => {
    if (unlisten) {
      unlisten();
    }
  });

  function formatProgress(progress: ScanProgress): string {
    if (progress.phase === 'discovering') {
      const zipInfo = progress.zips_scanned > 0 ? ` (${progress.zips_scanned} zips)` : '';
      return `Discovering files... ${progress.files_found} found${zipInfo}`;
    }
    if (progress.phase === 'inserting') {
      const pct = progress.files_total > 0
        ? Math.round((progress.files_inserted / progress.files_total) * 100)
        : 0;
      return `Saving to database... ${progress.files_inserted}/${progress.files_total} (${pct}%)`;
    }
    return `Scan complete! ${progress.files_found} assets discovered.`;
  }

  async function selectFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder to scan',
      });

      if (selected && typeof selected === 'string') {
        await startScan(selected);
      }
    } catch (error) {
      console.error('Failed to select folder:', error);
    }
  }

  async function startScan(path: string) {
    uiState.isScanning = true;
    uiState.scanProgress = 'Starting scan...';
    scanPhase = 'discovering';
    filesFound = 0;
    filesInserted = 0;
    filesTotal = 0;
    zipsScanned = 0;
    currentPath = path;

    // Set up progress listener
    unlisten = await listen<ScanProgress>('scan-progress', (event) => {
      const progress = event.payload;
      uiState.scanProgress = formatProgress(progress);
      scanPhase = progress.phase;
      filesFound = progress.files_found;
      filesInserted = progress.files_inserted;
      filesTotal = progress.files_total;
      zipsScanned = progress.zips_scanned;
    });

    try {
      const sessionId = await invoke<number>('start_scan', { rootPath: path });
      uiState.currentSessionId = sessionId;
      scanPhase = 'complete';

      // Reload assets for current tab and refresh pending count
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
      await processingState.refreshPendingCount();

      // Emit custom event to notify other pages to refresh
      await emit('scan-complete');

      // Clear progress message after delay
      setTimeout(() => {
        uiState.scanProgress = '';
        scanPhase = 'idle';
      }, 5000);
    } catch (error) {
      console.error('Failed to scan:', error);
      uiState.scanProgress = `Error: ${error}`;
      scanPhase = 'idle';
    } finally {
      uiState.isScanning = false;
      // Clean up listener
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    }
  }

  let progressPercent = $derived(
    filesTotal > 0 ? Math.round((filesInserted / filesTotal) * 100) : 0
  );
</script>

<div class="flex flex-col h-full overflow-auto p-6">
  <!-- Page Header -->
  <div class="mb-6">
    <h1 class="text-2xl font-bold text-primary">Scan Folder</h1>
    <p class="text-sm text-secondary mt-1">Discover and import assets from your filesystem</p>
  </div>

  <!-- Main scan area -->
  <div class="flex-1 flex flex-col items-center justify-center">
    {#if uiState.isScanning}
      <!-- Scanning in progress -->
      <div class="w-full max-w-md">
        <div class="bg-secondary border border-default rounded-xl p-6">
          <div class="flex items-center justify-center mb-4">
            <Spinner size="lg" />
          </div>

          <h3 class="text-lg font-semibold text-primary text-center mb-2">
            {#if scanPhase === 'discovering'}
              Discovering Files
            {:else if scanPhase === 'inserting'}
              Saving to Database
            {:else}
              Scanning...
            {/if}
          </h3>

          {#if scanPhase === 'discovering'}
            <div class="text-center mb-4">
              <p class="text-3xl font-bold text-accent">{filesFound}</p>
              <p class="text-sm text-secondary">files found</p>
              {#if zipsScanned > 0}
                <p class="text-xs text-tertiary mt-1">{zipsScanned} zip archives scanned</p>
              {/if}
            </div>
          {:else if scanPhase === 'inserting'}
            <div class="mb-4">
              <div class="flex items-center justify-between text-sm mb-2">
                <span class="text-secondary">Progress</span>
                <span class="text-primary font-medium">{filesInserted} / {filesTotal}</span>
              </div>
              <div class="h-2 bg-tertiary rounded-full overflow-hidden">
                <div
                  class="h-full bg-accent transition-all duration-300"
                  style="width: {progressPercent}%"
                ></div>
              </div>
              <p class="text-xs text-tertiary text-center mt-2">{progressPercent}% complete</p>
            </div>
          {/if}

          <p class="text-xs text-tertiary text-center truncate" title={currentPath}>
            {currentPath}
          </p>
        </div>
      </div>
    {:else if scanPhase === 'complete'}
      <!-- Scan complete -->
      <div class="text-center">
        <div class="w-20 h-20 rounded-full bg-success/10 flex items-center justify-center mx-auto mb-4">
          <svg class="w-10 h-10 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
        </div>
        <h3 class="text-xl font-semibold text-primary mb-2">Scan Complete</h3>
        <p class="text-secondary mb-2">{filesFound} assets discovered</p>
        <div class="flex items-center justify-center gap-4 mt-6">
          <button
            onclick={selectFolder}
            class="px-4 py-2 text-sm font-medium text-secondary border border-default hover:bg-tertiary rounded-lg transition-default"
          >
            Scan Another
          </button>
          <a
            href="/library"
            class="px-4 py-2 text-sm font-medium text-white bg-accent hover:opacity-90 rounded-lg transition-default"
          >
            View Library
          </a>
        </div>
      </div>
    {:else}
      <!-- Idle state - prompt to scan -->
      <button
        onclick={selectFolder}
        class="group flex flex-col items-center justify-center w-full max-w-md p-12 border-2 border-dashed border-default hover:border-accent rounded-2xl transition-default cursor-pointer bg-secondary/50 hover:bg-secondary"
      >
        <div class="w-20 h-20 rounded-full bg-accent/10 group-hover:bg-accent/20 flex items-center justify-center mb-4 transition-default">
          <svg class="w-10 h-10 text-accent" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
          </svg>
        </div>
        <h3 class="text-xl font-semibold text-primary mb-2">Select a Folder</h3>
        <p class="text-sm text-secondary text-center">
          Choose a folder to scan for images and audio files
        </p>
        <p class="text-xs text-tertiary mt-2">
          Supports ZIP archives
        </p>
      </button>

      <!-- Asset stats -->
      {#if assetsState.totalCount > 0}
        <div class="mt-8 text-center">
          <p class="text-sm text-secondary">
            Total assets in library: <span class="font-semibold text-primary">{assetsState.totalCount}</span>
          </p>
        </div>
      {/if}
    {/if}
  </div>
</div>
