<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { uiState } from '$lib/state/ui.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { processingState } from '$lib/state/tasks.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  interface ScanProgress {
    phase: 'discovering' | 'inserting' | 'scanning' | 'complete';
    files_found: number;
    files_inserted: number;
    files_total: number;
    zips_scanned: number;
    current_path: string | null;
  }

  let unlisten: UnlistenFn | null = null;

  function formatProgress(progress: ScanProgress): string {
    if (progress.phase === 'discovering') {
      const zipInfo = progress.zips_scanned > 0 ? ` (${progress.zips_scanned} zips)` : '';
      return `Discovering files... ${progress.files_found} found${zipInfo}`;
    }
    if (progress.phase === 'scanning') {
      const zipInfo = progress.zips_scanned > 0 ? ` (${progress.zips_scanned} zips)` : '';
      if (progress.files_total > 0) {
        const pct = Math.round((progress.files_inserted / progress.files_total) * 100);
        return `Saving to database... ${progress.files_inserted}/${progress.files_total} (${pct}%)`;
      }
      return `Scanning... ${progress.files_found} found, ${progress.files_inserted} saved${zipInfo}`;
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

    // Set up progress listener
    unlisten = await listen<ScanProgress>('scan-progress', (event) => {
      uiState.scanProgress = formatProgress(event.payload);
    });

    try {
      const sessionId = await invoke<number>('start_scan', { rootPath: path });
      uiState.currentSessionId = sessionId;

      // Reload assets for current tab and refresh pending count
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
      await processingState.refreshPendingCount();

      // Emit custom event to notify parent to refresh asset counts
      await emit('scan-complete');

      // Clear progress message after delay
      setTimeout(() => {
        uiState.scanProgress = '';
      }, 3000);
    } catch (error) {
      console.error('Failed to scan:', error);
      uiState.scanProgress = `Error: ${error}`;
    } finally {
      uiState.isScanning = false;
      // Clean up listener
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 bg-secondary border-b border-default">
  <div class="flex items-center gap-4">
    <button
      onclick={selectFolder}
      disabled={uiState.isScanning}
      class="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {uiState.isScanning ? 'Scanning...' : 'Scan Folder'}
    </button>

    {#if uiState.scanProgress}
      <div class="flex items-center gap-2">
        {#if uiState.isScanning}
          <Spinner size="sm" color="blue" />
        {/if}
        <span class="text-sm text-secondary">{uiState.scanProgress}</span>
      </div>
    {/if}
  </div>

  {#if assetsState.totalCount > 0}
    <div class="text-sm text-secondary">
      Total assets discovered: <span class="font-semibold text-primary">{assetsState.totalCount}</span>
    </div>
  {/if}
</div>
