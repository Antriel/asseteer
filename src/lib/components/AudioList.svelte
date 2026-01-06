<script lang="ts">
  import type { Asset } from '$lib/types';
  import AudioPlayer from './AudioPlayer.svelte';
  import VirtualList from './shared/VirtualList.svelte';
  import { AudioIcon, PlayIcon, PauseIcon, FolderIcon } from './icons';
  import Badge from './shared/Badge.svelte';
  import { openPath } from '@tauri-apps/plugin-opener';

  // Extended asset type with optional similarity score
  type AudioAsset = Asset & { similarity?: number };

  interface Props {
    assets: AudioAsset[];
    showSimilarity?: boolean;
  }

  let { assets, showSimilarity = false }: Props = $props();

  function formatSimilarity(similarity: number): string {
    return `${Math.round(similarity * 100)}%`;
  }

  let selectedAsset = $state<Asset | null>(null);
  let isPlaying = $state(false);
  let shouldAutoPlay = $state(false);

  // Item height: button with h-20 (80px) + gap-2 (8px) = 88px per item
  const itemHeight = 88;

  function formatDuration(ms: number): string {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
  }

  function formatFileSize(bytes: number): string {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }

  function formatLocation(asset: Asset): string {
    if (asset.zip_entry) {
      return `${asset.path}/${asset.zip_entry}`;
    }
    return asset.path;
  }

  function formatDirectoryPath(asset: Asset): string {
    if (asset.zip_entry) {
      // For zip entries, show full zip path + internal path (without filename)
      const zipEntryParts = asset.zip_entry.split('/');
      const internalDir = zipEntryParts.slice(0, -1).join('/');
      return internalDir ? `${asset.path}/${internalDir}` : asset.path;
    }
    // For regular files, show directory path only
    const parts = asset.path.split(/[\\/]/);
    return parts.slice(0, -1).join('/') || '/';
  }

  function playAsset(asset: Asset) {
    // If clicking the same asset, just toggle play (handled by player)
    if (selectedAsset?.id === asset.id) {
      return;
    }
    selectedAsset = asset;
    shouldAutoPlay = true;
  }

  async function openDirectory(asset: Asset) {
    try {
      let dirPath: string;

      if (asset.zip_entry) {
        // For zip entries, combine zip path with the directory inside the zip
        const entryDir = asset.zip_entry.replace(/[^/]+$/, ''); // Remove filename, keep directory
        dirPath = `${asset.path}\\${entryDir.replace(/\//g, '\\')}`;
      } else {
        // For regular files, get the directory containing the file
        dirPath = asset.path.replace(/[^\\]+$/, '');
      }

      await openPath(dirPath);
    } catch (error) {
      console.error('Failed to open directory:', error);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full overflow-hidden">
  <!-- Single player at the top -->
  {#if selectedAsset}
    <div class="p-4 bg-primary border border-default rounded-lg shadow-lg flex-shrink-0">
      <div class="flex items-center gap-4 mb-3">
        <div class="w-12 h-12 flex items-center justify-center bg-accent rounded-lg flex-shrink-0">
          <AudioIcon size="lg" class="text-white" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-semibold text-primary whitespace-nowrap overflow-hidden text-ellipsis">
            {selectedAsset.filename}
          </p>
          <div class="flex gap-4 mt-1 text-xs text-secondary">
            {#if selectedAsset.duration_ms}
              <span>{formatDuration(selectedAsset.duration_ms)}</span>
            {/if}
            {#if selectedAsset.sample_rate}
              <span>{selectedAsset.sample_rate / 1000} kHz</span>
            {/if}
            {#if selectedAsset.channels}
              <span>{selectedAsset.channels === 1 ? 'Mono' : 'Stereo'}</span>
            {/if}
            <span>{selectedAsset.format.toUpperCase()}</span>
          </div>
        </div>
        <button
          class="w-8 h-8 flex items-center justify-center text-secondary hover:text-primary border-none bg-transparent rounded cursor-pointer transition-colors flex-shrink-0"
          onclick={() => openDirectory(selectedAsset!)}
          title="Open folder"
        >
          <FolderIcon size="sm" />
        </button>
      </div>
      <AudioPlayer
        asset={selectedAsset}
        isActive={true}
        autoPlay={shouldAutoPlay}
        onPlay={() => {
          isPlaying = true;
          shouldAutoPlay = false;
        }}
        onPause={() => isPlaying = false}
      />
    </div>
  {:else}
    <div class="p-6 bg-secondary border border-default rounded-lg text-center text-secondary flex-shrink-0">
      Select an audio file to play
    </div>
  {/if}

  <!-- List of audio assets with virtual scrolling -->
  <div class="flex-1 overflow-hidden">
    <VirtualList items={assets} {itemHeight} bufferItems={5}>
      {#snippet children({ visibleItems, startIndex })}
        <div class="flex flex-col gap-2">
          {#each visibleItems as asset, idx (asset.id)}
            <button
              class="flex items-center gap-4 p-4 bg-secondary border border-default rounded-lg transition-all hover:border-accent cursor-pointer text-left h-20"
              class:!bg-accent-light={selectedAsset?.id === asset.id}
              class:!border-accent={selectedAsset?.id === asset.id}
              onclick={() => playAsset(asset)}
              title={formatLocation(asset)}
            >
              <!-- Audio icon -->
              <div class="w-12 h-12 flex items-center justify-center bg-primary rounded-lg flex-shrink-0">
                <AudioIcon size="lg" class="text-secondary" />
              </div>

              <!-- Audio metadata -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 min-w-0">
                  <p class="font-semibold text-primary whitespace-nowrap overflow-hidden text-ellipsis flex-shrink-0">
                    {asset.filename}
                  </p>
                  <span class="text-xs text-secondary whitespace-nowrap overflow-hidden text-ellipsis flex-1 min-w-0" style="direction: rtl;">
                    {formatDirectoryPath(asset)}
                  </span>
                  {#if asset.zip_entry}
                    <Badge variant="info">ZIP</Badge>
                  {/if}
                </div>
                <div class="flex gap-4 mt-1 text-xs text-secondary">
                  {#if asset.duration_ms}
                    <span>{formatDuration(asset.duration_ms)}</span>
                  {/if}
                  {#if asset.sample_rate}
                    <span>{asset.sample_rate / 1000} kHz</span>
                  {/if}
                  {#if asset.channels}
                    <span>{asset.channels === 1 ? 'Mono' : 'Stereo'}</span>
                  {/if}
                  <span>{asset.format.toUpperCase()}</span>
                  <span>{formatFileSize(asset.file_size)}</span>
                </div>
              </div>

              <!-- Similarity score (semantic search) -->
              {#if showSimilarity && asset.similarity !== undefined}
                <div class="flex-shrink-0 px-2 py-1 bg-purple-100 dark:bg-purple-900/30 rounded text-xs font-medium text-purple-700 dark:text-purple-300">
                  {formatSimilarity(asset.similarity)}
                </div>
              {/if}

              <!-- Playing indicator -->
              {#if selectedAsset?.id === asset.id}
                <div class="flex-shrink-0">
                  {#if isPlaying}
                    <PauseIcon size="md" class="text-accent" circled />
                  {:else}
                    <PlayIcon size="md" class="text-accent" circled />
                  {/if}
                </div>
              {/if}
            </button>
          {/each}
        </div>
      {/snippet}
    </VirtualList>
  </div>
</div>
