<script lang="ts">
  import { untrack } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { loadAssetBlobUrl } from '$lib/utils/assetBlob';
  import { type Asset, getAssetFilePath } from '$lib/types';
  import {
    getThumbnailUrl,
    hasThumbnailFailed,
    requestThumbnail,
    cancelThumbnail,
    cacheReset,
  } from '$lib/state/thumbnails.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  interface Props {
    asset: Asset;
    size?: 'small' | 'medium' | 'large';
  }

  let { asset, size = 'medium' }: Props = $props();

  const THUMBNAIL_MAX = 128;

  const sizeClasses = $derived.by(() => {
    switch (size) {
      case 'small':
        return 'h-32';
      case 'medium':
        return 'h-48';
      case 'large':
        return 'h-64';
    }
  });

  // Small images don't need thumbnails — show original directly
  let isSmallImage = $derived(
    asset.width != null &&
      asset.height != null &&
      asset.width <= THUMBNAIL_MAX &&
      asset.height <= THUMBNAIL_MAX,
  );

  let thumbnailUrl = $derived(getThumbnailUrl(asset.id));
  let thumbnailFailed = $derived(hasThumbnailFailed(asset.id));

  // For small images: direct URL (regular file) or loaded blob URL (zip entry)
  let smallImageUrl = $state<string | null>(null);
  let smallImageFailed = $state(false);

  let isLoading = $derived(
    isSmallImage ? !smallImageUrl && !smallImageFailed : !thumbnailUrl && !thumbnailFailed,
  );

  let displayUrl = $derived(isSmallImage ? smallImageUrl : thumbnailUrl);

  $effect(() => {
    if (isSmallImage) {
      if (!asset.zip_entry) {
        // Regular file — use convertFileSrc for zero-IPC direct access
        smallImageUrl = convertFileSrc(getAssetFilePath(asset));
      } else {
        // Zip entry — need IPC to extract bytes
        loadAssetBlobUrl(asset.id, `image/${asset.format}`)
          .then((url) => {
            smallImageUrl = url;
          })
          .catch(() => {
            smallImageFailed = true;
          });
        return () => {
          // Revoke blob URL for zip entries
          if (smallImageUrl && asset.zip_entry) {
            URL.revokeObjectURL(smallImageUrl);
          }
        };
      }
    } else {
      void cacheReset.version; // re-run when cache is cleared so we re-request
      // untrack prevents cache.has() inside requestThumbnail from creating a
      // reactive dependency on the SvelteMap — without this, every cache.set()
      // (each thumbnail completion) would re-run ALL components' effects and
      // trigger a cancel+re-request cascade that grows the queue to 1000+.
      untrack(() => requestThumbnail(asset.id));
      return () => cancelThumbnail(asset.id);
    }
  });
</script>

<div class="w-full flex items-center justify-center bg-tertiary overflow-hidden {sizeClasses}">
  {#if isLoading}
    <div class="flex items-center justify-center w-full h-full">
      <Spinner size="md" />
    </div>
  {:else if displayUrl}
    <img src={displayUrl} alt="Thumbnail" class="w-full h-full object-contain" style={isSmallImage ? 'image-rendering: pixelated' : ''} />
  {:else}
    <div class="flex items-center justify-center w-full h-full">
      <span class="text-xs text-secondary">No preview</span>
    </div>
  {/if}
</div>
