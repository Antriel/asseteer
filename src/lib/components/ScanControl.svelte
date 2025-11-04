<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { uiState } from '$lib/state/ui.svelte';
  import { assetsState } from '$lib/state/assets.svelte';

  async function selectFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder to scan',
      });

      if (selected && typeof selected === 'string') {
        startScan(selected);
      }
    } catch (error) {
      console.error('Failed to select folder:', error);
    }
  }

  async function startScan(path: string) {
    uiState.isScanning = true;
    uiState.scanProgress = 'Starting scan...';

    try {
      const sessionId = await invoke<number>('start_scan', { rootPath: path });
      uiState.currentSessionId = sessionId;
      uiState.scanProgress = 'Scan complete!';

      // Reload assets
      await assetsState.loadAssets();
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
      class="btn btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {uiState.isScanning ? 'Scanning...' : 'Scan Folder'}
    </button>

    {#if uiState.scanProgress}
      <span class="text-sm text-secondary">{uiState.scanProgress}</span>
    {/if}
  </div>

  {#if assetsState.totalCount > 0}
    <div class="text-sm text-secondary">
      Total assets: {assetsState.totalCount}
    </div>
  {/if}
</div>
