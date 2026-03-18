<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount, onDestroy } from 'svelte';
  import { uiState } from '$lib/state/ui.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { processingState } from '$lib/state/tasks.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  // Event payload from Rust backend
  interface ScanProgressEvent {
    phase: 'discovering' | 'inserting' | 'scanning' | 'complete';
    files_found: number;
    files_inserted: number;
    files_total: number;
    zips_scanned: number;
    current_path: string | null;
  }

  let unlisten: UnlistenFn | null = null;

  // Re-establish listener on mount if scan is in progress
  onMount(async () => {
    if (uiState.isScanning && !unlisten) {
      unlisten = await listen<ScanProgressEvent>('scan-progress', (event) => {
        handleProgress(event.payload);
      });
    }
  });

  onDestroy(() => {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  });

  function handleProgress(event: ScanProgressEvent) {
    uiState.scanDetails.phase = event.phase;
    uiState.scanDetails.filesFound = event.files_found;
    uiState.scanDetails.filesInserted = event.files_inserted;
    uiState.scanDetails.filesTotal = event.files_total;
    uiState.scanDetails.zipsScanned = event.zips_scanned;

    // Update summary progress string
    if (event.phase === 'discovering') {
      const zipInfo = event.zips_scanned > 0 ? ` (${event.zips_scanned} zips)` : '';
      uiState.scanProgress = `Discovering files... ${event.files_found} found${zipInfo}`;
    } else if (event.phase === 'scanning') {
      const zipInfo = event.zips_scanned > 0 ? ` (${event.zips_scanned} zips)` : '';
      if (event.files_total > 0) {
        // Discovery done, show insertion progress
        const pct = Math.round((event.files_inserted / event.files_total) * 100);
        uiState.scanProgress = `Saving to database... ${event.files_inserted}/${event.files_total} (${pct}%)`;
      } else {
        uiState.scanProgress = `Scanning... ${event.files_found} found, ${event.files_inserted} saved${zipInfo}`;
      }
    } else if (event.phase === 'inserting') {
      const pct =
        event.files_total > 0 ? Math.round((event.files_inserted / event.files_total) * 100) : 0;
      uiState.scanProgress = `Saving to database... ${event.files_inserted}/${event.files_total} (${pct}%)`;
    } else {
      uiState.scanProgress = `Scan complete! ${event.files_found} assets discovered.`;
    }
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
    uiState.resetScanDetails();
    uiState.scanDetails.phase = 'discovering';
    uiState.scanDetails.currentPath = path;

    // Clean up any existing listener before registering a new one
    if (unlisten) {
      unlisten();
      unlisten = null;
    }

    // Set up progress listener
    unlisten = await listen<ScanProgressEvent>('scan-progress', (event) => {
      handleProgress(event.payload);
    });

    try {
      const sessionId = await invoke<number>('start_scan', { rootPath: path });
      uiState.currentSessionId = sessionId;
      uiState.scanDetails.phase = 'complete';

      // Reload assets for current tab and refresh pending count
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
      await processingState.refreshPendingCount();

      // Emit custom event to notify other pages to refresh
      await emit('scan-complete');

      // Clear progress message after delay
      setTimeout(() => {
        uiState.scanProgress = '';
        uiState.scanDetails.phase = 'idle';
      }, 5000);
    } catch (error) {
      console.error('Failed to scan:', error);
      uiState.scanProgress = `Error: ${error}`;
      uiState.scanDetails.phase = 'idle';
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
    uiState.scanDetails.filesTotal > 0
      ? Math.round((uiState.scanDetails.filesInserted / uiState.scanDetails.filesTotal) * 100)
      : 0,
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
            {#if uiState.scanDetails.phase === 'discovering'}
              Discovering Files
            {:else if uiState.scanDetails.phase === 'scanning'}
              {#if uiState.scanDetails.filesTotal > 0}
                Saving to Database
              {:else}
                Scanning Files
              {/if}
            {:else if uiState.scanDetails.phase === 'inserting'}
              Saving to Database
            {:else}
              Scanning...
            {/if}
          </h3>

          {#if uiState.scanDetails.phase === 'discovering'}
            <div class="text-center mb-4">
              <p class="text-3xl font-bold text-accent">{uiState.scanDetails.filesFound}</p>
              <p class="text-sm text-secondary">files found</p>
              {#if uiState.scanDetails.zipsScanned > 0}
                <p class="text-xs text-tertiary mt-1">
                  {uiState.scanDetails.zipsScanned} zip archives scanned
                </p>
              {/if}
            </div>
          {:else if uiState.scanDetails.phase === 'scanning'}
            <div class="mb-4">
              {#if uiState.scanDetails.filesTotal > 0}
                <!-- Discovery done: show progress bar for remaining inserts -->
                <div class="flex items-center justify-between text-sm mb-2">
                  <span class="text-secondary">Progress</span>
                  <span class="text-primary font-medium"
                    >{uiState.scanDetails.filesInserted} / {uiState.scanDetails.filesTotal}</span
                  >
                </div>
                <div class="h-2 bg-tertiary rounded-full overflow-hidden">
                  <div
                    class="h-full bg-accent transition-all duration-300"
                    style="width: {progressPercent}%"
                  ></div>
                </div>
                <p class="text-xs text-tertiary text-center mt-2">{progressPercent}% complete</p>
              {:else}
                <!-- Discovery + insertion happening concurrently -->
                <div class="flex items-center justify-between text-sm mb-2">
                  <div class="text-center flex-1">
                    <p class="text-2xl font-bold text-accent">{uiState.scanDetails.filesFound}</p>
                    <p class="text-xs text-secondary">found</p>
                  </div>
                  <div class="text-center flex-1">
                    <p class="text-2xl font-bold text-success">
                      {uiState.scanDetails.filesInserted}
                    </p>
                    <p class="text-xs text-secondary">saved</p>
                  </div>
                </div>
                {#if uiState.scanDetails.zipsScanned > 0}
                  <p class="text-xs text-tertiary text-center mt-1">
                    {uiState.scanDetails.zipsScanned} zip archives scanned
                  </p>
                {/if}
              {/if}
            </div>
          {:else if uiState.scanDetails.phase === 'inserting'}
            <div class="mb-4">
              <div class="flex items-center justify-between text-sm mb-2">
                <span class="text-secondary">Progress</span>
                <span class="text-primary font-medium"
                  >{uiState.scanDetails.filesInserted} / {uiState.scanDetails.filesTotal}</span
                >
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

          <p
            class="text-xs text-tertiary text-center truncate"
            title={uiState.scanDetails.currentPath}
          >
            {uiState.scanDetails.currentPath}
          </p>
        </div>
      </div>
    {:else if uiState.scanDetails.phase === 'complete'}
      <!-- Scan complete -->
      <div class="text-center">
        <div
          class="w-20 h-20 rounded-full bg-success/10 flex items-center justify-center mx-auto mb-4"
        >
          <svg class="w-10 h-10 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M5 13l4 4L19 7"
            />
          </svg>
        </div>
        <h3 class="text-xl font-semibold text-primary mb-2">Scan Complete</h3>
        <p class="text-secondary mb-2">{uiState.scanDetails.filesFound} assets discovered</p>
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
        <div
          class="w-20 h-20 rounded-full bg-accent/10 group-hover:bg-accent/20 flex items-center justify-center mb-4 transition-default"
        >
          <svg class="w-10 h-10 text-accent" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.5"
              d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
            />
          </svg>
        </div>
        <h3 class="text-xl font-semibold text-primary mb-2">Select a Folder</h3>
        <p class="text-sm text-secondary text-center">
          Choose a folder to scan for images and audio files
        </p>
        <p class="text-xs text-tertiary mt-2">Supports ZIP archives</p>
      </button>

      <!-- Asset stats -->
      {#if assetsState.totalCount > 0}
        <div class="mt-8 text-center">
          <p class="text-sm text-secondary">
            Total assets in library: <span class="font-semibold text-primary"
              >{assetsState.totalCount}</span
            >
          </p>
        </div>
      {/if}
    {/if}
  </div>
</div>
