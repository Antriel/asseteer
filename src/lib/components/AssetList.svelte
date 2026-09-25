<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { Asset } from '$lib/types';
  import { getAssetDisplayPath } from '$lib/types';
  import { formatFileSize } from '$lib/utils/format';
  import AssetThumbnail from './AssetThumbnail.svelte';
  import Badge from './shared/Badge.svelte';
  import AssetContextMenu from './shared/AssetContextMenu.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { showInFolder, openDirectory, dragOut } from '$lib/actions/assetActions';
  import { ListSelection } from '$lib/state/listSelection.svelte';

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

  // Ctrl/Shift+click picks tiles for drag / Copy Path; a plain click opens the lightbox
  const selection = new ListSelection();

  $effect(() => {
    const list = assets;
    untrack(() => {
      if (selection.size > 0) selection.retain(list.map((a) => a.id));
    });
  });

  /** Returns true when the click was a selection gesture (and must not open anything). */
  function handleSelectClick(e: MouseEvent, asset: Asset): boolean {
    if (e.ctrlKey || e.metaKey) {
      selection.toggle(asset.id);
      return true;
    }
    if (e.shiftKey) {
      selection.extendTo(
        assets.map((a) => a.id),
        asset.id,
      );
      return true;
    }
    selection.clearAt(asset.id);
    return false;
  }

  // Context menu
  let contextMenu = $state<{ x: number; y: number; asset: Asset } | null>(null);

  function handleContextMenu(e: MouseEvent, asset: Asset) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY, asset };
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

{#if contextMenu}
  <AssetContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    asset={contextMenu.asset}
    targets={selection.targets(assets, contextMenu.asset)}
    onclose={() => (contextMenu = null)}
    onShowInFolder={(a) => showInFolder(a, 'image')}
    onOpenDirectory={openDirectory}
  />
{/if}

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
          <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
          <div
            class="grid grid-cols-[80px_1fr_100px_120px_100px] items-center px-4 border-b border-default select-none {selection.has(
              asset.id,
            )
              ? 'bg-accent-light'
              : 'hover:bg-secondary'}"
            style="height: {rowHeight}px;"
            onclick={(e) => handleSelectClick(e, asset)}
            oncontextmenu={(e) => handleContextMenu(e, asset)}
            {@attach dragOut(() => selection.targets(assets, asset))}
          >
            <button
              class="py-2 cursor-pointer"
              onclick={(e) => {
                // Modifier clicks bubble to the row as selection gestures
                if (!e.ctrlKey && !e.metaKey && !e.shiftKey) viewState.openLightbox(asset);
              }}
            >
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
