<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset, FolderLocation } from '$lib/types';
  import { getAssetDisplayPath, getAssetDirectoryPath } from '$lib/types';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { formatFileSize } from '$lib/state/assets.svelte';
  import AssetThumbnail from './AssetThumbnail.svelte';
  import Badge from './shared/Badge.svelte';
  import { FolderIcon } from '$lib/components/icons';
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { exploreState } from '$lib/state/explore.svelte';

  interface Props {
    assets: Asset[];
    isLoading?: boolean;
  }

  let { assets, isLoading = false }: Props = $props();

  let containerElement: HTMLDivElement;
  let scrollTop = $state(0);
  let containerHeight = $state(0);

  // Row height: thumbnail (64px) + padding (16px top/bottom) = 80px + 1px border
  const rowHeight = 81;
  const bufferRows = 5; // Extra rows above and below for smooth scrolling

  // Calculate virtual scrolling parameters
  const totalRows = $derived(assets.length);
  const totalHeight = $derived(totalRows * rowHeight);
  const visibleRows = $derived(Math.ceil(containerHeight / rowHeight) + 1);

  const startRow = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - bufferRows));
  const endRow = $derived(Math.min(totalRows, startRow + visibleRows + bufferRows * 2));

  const visibleAssets = $derived(assets.slice(startRow, endRow));
  const offsetY = $derived(startRow * rowHeight);

  function formatDimensions(asset: Asset): string {
    if (asset.width && asset.height) {
      return `${asset.width} × ${asset.height}`;
    } else if (asset.duration_ms) {
      return `${(asset.duration_ms / 1000).toFixed(1)}s`;
    }
    return '—';
  }

  function formatLocation(asset: Asset): string {
    return getAssetDisplayPath(asset);
  }

  // Context menu
  let contextMenu = $state<{ x: number; y: number; asset: Asset } | null>(null);

  function handleContextMenu(e: MouseEvent, asset: Asset) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY, asset };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function showInFolder(asset: Asset) {
    viewState.openFolderSidebar();
    await exploreState.loadRoots();
    await exploreState.navigateToAsset(asset);
    let location: FolderLocation;
    if (asset.zip_file) {
      const zipParts = (asset.zip_entry ?? '').split('/').filter(Boolean);
      const zipDirParts = zipParts.slice(0, -1);
      const zipPrefix = zipDirParts.length > 0 ? zipDirParts.join('/') + '/' : '';
      location = { type: 'zip', folderId: asset.folder_id, relPath: asset.rel_path, zipFile: asset.zip_file, zipPrefix };
    } else {
      location = { type: 'folder', folderId: asset.folder_id, relPath: asset.rel_path };
    }
    assetsState.setFolderFilter(location, 'image');
  }

  async function openDirectory(asset: Asset) {
    try {
      let dirPath: string;
      if (asset.zip_file) {
        dirPath = asset.rel_path
          ? asset.folder_path + '\\' + asset.rel_path.replace(/\//g, '\\')
          : asset.folder_path;
      } else {
        dirPath = getAssetDirectoryPath(asset).replace(/\//g, '\\');
      }
      await openPath(dirPath);
    } catch (error) {
      console.error('Failed to open directory:', error);
    }
  }

  function handleScroll(event: Event) {
    const target = event.target as HTMLDivElement;
    scrollTop = target.scrollTop;
  }

  function updateContainerHeight() {
    if (containerElement) {
      containerHeight = containerElement.clientHeight;
    }
  }

  onMount(() => {
    updateContainerHeight();

    // Update on window resize
    const resizeObserver = new ResizeObserver(() => {
      updateContainerHeight();
    });

    if (containerElement) {
      resizeObserver.observe(containerElement);
    }

    return () => {
      resizeObserver.disconnect();
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
{#if contextMenu}
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
        onclick={() => { const a = contextMenu!.asset; closeContextMenu(); showInFolder(a); }}
      >
        <FolderIcon size="sm" class="text-secondary" />
        Show in Folder
      </button>
      <button
        class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
        onclick={() => { const a = contextMenu!.asset; closeContextMenu(); openDirectory(a); }}
      >
        <svg class="w-4 h-4 text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
        </svg>
        Open in File Explorer
      </button>
    </div>
  </div>
{/if}

<div bind:this={containerElement} class="relative overflow-y-auto h-full" onscroll={handleScroll}>
  {#if isLoading}
    <div class="flex items-center justify-center h-full">
      <p class="text-secondary">Loading...</p>
    </div>
  {:else if assets.length === 0}
    <div class="flex items-center justify-center h-full">
      <p class="text-secondary">No assets found.</p>
    </div>
  {:else}
    <!-- Header row (sticky) -->
    <div
      class="sticky top-0 bg-secondary border-b border-default z-10 grid grid-cols-[80px_1fr_100px_120px_100px] px-4 py-2 text-sm font-medium text-secondary"
    >
      <span>Preview</span>
      <span>Name</span>
      <span>Type</span>
      <span>Dimensions</span>
      <span>Size</span>
    </div>

    <!-- Virtual scroll container -->
    <div style="height: {totalHeight}px; position: relative;">
      <div class="absolute w-full" style="transform: translateY({offsetY}px);">
        {#each visibleAssets as asset (asset.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="grid grid-cols-[80px_1fr_100px_120px_100px] items-center px-4 border-b border-default hover:bg-secondary"
            style="height: {rowHeight}px;"
            oncontextmenu={(e) => handleContextMenu(e, asset)}
          >
            <button class="py-2 cursor-pointer" onclick={() => viewState.openLightbox(asset)}>
              <AssetThumbnail {asset} />
            </button>
            <div class="py-2 text-sm text-primary" title={formatLocation(asset)}>
              <div class="flex items-center gap-2">
                <span>{asset.filename}</span>
                {#if asset.format === 'gif'}
                  <Badge variant="info">GIF</Badge>
                {/if}
                {#if asset.zip_entry}
                  <Badge variant="info">ZIP</Badge>
                {/if}
              </div>
            </div>
            <div class="py-2 text-sm text-secondary">
              {asset.asset_type}
            </div>
            <div class="py-2 text-sm text-secondary">
              {formatDimensions(asset)}
            </div>
            <div class="py-2 text-sm text-secondary">
              {formatFileSize(asset.file_size)}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
