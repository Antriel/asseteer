<script lang="ts">
  import type { Asset } from '$lib/types';
  import AudioPlayer from './AudioPlayer.svelte';
  import VirtualList from './shared/VirtualList.svelte';
  import { AudioIcon, FolderIcon, SearchIcon } from './icons';
  import Badge from './shared/Badge.svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import { ZIP_SEP } from '$lib/database/queries';

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
  let shouldAutoPlay = $state(false);
  let playKey = $state(0);
  let audioPlayerRef = $state<ReturnType<typeof AudioPlayer> | null>(null);
  let virtualListRef = $state<ReturnType<typeof VirtualList> | null>(null);
  // Track if audio should auto-play on navigation (true while playing or after natural end, false after manual pause)
  let shouldContinuePlaying = $state(false);
  let containerRef = $state<HTMLDivElement | null>(null);

  // Item height: button with h-20 (80px) + gap-2 (8px) = 88px per item
  const itemHeight = 88;

  function formatDuration(ms: number): string {
    const totalSeconds = ms / 1000;
    const minutes = Math.floor(totalSeconds / 60);
    if (totalSeconds < 10) {
      const secs = totalSeconds % 60;
      const wholeSecs = Math.floor(secs);
      const millis = Math.floor((secs - wholeSecs) * 1000);
      return `${minutes}:${wholeSecs.toString().padStart(2, '0')}.${millis.toString().padStart(3, '0')}`;
    }
    const remainingSeconds = Math.floor(totalSeconds % 60);
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
    if (selectedAsset?.id === asset.id) {
      // Same asset - restart playback from beginning
      playKey++;
      shouldAutoPlay = true;
      shouldContinuePlaying = true;
    } else {
      selectedAsset = asset;
      shouldAutoPlay = true;
      shouldContinuePlaying = true;
    }
    // Refocus container so keyboard navigation continues working
    containerRef?.focus();
  }

  async function showInFolder(asset: Asset) {
    viewState.openFolderSidebar();
    await exploreState.loadRoots();
    await exploreState.navigateToAssetPath(asset.path, asset.zip_entry ?? undefined);
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    if (asset.zip_entry) {
      // Build the correct ZIP folder filter including the internal directory
      const zipParts = asset.zip_entry.split('/').filter(Boolean);
      const zipDirParts = zipParts.slice(0, -1);
      const zipPrefix = zipDirParts.length > 0 ? zipDirParts.join('/') + '/' : '';
      assetsState.setFolderFilter(asset.path + ZIP_SEP + zipPrefix, assetType);
    } else {
      assetsState.setFolderFilter(asset.path, assetType);
    }
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

  function getSelectedIndex(): number {
    if (!selectedAsset) return -1;
    return assets.findIndex((a) => a.id === selectedAsset!.id);
  }

  function navigateToIndex(newIndex: number) {
    if (newIndex < 0 || newIndex >= assets.length) return;

    const newAsset = assets[newIndex];
    const wasPlaying = shouldContinuePlaying;

    selectedAsset = newAsset;

    // Scroll to make the item visible with 1 item buffer
    virtualListRef?.scrollToIndex(newIndex, 1);

    if (wasPlaying) {
      shouldAutoPlay = true;
      shouldContinuePlaying = true;
    }
  }

  // Context menu
  let contextMenu = $state<{ x: number; y: number; asset: AudioAsset } | null>(null);

  function handleContextMenu(e: MouseEvent, asset: AudioAsset) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY, asset };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function findSimilar(asset: AudioAsset) {
    closeContextMenu();
    try {
      await clapState.searchBySimilarity(asset.id, asset.filename, undefined, assetsState.durationFilter);
    } catch (error) {
      showToast(`${error}`, 'error');
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    const currentIndex = getSelectedIndex();

    // Arrow Up / Shift+Tab - navigate up
    if (e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey)) {
      e.preventDefault();
      if (currentIndex <= 0) {
        // Already at top or no selection - select first item
        navigateToIndex(0);
      } else {
        navigateToIndex(currentIndex - 1);
      }
      return;
    }

    // Arrow Down / Tab - navigate down
    if (e.key === 'ArrowDown' || (e.key === 'Tab' && !e.shiftKey)) {
      e.preventDefault();
      if (currentIndex < 0) {
        // No selection - select first item
        navigateToIndex(0);
      } else if (currentIndex < assets.length - 1) {
        navigateToIndex(currentIndex + 1);
      }
      return;
    }

    // Space - toggle play/pause
    if (e.key === ' ') {
      e.preventDefault();
      if (!selectedAsset && assets.length > 0) {
        // No selection - select and play first item
        selectedAsset = assets[0];
        shouldAutoPlay = true;
        shouldContinuePlaying = true;
      } else if (audioPlayerRef) {
        audioPlayerRef.toggle();
      }
      return;
    }

    // Arrow Left - seek backward 10%
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      if (audioPlayerRef && selectedAsset) {
        audioPlayerRef.seekByPercent(-0.1);
      }
      return;
    }

    // Arrow Right - seek forward 10%
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      if (audioPlayerRef && selectedAsset) {
        const result = audioPlayerRef.seekByPercent(0.1);
        if (result.stopped) {
          // Seeking past end stopped playback - but keep shouldContinuePlaying true
          // so navigation will auto-play next item
        }
      }
      return;
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="flex flex-col gap-4 p-4 h-full overflow-hidden outline-none"
  bind:this={containerRef}
  tabindex="0"
  role="application"
  aria-label="Audio list player"
  onkeydown={handleKeyDown}
