<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset, FolderLocation } from '$lib/types';
  import { getAssetDirectoryPath } from '$lib/types';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { FolderIcon } from '$lib/components/icons';
  import ImageThumbnail from './ImageThumbnail.svelte';

  interface Props {
    assets: Asset[];
  }

  let { assets }: Props = $props();

  let containerElement: HTMLDivElement;
  let scrollTop = $state(0);
  let containerHeight = $state(0);
  let windowWidth = $state(typeof window !== 'undefined' ? window.innerWidth : 1280);

  // Computed grid column classes and counts
  const gridClasses = $derived.by(() => {
    switch (viewState.thumbnailSize) {
      case 'small':
        return 'grid-cols-6 xl:grid-cols-8';
      case 'medium':
        return 'grid-cols-4 xl:grid-cols-6';
      case 'large':
        return 'grid-cols-3 xl:grid-cols-4';
    }
  });

  // Calculate columns based on viewport width and thumbnail size
  const columnCount = $derived.by(() => {
    const isXL = windowWidth >= 1280;

    switch (viewState.thumbnailSize) {
      case 'small':
        return isXL ? 8 : 6;
      case 'medium':
        return isXL ? 6 : 4;
      case 'large':
        return isXL ? 4 : 3;
    }
  });

  // Fixed row heights per size mode — never measure from DOM to prevent oscillation.
  // Thumbnail height + fixed info area (40px) + gap (8px) + border (2px)
  const rowHeight = $derived.by(() => {
    switch (viewState.thumbnailSize) {
      case 'small':
        return 128 + 40 + 8 + 2; // 178
      case 'medium':
        return 192 + 40 + 8 + 2; // 242
      case 'large':
        return 256 + 40 + 8 + 2; // 306
    }
  });

  // Calculate virtual scrolling parameters
  const totalRows = $derived(Math.ceil(assets.length / columnCount));
  const totalHeight = $derived(totalRows * rowHeight);
  const visibleRows = $derived(Math.ceil(containerHeight / rowHeight) + 1);
  const bufferRows = 2;

  const startRow = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - bufferRows));
  const endRow = $derived(Math.min(totalRows, startRow + visibleRows + bufferRows * 2));

  const startIndex = $derived(startRow * columnCount);
  const endIndex = $derived(Math.min(assets.length, endRow * columnCount));
  const visibleAssets = $derived(assets.slice(startIndex, endIndex));
  const offsetY = $derived(startRow * rowHeight);

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
    const assetType = 'image';
    let location: FolderLocation;
    if (asset.zip_file) {
      const zipParts = (asset.zip_entry ?? '').split('/').filter(Boolean);
      const zipDirParts = zipParts.slice(0, -1);
      const zipPrefix = zipDirParts.length > 0 ? zipDirParts.join('/') + '/' : '';
      location = { type: 'zip', folderId: asset.folder_id, relPath: asset.rel_path, zipFile: asset.zip_file, zipPrefix };
    } else {
      location = { type: 'folder', folderId: asset.folder_id, relPath: asset.rel_path };
    }
    assetsState.setFolderFilter(location, assetType);
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

  function handleImageClick(asset: Asset) {
    viewState.openLightbox(asset);
  }

  function handleScroll(event: Event) {
    const target = event.target as HTMLDivElement;
    scrollTop = target.scrollTop;
  }

  function updateContainerHeight() {
    if (containerElement) {
      containerHeight = containerElement.clientHeight;
    }
    windowWidth = window.innerWidth;
  }

  onMount(() => {
    updateContainerHeight();

    const resizeObserver = new ResizeObserver(() => {
      updateContainerHeight();
    });

    if (containerElement) {
      resizeObserver.observe(containerElement);
    }

    const handleResize = () => {
      windowWidth = window.innerWidth;
    };
    window.addEventListener('resize', handleResize);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener('resize', handleResize);
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
  <div style="height: {totalHeight}px; position: relative;">
    <div
      class="grid {gridClasses} gap-2 p-4 absolute w-full"
      style="transform: translateY({offsetY}px);"
    >
      {#each visibleAssets as asset (asset.id)}
        <button
          class="relative bg-secondary border border-default rounded-lg overflow-hidden cursor-pointer hover:border-accent hover:shadow-md hover:-translate-y-0.5"
          onclick={() => handleImageClick(asset)}
          oncontextmenu={(e) => handleContextMenu(e, asset)}
        >
          <ImageThumbnail {asset} size={viewState.thumbnailSize} />
          {#if asset.format === 'gif'}
            <span
              class="absolute top-1.5 left-1.5 text-[0.625rem] font-bold leading-none px-1 py-0.5 rounded bg-black/60 text-white tracking-wide"
              >GIF</span
            >
          {/if}

          <div class="h-10 p-2 bg-primary">
            <p
              class="text-xs font-medium text-primary whitespace-nowrap overflow-hidden text-ellipsis"
              title={asset.filename}
            >
              {asset.filename}
            </p>
            {#if asset.width && asset.height}
              <p class="text-[0.625rem] text-secondary mt-0.5">
                {asset.width} × {asset.height}
              </p>
            {/if}
          </div>
        </button>
      {/each}
    </div>
  </div>
</div>
