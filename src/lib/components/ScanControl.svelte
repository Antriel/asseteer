<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { uiState } from '$lib/state/ui.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { onMount } from 'svelte';

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

  async function processAssets() {
    uiState.isProcessing = true;
    uiState.processProgress = 'Starting processing...';

    try {
      // Process images
      uiState.processProgress = 'Processing images...';
      const imageCount = await invoke<number>('process_pending_images');

      // Process audio
      uiState.processProgress = 'Processing audio...';
      const audioCount = await invoke<number>('process_pending_audio');

      uiState.processProgress = `Processed ${imageCount} images and ${audioCount} audio files!`;

      // Reload assets
      await assetsState.loadAssets();
    } catch (error) {
      console.error('Failed to process:', error);
      uiState.processProgress = `Error: ${error}`;
    } finally {
      uiState.isProcessing = false;
    }
  }

  // Listen to process progress events
  onMount(() => {
    const unlisten = listen('process-progress', (event: any) => {
      const { total, processed, status } = event.payload;
      if (status === 'complete') {
        uiState.processProgress = `Processing complete!`;
      } else {
        uiState.processProgress = `Processing: ${processed}/${total}`;
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  });
</script>

<div class="flex flex-col gap-4 p-4 bg-secondary border-b border-default">
  <div class="flex items-center gap-4">
    <button
      onclick={selectFolder}
      disabled={uiState.isScanning || uiState.isProcessing}
      class="btn btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {uiState.isScanning ? 'Scanning...' : 'Scan Folder'}
    </button>

    <button
      onclick={processAssets}
      disabled={uiState.isProcessing || uiState.isScanning || assetsState.totalCount === 0}
      class="btn btn-secondary disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {uiState.isProcessing ? 'Processing...' : 'Process Assets'}
    </button>

    {#if uiState.scanProgress}
      <span class="text-sm text-secondary">{uiState.scanProgress}</span>
    {/if}

    {#if uiState.processProgress}
      <span class="text-sm text-secondary">{uiState.processProgress}</span>
    {/if}
  </div>

  {#if assetsState.totalCount > 0}
    <div class="text-sm text-secondary">
      Total assets: {assetsState.totalCount}
    </div>
  {/if}
</div>
