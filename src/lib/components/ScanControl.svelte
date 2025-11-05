<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { emit } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { uiState } from '$lib/state/ui.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { processingState } from '$lib/state/tasks.svelte';
  import { viewState } from '$lib/state/view.svelte';

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
    uiState.scanProgress = 'Scanning directory...';

    try {
      const sessionId = await invoke<number>('start_scan', { rootPath: path });
      uiState.currentSessionId = sessionId;
      uiState.scanProgress = 'Scan complete! Assets discovered.';

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
          <div class="w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
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
