<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset } from '$lib/types';
  import { formatFileSize } from '$lib/state/assets.svelte';
  import AssetThumbnail from './AssetThumbnail.svelte';
  import Badge from './shared/Badge.svelte';

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

  const startIndex = $derived(startRow);
  const endIndex = $derived(endRow);
  const visibleAssets = $derived(assets.slice(startIndex, endIndex));
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
    if (asset.zip_entry) {
      // Extract zip filename from path
      const zipName = asset.path.split(/[\\/]/).pop() || asset.path;
      return `${zipName}/${asset.zip_entry}`;
    }
    return asset.path;
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

<div
  bind:this={containerElement}
  class="relative overflow-y-auto h-full"
  onscroll={handleScroll}
>
  {#if isLoading}
    <div class="flex items-center justify-center h-full">
      <p class="text-secondary">Loading...</p>
    </div>
  {:else if assets.length === 0}
    <div class="flex items-center justify-center h-full">
      <p class="text-secondary">No assets found.</p>
    </div>
  {:else}
    <table class="w-full" style="height: {totalHeight}px;">
      <thead class="sticky top-0 bg-secondary border-b border-default z-10">
        <tr>
          <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Preview</th>
          <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Name</th>
          <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Type</th>
          <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Dimensions</th>
          <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Size</th>
        </tr>
      </thead>
      <tbody>
        <!-- Spacer for items before visible range -->
        {#if startIndex > 0}
          <tr style="height: {offsetY}px;"><td colspan="5"></td></tr>
        {/if}

        <!-- Visible items -->
        {#each visibleAssets as asset, idx (asset.id)}
          {@const index = startIndex + idx}
          <tr class="border-b border-default hover:bg-secondary" style="height: {rowHeight}px;">
            <td class="px-4 py-2">
              <AssetThumbnail assetId={asset.id} assetType={asset.asset_type} />
            </td>
            <td class="px-4 py-2 text-sm text-primary" title={formatLocation(asset)}>
              <div class="flex items-center gap-2">
                <span>{asset.filename}</span>
                {#if asset.zip_entry}
                  <Badge variant="info">ZIP</Badge>
                {/if}
              </div>
            </td>
            <td class="px-4 py-2 text-sm text-secondary">
              {asset.asset_type}
            </td>
            <td class="px-4 py-2 text-sm text-secondary">
              {formatDimensions(asset)}
            </td>
            <td class="px-4 py-2 text-sm text-secondary">
              {formatFileSize(asset.file_size)}
            </td>
          </tr>
        {/each}

        <!-- Spacer for items after visible range -->
        {#if endIndex < assets.length}
          <tr style="height: {totalHeight - offsetY - (visibleAssets.length * rowHeight)}px;"><td colspan="5"></td></tr>
        {/if}
      </tbody>
    </table>
  {/if}
</div>
