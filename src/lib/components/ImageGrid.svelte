<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset } from '$lib/types';
  import { viewState } from '$lib/state/view.svelte';
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
      case 'small': return 'grid-cols-6 xl:grid-cols-8';
      case 'medium': return 'grid-cols-4 xl:grid-cols-6';
      case 'large': return 'grid-cols-3 xl:grid-cols-4';
    }
  });

  // Calculate columns based on viewport width and thumbnail size
  const columnCount = $derived.by(() => {
    const isXL = windowWidth >= 1280;

    switch (viewState.thumbnailSize) {
      case 'small': return isXL ? 8 : 6;
      case 'medium': return isXL ? 6 : 4;
      case 'large': return isXL ? 4 : 3;
    }
  });

  // Row height based on thumbnail size + padding + metadata
  const rowHeight = $derived.by(() => {
    switch (viewState.thumbnailSize) {
      case 'small': return 128 + 48 + 8; // h-32 + metadata height + gap
      case 'medium': return 192 + 56 + 8; // h-48 + metadata height + gap
      case 'large': return 256 + 64 + 8; // h-64 + metadata height + gap
    }
  });

  // Calculate virtual scrolling parameters
  const totalRows = $derived(Math.ceil(assets.length / columnCount));
  const totalHeight = $derived(totalRows * rowHeight);
  const visibleRows = $derived(Math.ceil(containerHeight / rowHeight) + 1);
  const bufferRows = 2; // Extra rows above and below for smooth scrolling

  const startRow = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - bufferRows));
  const endRow = $derived(Math.min(totalRows, startRow + visibleRows + bufferRows * 2));

  const startIndex = $derived(startRow * columnCount);
  const endIndex = $derived(Math.min(assets.length, endRow * columnCount));
  const visibleAssets = $derived(assets.slice(startIndex, endIndex));
  const offsetY = $derived(startRow * rowHeight);

  function handleImageClick(asset: Asset, index: number) {
    viewState.openLightbox(asset, index);
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

    // Update on window resize
    const resizeObserver = new ResizeObserver(() => {
      updateContainerHeight();
    });

    if (containerElement) {
      resizeObserver.observe(containerElement);
    }

    const handleResize = () => { windowWidth = window.innerWidth; };
    window.addEventListener('resize', handleResize);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener('resize', handleResize);
    };
  });
</script>

<div
  bind:this={containerElement}
  class="relative overflow-y-auto h-full"
  onscroll={handleScroll}
>
  <div style="height: {totalHeight}px; position: relative;">
    <div
      class="grid {gridClasses} gap-2 p-4 absolute w-full"
      style="transform: translateY({offsetY}px);"
    >
      {#each visibleAssets as asset, idx (asset.id)}
        {@const index = startIndex + idx}
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
  </div>
</div>
