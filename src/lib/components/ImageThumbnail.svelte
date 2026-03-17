<script lang="ts">
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { invoke } from '@tauri-apps/api/core';
  import type { Asset } from '$lib/types';
  import { getThumbnailUrl, hasThumbnailFailed, requestThumbnail, cancelThumbnail } from '$lib/state/thumbnails.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  interface Props {
    asset: Asset;
    size?: 'small' | 'medium' | 'large';
  }

  let { asset, size = 'medium' }: Props = $props();

  const THUMBNAIL_MAX = 128;

  const sizeClasses = $derived.by(() => {
    switch (size) {
      case 'small': return 'h-32';
      case 'medium': return 'h-48';
      case 'large': return 'h-64';
    }
  });

  // Small images don't need thumbnails — show original directly
  let isSmallImage = $derived(
    asset.width != null && asset.height != null &&
    asset.width <= THUMBNAIL_MAX && asset.height <= THUMBNAIL_MAX
  );

  let thumbnailUrl = $derived(getThumbnailUrl(asset.id));
  let thumbnailFailed = $derived(hasThumbnailFailed(asset.id));

  // For small images: direct URL (regular file) or loaded blob URL (zip entry)
  let smallImageUrl = $state<string | null>(null);
  let smallImageFailed = $state(false);

  let isLoading = $derived(
    isSmallImage
      ? !smallImageUrl && !smallImageFailed
      : !thumbnailUrl && !thumbnailFailed
  );

  let displayUrl = $derived(isSmallImage ? smallImageUrl : thumbnailUrl);

  onMount(() => {
    if (isSmallImage) {
      if (!asset.zip_entry) {
        // Regular file — use convertFileSrc for zero-IPC direct access
        smallImageUrl = convertFileSrc(asset.path);
      } else {
        // Zip entry — need IPC to extract bytes
        invoke<number[]>('get_asset_bytes', { assetId: asset.id })
          .then((bytes) => {
            const arr = new Uint8Array(bytes);
            const blob = new Blob([arr], { type: `image/${asset.format}` });
            smallImageUrl = URL.createObjectURL(blob);
          })
          .catch(() => {
            smallImageFailed = true;
          });
      }
      return () => {
        // Revoke blob URL for zip entries
        if (smallImageUrl && asset.zip_entry) {
          URL.revokeObjectURL(smallImageUrl);
        }
      };
    } else {
      requestThumbnail(asset.id);
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
    <img
      src={displayUrl}
      alt="Thumbnail"
      class="w-full h-full object-cover"
    />
  {:else}
    <div class="flex items-center justify-center w-full h-full">
      <span class="text-xs text-secondary">No preview</span>
    </div>
  {/if}
</div>