>
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
            <span
              >{selectedAsset.duration_ms ? formatDuration(selectedAsset.duration_ms) : '—'}</span
            >
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
          class="w-8 h-8 flex items-center justify-center text-secondary hover:text-purple-500 border-none bg-transparent rounded cursor-pointer transition-colors flex-shrink-0"
          onclick={() => findSimilar(selectedAsset!)}
          title="Find similar sounds"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
          </svg>
        </button>
        <button
          class="w-8 h-8 flex items-center justify-center text-secondary hover:text-accent border-none bg-transparent rounded cursor-pointer transition-colors flex-shrink-0"
          onclick={() => showInFolder(selectedAsset!)}
          title="Show in folder"
        >
          <FolderIcon size="sm" />
        </button>
        <button
          class="w-8 h-8 flex items-center justify-center text-secondary hover:text-primary border-none bg-transparent rounded cursor-pointer transition-colors flex-shrink-0"
          onclick={() => openDirectory(selectedAsset!)}
          title="Open in file explorer"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
            />
          </svg>
        </button>
      </div>
      <AudioPlayer
        bind:this={audioPlayerRef}
        asset={selectedAsset}
        isActive={true}
        autoPlay={shouldAutoPlay}
        restartKey={playKey}
        onPlay={() => {
          shouldAutoPlay = false;
          shouldContinuePlaying = true;
        }}
        onPause={() => {
          // Manual pause (not from onEnded) - stop auto-playing on navigation
          shouldContinuePlaying = false;
        }}
        onEnded={() => {
          // Natural end - keep shouldContinuePlaying true so navigation auto-plays
          // Note: onPause is called before onEnded, so we need to restore it
          shouldContinuePlaying = true;
        }}
      />
    </div>
  {:else}
    <div
      class="p-6 bg-secondary border border-default rounded-lg text-center text-secondary flex-shrink-0"
    >
      Select an audio file to play
    </div>
  {/if}

  <!-- List of audio assets with virtual scrolling -->
  <div class="flex-1 overflow-hidden">
    <VirtualList bind:this={virtualListRef} items={assets} {itemHeight} bufferItems={5}>
      {#snippet children({ visibleItems, startIndex })}
        <div class="flex flex-col gap-2">
          {#each visibleItems as asset, idx (asset.id)}
            <button
              class="flex items-center gap-4 p-4 bg-secondary border border-default rounded-lg transition-all hover:border-accent cursor-pointer text-left h-20 focus:outline-none"
              class:!bg-accent-light={selectedAsset?.id === asset.id}
              class:!border-accent={selectedAsset?.id === asset.id}
              onclick={() => playAsset(asset)}
              oncontextmenu={(e) => handleContextMenu(e, asset)}
              tabindex="-1"
              title={formatLocation(asset)}
            >
              <!-- Audio icon -->
              <div
                class="w-12 h-12 flex items-center justify-center rounded-lg flex-shrink-0"
                class:bg-accent={selectedAsset?.id === asset.id}
                class:bg-primary={selectedAsset?.id !== asset.id}
              >
                <AudioIcon
                  size="lg"
                  class={selectedAsset?.id === asset.id ? 'text-white' : 'text-secondary'}
                />
              </div>

              <!-- Audio metadata -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 min-w-0">
                  <p
                    class="font-semibold text-primary whitespace-nowrap overflow-hidden text-ellipsis flex-shrink-0"
                  >
                    {asset.filename}
                  </p>
                  <span
                    class="text-xs text-secondary whitespace-nowrap overflow-hidden text-ellipsis flex-1 min-w-0"
                    style="direction: rtl;"
                  >
                    {formatDirectoryPath(asset)}
                  </span>
                  {#if asset.zip_entry}
                    <Badge variant="info">ZIP</Badge>
                  {/if}
                </div>
                <div class="flex gap-4 mt-1 text-xs text-secondary">
                  <span>{asset.duration_ms ? formatDuration(asset.duration_ms) : '—'}</span>
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
                <div
                  class="flex-shrink-0 px-2 py-1 bg-purple-100 dark:bg-purple-900/30 rounded text-xs font-medium text-purple-700 dark:text-purple-300"
                >
                  {formatSimilarity(asset.similarity)}
                </div>
              {/if}
            </button>
          {/each}
        </div>
      {/snippet}
    </VirtualList>
  </div>
</div>

<!-- Context menu -->
{#if contextMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 z-50"
    onclick={closeContextMenu}
    oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}
  >
    <div
      class="absolute bg-elevated border border-default rounded-lg shadow-lg py-1 min-w-[180px]"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
    >
      <button
        class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
        onclick={() => findSimilar(contextMenu!.asset)}
      >
        <svg class="w-4 h-4 text-purple-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
        </svg>
        Find Similar Sounds
      </button>
      <button
        class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
        onclick={() => { closeContextMenu(); showInFolder(contextMenu!.asset); }}
      >
        <FolderIcon size="sm" class="text-secondary" />
        Show in Folder
      </button>
      <button
        class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
        onclick={() => { closeContextMenu(); openDirectory(contextMenu!.asset); }}
      >
        <svg class="w-4 h-4 text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
        </svg>
        Open in File Explorer
      </button>
    </div>
  </div>
{/if}
