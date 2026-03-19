<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset } from '$lib/types';
  import { getAssetDisplayPath } from '$lib/types';
  import { formatFileSize } from '$lib/state/assets.svelte';
  import AssetThumbnail from './AssetThumbnail.svelte';
  import Badge from './shared/Badge.svelte';
  import { viewState } from '$lib/state/view.svelte';

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
          <div
            class="grid grid-cols-[80px_1fr_100px_120px_100px] items-center px-4 border-b border-default hover:bg-secondary"
            style="height: {rowHeight}px;"
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
