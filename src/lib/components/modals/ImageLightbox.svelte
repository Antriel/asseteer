<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { invoke } from '@tauri-apps/api/core';
  import { getAssetFilePath, getAssetDisplayPath } from '$lib/types';
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

  let zoom = $state(1);
  let showMetadata = $state(false);
  let imageSrc = $state<string>('');
  let blobUrl = $state<string | null>(null);
  let loading = $state(true);

  // Load image when asset changes - track only asset.id
  $effect(() => {
    // Track the asset.id (this is what triggers the effect)
    const assetId = asset.id;
    const zipEntry = asset.zip_entry;
    const assetPath = getAssetFilePath(asset);
    const assetFormat = asset.format;

    // Use untrack to prevent state updates from re-triggering the effect
    untrack(() => {
      // Clean up previous blob URL if exists
      if (blobUrl) {
        URL.revokeObjectURL(blobUrl);
        blobUrl = null;
      }

      loading = true;

      // Load the new asset
      (async () => {
        try {
          if (zipEntry) {
            // Asset is inside a zip - need to extract it
            const bytes = await invoke<number[]>('get_asset_bytes', { assetId });
            const blob = new Blob([new Uint8Array(bytes)], { type: `image/${assetFormat}` });
            const newBlobUrl = URL.createObjectURL(blob);

            untrack(() => {
              blobUrl = newBlobUrl;
              imageSrc = newBlobUrl;
              loading = false;
            });
          } else {
            // Regular file - use convertFileSrc
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
        zoom = Math.min(zoom + 0.5, 5);
        break;
      case '-':
        zoom = Math.max(zoom - 0.5, 0.5);
        break;
      case '0':
        zoom = 1;
        break;
      case 'i':
      case 'I':
        showMetadata = !showMetadata;
        break;
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
    return () => document.removeEventListener('keydown', handleKeydown);
  });
</script>

<div
  class="fixed inset-0 bg-black/90 flex items-center justify-center z-[1000]"
  role="button"
  tabindex="0"
  onclick={onClose}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') onClose();
  }}
>
  <div
    class="relative w-[90vw] h-[90vh] flex flex-col"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    <!-- Close button -->
    <button class="absolute top-4 right-4 btn-lightbox-nav z-10" onclick={onClose}> × </button>

    <!-- Navigation -->
    {#if onPrev}
      <button class="absolute top-1/2 -translate-y-1/2 left-4 btn-lightbox-nav" onclick={onPrev}>
        ‹
      </button>
    {/if}
    {#if onNext}
      <button class="absolute top-1/2 -translate-y-1/2 right-4 btn-lightbox-nav" onclick={onNext}>
        ›
      </button>
    {/if}

    <!-- Image display -->
    <div class="flex-1 flex items-center justify-center overflow-auto">
      {#if loading}
        <div class="text-white text-center">
          <div class="text-2xl mb-2">Loading...</div>
        </div>
      {:else if imageSrc}
        <img
          src={imageSrc}
          alt={asset.filename}
          style="transform: scale({zoom})"
          class="max-w-full max-h-full object-contain transition-transform duration-200"
        />
      {:else}
        <div class="text-white text-center">
          <div class="text-2xl mb-2">Failed to load image</div>
        </div>
      {/if}
    </div>

    <!-- Controls -->
    <div class="flex justify-between items-center p-4 bg-black/80 text-white">
      <div>
        <p class="font-medium">{asset.filename}</p>
        {#if asset.width && asset.height}
          <p class="text-sm text-gray-300">
            {asset.width} × {asset.height} • {(asset.file_size / 1024).toFixed(0)} KB
          </p>
        {/if}
      </div>

      <div class="flex gap-2 items-center">
        <button class="btn-lightbox-control" onclick={showInFolder} title="Show in folder">
          <svg class="w-4 h-4 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24"
            ><path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
            /></svg
          >
        </button>
        <span class="w-px h-5 bg-gray-600"></span>
        <button class="btn-lightbox-control" onclick={() => (zoom = Math.max(zoom - 0.5, 0.5))}
          >−</button
        >
        <span class="min-w-[4rem] text-center">{Math.round(zoom * 100)}%</span>
        <button class="btn-lightbox-control" onclick={() => (zoom = Math.min(zoom + 0.5, 5))}
          >+</button
        >
        <button class="btn-lightbox-control" onclick={() => (zoom = 1)}>Reset</button>
        <button class="btn-lightbox-control" onclick={() => (showMetadata = !showMetadata)}
          >Info</button
        >
      </div>
    </div>

    <!-- Metadata panel -->
    {#if showMetadata}
      <div class="absolute top-16 right-4 w-[300px] p-4 bg-black/90 text-white rounded-lg">
        <h3 class="text-lg font-semibold mb-3">Image Details</h3>
        <dl class="space-y-2">
          <div>
            <dt class="text-sm text-gray-400">Filename:</dt>
            <dd class="text-sm">{asset.filename}</dd>
          </div>

          <div>
            <dt class="text-sm text-gray-400">Path:</dt>
            <dd class="text-sm break-all">{getAssetDisplayPath(asset)}</dd>
          </div>

          {#if asset.width && asset.height}
            <div>
              <dt class="text-sm text-gray-400">Dimensions:</dt>
              <dd class="text-sm">{asset.width} × {asset.height} px</dd>
            </div>
          {/if}

          <div>
            <dt class="text-sm text-gray-400">Format:</dt>
            <dd class="text-sm">{asset.format.toUpperCase()}</dd>
          </div>

          <div>
            <dt class="text-sm text-gray-400">File Size:</dt>
            <dd class="text-sm">{(asset.file_size / 1024).toFixed(2)} KB</dd>
          </div>
        </dl>
      </div>
    {/if}
  </div>
</div>
