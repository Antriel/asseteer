<script lang="ts">
  import type { Asset } from '$lib/types';
  import { viewState } from '$lib/state/view.svelte';
  import ImageThumbnail from './ImageThumbnail.svelte';

  interface Props {
    assets: Asset[];
  }

  let { assets }: Props = $props();

  // Computed grid column classes based on thumbnail size
  const gridClasses = $derived.by(() => {
    switch (viewState.thumbnailSize) {
      case 'small': return 'grid-cols-6 xl:grid-cols-8';
      case 'medium': return 'grid-cols-4 xl:grid-cols-6';
      case 'large': return 'grid-cols-3 xl:grid-cols-4';
    }
  });

  function handleImageClick(asset: Asset, index: number) {
    viewState.openLightbox(asset, index);
  }
</script>

<div class="grid {gridClasses} gap-2 p-4">
  {#each assets as asset, index (asset.id)}
    <button
      class="relative bg-secondary border border-default rounded-lg overflow-hidden transition-all cursor-pointer hover:border-accent hover:shadow-md hover:-translate-y-0.5"
      onclick={() => handleImageClick(asset, index)}
    >
      <ImageThumbnail assetId={asset.id} size={viewState.thumbnailSize} />

      <div class="p-2 bg-primary">
        <p class="text-xs font-medium text-primary whitespace-nowrap overflow-hidden text-ellipsis" title={asset.filename}>
          {asset.filename}
        </p>
        {#if asset.width && asset.height}
          <p class="text-[0.625rem] text-secondary mt-1">
            {asset.width} × {asset.height}
          </p>
        {/if}
      </div>
    </button>
  {/each}
</div>
