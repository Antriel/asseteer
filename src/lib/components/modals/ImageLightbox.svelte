<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { loadAssetBlobUrl } from '$lib/utils/assetBlob';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { getAssetFilePath, getAssetDisplayPath, getAssetDirectoryPath } from '$lib/types';
  import type { Asset, FolderLocation } from '$lib/types';
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { exploreState } from '$lib/state/explore.svelte';

  interface Props {
    asset: Asset;
    onClose: () => void;
    onNext?: () => void;
    onPrev?: () => void;
  }

  let { asset, onClose, onNext, onPrev }: Props = $props();

  async function openInExplorer() {
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
      console.error('[ImageLightbox] Failed to open in explorer:', error);
    }
  }

  async function showInFolder() {
    viewState.openFolderSidebar();
    await exploreState.loadRoots();
    await exploreState.navigateToAsset(asset);
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
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
    onClose();
  }

  let scale = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let isPanning = $state(false);
  let lastPointer = $state({ x: 0, y: 0 });
  let naturalWidth = $state(0);
  let naturalHeight = $state(0);
  let hasFittedOnce = $state(false);
  let containerEl: HTMLDivElement | undefined = $state();
  let showMetadata = $state(false);
  let showBounds = $state(false);
  let imageSrc = $state<string>('');
  let blobUrl = $state<string | null>(null);
  let loading = $state(true);

  const MIN_SCALE = 0.1;
  const MAX_SCALE = 40;

  function fitToView() {
    if (!containerEl || !naturalWidth || !naturalHeight) return;
    const rect = containerEl.getBoundingClientRect();
    const padding = 60;
    const scaleX = (rect.width - padding) / naturalWidth;
    const scaleY = (rect.height - padding) / naturalHeight;
    scale = Math.min(scaleX, scaleY, 1);
    panX = 0;
    panY = 0;
  }

  function actualSize() {
    scale = 1;
    panX = 0;
    panY = 0;
  }

  function zoomIn() {
    scale = Math.min(scale * 1.25, MAX_SCALE);
  }

  function zoomOut() {
    scale = Math.max(scale / 1.25, MIN_SCALE);
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const factor = e.deltaY > 0 ? 1 / 1.1 : 1.1;
    const newScale = Math.min(Math.max(scale * factor, MIN_SCALE), MAX_SCALE);

    // Zoom toward cursor position
    const rect = containerEl!.getBoundingClientRect();
    const cx = e.clientX - rect.left - rect.width / 2;
    const cy = e.clientY - rect.top - rect.height / 2;

    panX = cx - ((cx - panX) * newScale) / scale;
    panY = cy - ((cy - panY) * newScale) / scale;
    scale = newScale;
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    isPanning = true;
    lastPointer = { x: e.clientX, y: e.clientY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!isPanning) return;
    panX += e.clientX - lastPointer.x;
    panY += e.clientY - lastPointer.y;
    lastPointer = { x: e.clientX, y: e.clientY };
  }

  function onPointerUp() {
    isPanning = false;
  }

  // Load image when asset changes
  $effect(() => {
    const assetId = asset.id;
    const zipEntry = asset.zip_entry;
    const assetPath = getAssetFilePath(asset);
    const assetFormat = asset.format;

    untrack(() => {
      if (blobUrl) {
        URL.revokeObjectURL(blobUrl);
        blobUrl = null;
      }

      loading = true;
      hasFittedOnce = false;

      (async () => {
        try {
          if (zipEntry) {
            const newBlobUrl = await loadAssetBlobUrl(assetId, `image/${assetFormat}`);

            untrack(() => {
              blobUrl = newBlobUrl;
              imageSrc = newBlobUrl;
              loading = false;
            });
          } else {
            const src = convertFileSrc(assetPath);
            untrack(() => {
              imageSrc = src;
              loading = false;
            });
          }
        } catch (error) {
          console.error('[ImageLightbox] Failed to load image:', error);
          untrack(() => {
            imageSrc = '';
            loading = false;
          });
        }
      })();
    });
  });

  // Cleanup on unmount
  $effect(() => {
    return () => {
      untrack(() => {
        if (blobUrl) {
          URL.revokeObjectURL(blobUrl);
        }
      });
    };
  });

  function onImageLoad(e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    naturalWidth = img.naturalWidth;
    naturalHeight = img.naturalHeight;
    if (!hasFittedOnce) {
      fitToView();
      hasFittedOnce = true;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'Escape':
        onClose();
        break;
      case 'ArrowLeft':
        onPrev?.();
        break;
      case 'ArrowRight':
        onNext?.();
        break;
      case '+':
      case '=':
        zoomIn();
        break;
      case '-':
        zoomOut();
        break;
      case '0':
        fitToView();
        break;
      case '1':
        actualSize();
        break;
      case 'b':
      case 'B':
        showBounds = !showBounds;
        break;
      case 'i':
      case 'I':
        showMetadata = !showMetadata;
        break;
    }
  }

  let zoomPercent = $derived(Math.round(scale * 100));

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
    return () => document.removeEventListener('keydown', handleKeydown);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-[1000] flex flex-col bg-neutral-900/95">
  <!-- Toolbar -->
  <div class="flex items-center gap-1.5 px-3 py-1.5 bg-black/60 shrink-0 text-white">
    <span class="text-sm font-medium truncate mr-auto">
      {asset.filename}
      {#if naturalWidth && naturalHeight}
        <span class="text-neutral-400 ml-1.5 text-xs">{naturalWidth} &times; {naturalHeight}</span>
      {/if}
    </span>

    <button class="btn-lightbox-tool" onclick={showInFolder} title="Show in folder">
      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"
        ><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
        /></svg
      >
    </button>
    <button class="btn-lightbox-tool" onclick={openInExplorer} title="Open in file explorer">
      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"
        ><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
        /></svg
      >
    </button>

    <span class="w-px h-4 bg-white/20 mx-1"></span>

    <button
      class="px-1.5 py-0.5 text-xs rounded transition-colors {showBounds ? 'bg-white/20 text-white' : 'text-neutral-400 hover:text-white'}"
      onclick={() => (showBounds = !showBounds)}
      title="Toggle image bounds (B)"
    >
      Bounds
    </button>
    <button
      class="px-1.5 py-0.5 text-xs rounded transition-colors {showMetadata ? 'bg-white/20 text-white' : 'text-neutral-400 hover:text-white'}"
      onclick={() => (showMetadata = !showMetadata)}
      title="Image details (I)"
    >
      Info
    </button>

    <span class="w-px h-4 bg-white/20 mx-1"></span>

    <button class="btn-lightbox-tool" onclick={fitToView} title="Fit to view (0)">Fit</button>
    <button class="btn-lightbox-tool" onclick={actualSize} title="Actual size (1)">1:1</button>

    <span class="w-px h-4 bg-white/20 mx-1"></span>

    <button class="btn-lightbox-tool" onclick={zoomOut} title="Zoom out (-)">
      &minus;
    </button>
    <span class="text-xs text-neutral-300 w-12 text-center tabular-nums">{zoomPercent}%</span>
    <button class="btn-lightbox-tool" onclick={zoomIn} title="Zoom in (+)">
      +
    </button>

    <span class="w-px h-4 bg-white/20 mx-1"></span>

    <button class="btn-lightbox-tool" onclick={onClose} title="Close (Esc)">
      &times;
    </button>
  </div>

  <!-- Canvas area -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="flex-1 overflow-hidden select-none {isPanning ? 'cursor-grabbing' : 'cursor-grab'}"
    bind:this={containerEl}
    onwheel={onWheel}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
  >
    <!-- Navigation arrows -->
    {#if onPrev}
      <button
        class="absolute top-1/2 -translate-y-1/2 left-4 btn-lightbox-nav z-10"
        onclick={(e) => { e.stopPropagation(); onPrev!(); }}
        onpointerdown={(e) => e.stopPropagation()}
      >
        &#8249;
      </button>
    {/if}
    {#if onNext}
      <button
        class="absolute top-1/2 -translate-y-1/2 right-4 btn-lightbox-nav z-10"
        onclick={(e) => { e.stopPropagation(); onNext!(); }}
        onpointerdown={(e) => e.stopPropagation()}
      >
        &#8250;
      </button>
    {/if}

    {#if loading}
      <div class="w-full h-full flex items-center justify-center text-white text-lg">
        Loading...
      </div>
    {:else if imageSrc}
      <div class="w-full h-full flex items-center justify-center" style="pointer-events: none;">
        <div
          style="display: inline-block; transform: translate({panX}px, {panY}px) scale({scale}); transform-origin: center; background: repeating-conic-gradient(#555 0% 25%, #444 0% 50%) 50% / 16px 16px; {showBounds ? 'outline: 2px dashed rgba(120, 180, 255, 0.8); outline-offset: 0px;' : ''}"
        >
          <img
            src={imageSrc}
            alt={asset.filename}
            style="opacity: {hasFittedOnce ? 1 : 0}; transition: opacity 100ms; display: block; image-rendering: {scale >= 4 ? 'pixelated' : 'auto'};"
            class="max-w-none"
            onload={onImageLoad}
            draggable="false"
          />
        </div>
      </div>
    {:else}
      <div class="w-full h-full flex items-center justify-center text-white text-lg">
        Failed to load image
      </div>
    {/if}
  </div>

  <!-- Metadata panel -->
  {#if showMetadata}
    <div class="absolute top-12 right-3 w-[280px] p-4 bg-black/85 text-white rounded-lg backdrop-blur-sm z-10">
      <h3 class="text-sm font-semibold mb-2">Image Details</h3>
      <dl class="space-y-1.5 text-xs">
        <div>
          <dt class="text-neutral-400">Filename</dt>
          <dd>{asset.filename}</dd>
        </div>
        <div>
          <dt class="text-neutral-400">Path</dt>
          <dd class="break-all">{getAssetDisplayPath(asset)}</dd>
        </div>
        {#if asset.width && asset.height}
          <div>
            <dt class="text-neutral-400">Dimensions</dt>
            <dd>{asset.width} &times; {asset.height} px</dd>
          </div>
        {/if}
        <div>
          <dt class="text-neutral-400">Format</dt>
          <dd>{asset.format.toUpperCase()}</dd>
        </div>
        <div>
          <dt class="text-neutral-400">File Size</dt>
          <dd>{(asset.file_size / 1024).toFixed(1)} KB</dd>
        </div>
      </dl>
    </div>
  {/if}

  <!-- Bottom info bar -->
  <div class="flex items-center justify-between px-3 py-1 bg-black/60 text-xs text-neutral-400 shrink-0">
    <span class="truncate">{getAssetDisplayPath(asset)}</span>
    {#if asset.file_size}
      <span class="shrink-0 ml-4">{(asset.file_size / 1024).toFixed(0)} KB</span>
    {/if}
  </div>
</div>
