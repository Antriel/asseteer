<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { getDatabase } from '$lib/database/connection';
  import { getAssetTypeCounts } from '$lib/database/queries';
  import type { ProcessingProgress } from '$lib/state/tasks.svelte';

  import ScanControl from '$lib/components/ScanControl.svelte';
  import TaskProgress from '$lib/components/TaskProgress.svelte';
  import TabBar from '$lib/components/shared/TabBar.svelte';
  import Toolbar from '$lib/components/shared/Toolbar.svelte';
  import ImageGrid from '$lib/components/ImageGrid.svelte';
  import AudioList from '$lib/components/AudioList.svelte';
  import AssetList from '$lib/components/AssetList.svelte';
  import ImageLightbox from '$lib/components/modals/ImageLightbox.svelte';

  let assetCounts = $state({ images: 0, audio: 0 });
  let unlistenFns: UnlistenFn[] = [];

  async function refreshAssetCounts() {
    const db = await getDatabase();
    assetCounts = await getAssetTypeCounts(db);
  }

  onMount(async () => {
    await refreshAssetCounts();

    // Load initial assets (images by default)
    assetsState.loadAssets('image');

    // Listen for scan completion to refresh counts
    const unlistenScan = await listen('scan-complete', async () => {
      console.log('[Main] Scan complete, refreshing asset counts');
      await refreshAssetCounts();
    });

    // Listen for processing completion to refresh counts
    const unlistenComplete = await listen<ProcessingProgress>('processing-complete', async () => {
      console.log('[Main] Processing complete, refreshing asset counts and reloading assets');
      await refreshAssetCounts();
      // Reload current tab's assets
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
    });

    unlistenFns.push(unlistenScan, unlistenComplete);
  });

  onDestroy(() => {
    unlistenFns.forEach((fn) => fn());
  });

  // Assets are already filtered by loadAssets() based on the current tab
  // No need for additional filtering here
  let displayedAssets = $derived(assetsState.assets);
</script>

<div class="flex flex-col h-screen bg-primary">
  <!-- Header -->
  <header class="px-6 py-4 border-b border-default bg-secondary">
    <h1 class="text-xl font-bold text-primary">Asset Manager</h1>
  </header>

  <!-- Scan Control -->
  <ScanControl />

  <!-- Task Progress -->
  <div class="px-4 py-2">
    <TaskProgress />
  </div>

  <!-- Tab Navigation -->
  <TabBar imageCount={assetCounts.images} audioCount={assetCounts.audio} />

  <!-- Toolbar (search, filters, view controls) -->
  <Toolbar />

  <!-- Main Content Area -->
  <main class="flex-1 overflow-y-auto relative">
    {#if assetsState.isLoading}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        <div class="w-10 h-10 border-3 border-default border-t-accent rounded-full animate-spin"></div>
        <p class="text-secondary">Loading assets...</p>
      </div>
    {:else if displayedAssets.length === 0}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        <svg class="w-16 h-16 text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
        </svg>
        <p class="text-primary font-medium">No {viewState.activeTab} found</p>
        <p class="text-sm text-secondary">Try adjusting your search or scan for assets</p>
      </div>
    {:else}
      {#if viewState.activeTab === 'images'}
        {#if viewState.layoutMode === 'grid'}
          <ImageGrid assets={displayedAssets} />
        {:else}
          <!-- Table view for images -->
          <AssetList assets={displayedAssets} isLoading={assetsState.isLoading} />
        {/if}
      {:else}
        <AudioList assets={displayedAssets} />
      {/if}
    {/if}
  </main>

  <!-- Lightbox Modal -->
  {#if viewState.lightboxAsset}
    <ImageLightbox
      asset={viewState.lightboxAsset}
      onClose={() => viewState.closeLightbox()}
      onNext={() => viewState.nextImage(displayedAssets)}
      onPrev={() => viewState.prevImage(displayedAssets)}
    />
  {/if}
</div>
