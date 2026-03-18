<script lang="ts">
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { invoke } from '@tauri-apps/api/core';
  import type { Asset } from '$lib/types';
  import {
    getThumbnailUrl,
    hasThumbnailFailed,
    requestThumbnail,
    cancelThumbnail,
  } from '$lib/state/thumbnails.svelte';

  interface Props {
    asset: Asset;
  }

  let { asset }: Props = $props();

  const THUMBNAIL_MAX = 128;

  let isSmallImage = $derived(
    asset.asset_type === 'image' &&
      asset.width != null &&
      asset.height != null &&
      asset.width <= THUMBNAIL_MAX &&
      asset.height <= THUMBNAIL_MAX,
  );

  let thumbnailUrl = $derived(getThumbnailUrl(asset.id));
  let thumbnailFailed = $derived(hasThumbnailFailed(asset.id));

  let smallImageUrl = $state<string | null>(null);
  let smallImageFailed = $state(false);

  let isLoading = $derived(
    asset.asset_type !== 'image'
      ? false
      : isSmallImage
        ? !smallImageUrl && !smallImageFailed
        : !thumbnailUrl && !thumbnailFailed,
  );

  let displayUrl = $derived(isSmallImage ? smallImageUrl : thumbnailUrl);

  onMount(() => {
    if (asset.asset_type !== 'image') return;

    if (isSmallImage) {
      if (!asset.zip_entry) {
        smallImageUrl = convertFileSrc(asset.path);
      } else {
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

<div
  class="flex items-center justify-center w-16 h-16 bg-secondary border border-default rounded overflow-hidden"
>
  {#if isLoading}
    <span class="text-xs text-secondary">...</span>
  {:else if displayUrl}
    <img src={displayUrl} alt="Thumbnail" class="w-full h-full object-cover" />
  {:else if asset.asset_type === 'audio'}
    <span class="text-xs text-secondary">🎵</span>
  {:else}
    <span class="text-xs text-secondary">No preview</span>
  {/if}
</div>
