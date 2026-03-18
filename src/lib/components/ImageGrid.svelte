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
