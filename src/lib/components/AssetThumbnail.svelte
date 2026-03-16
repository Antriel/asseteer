<script lang="ts">
  import { onMount } from 'svelte';
  import { getThumbnailUrl, hasThumbnailFailed, requestThumbnail } from '$lib/state/thumbnails.svelte';

  interface Props {
    assetId: number;
    assetType: string;
  }

  let { assetId, assetType }: Props = $props();

  // Request thumbnail immediately on mount (table view, no lazy loading needed
  // since virtual scrolling only renders visible rows)
  onMount(() => {
    if (assetType === 'image') {
      requestThumbnail(assetId);
    }
  });

  let thumbnailUrl = $derived(getThumbnailUrl(assetId));
  let thumbnailFailed = $derived(hasThumbnailFailed(assetId));
  let isLoading = $derived(assetType === 'image' && !thumbnailUrl && !thumbnailFailed);
</script>

<div class="flex items-center justify-center w-16 h-16 bg-secondary border border-default rounded overflow-hidden">
  {#if isLoading}
    <span class="text-xs text-secondary">...</span>
  {:else if thumbnailUrl}
    <img src={thumbnailUrl} alt="Thumbnail" class="w-full h-full object-cover" />
  {:else if assetType === 'audio'}
    <span class="text-xs text-secondary">🎵</span>
  {:else}
    <span class="text-xs text-secondary">No preview</span>
  {/if}
</div>
