<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { getDatabase } from '$lib/database/connection';
  import { getAssetTypeCounts } from '$lib/database/queries';
  import type { CategoryProgress, Asset } from '$lib/types';

  import TabBar from '$lib/components/shared/TabBar.svelte';
  import Toolbar from '$lib/components/shared/Toolbar.svelte';
  import ImageGrid from '$lib/components/ImageGrid.svelte';
  import AudioList from '$lib/components/AudioList.svelte';
  import AssetList from '$lib/components/AssetList.svelte';
  import ImageLightbox from '$lib/components/modals/ImageLightbox.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';

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
      console.log('[Library] Scan complete, refreshing asset counts');
      await refreshAssetCounts();
    });

    // Listen for category-specific processing completion to refresh counts
    const handleProcessingComplete = async () => {
      console.log('[Library] Processing complete, refreshing asset counts and reloading assets');
      await refreshAssetCounts();
      // Reload current tab's assets
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
    };

    const unlistenImageComplete = await listen<CategoryProgress>('processing-complete-image', handleProcessingComplete);
    const unlistenAudioComplete = await listen<CategoryProgress>('processing-complete-audio', handleProcessingComplete);

    unlistenFns.push(unlistenScan, unlistenImageComplete, unlistenAudioComplete);
  });

  onDestroy(() => {
    unlistenFns.forEach((fn) => fn());
  });

  // Assets are already filtered by loadAssets() based on the current tab
  let displayedAssets = $derived(assetsState.assets);

  // Check if we're in semantic search mode with results
  let isSemanticMode = $derived(
    viewState.activeTab === 'audio' &&
    clapState.semanticSearchEnabled &&
    clapState.semanticResults.length > 0
  );

  // Create a map of asset_id to similarity for the current semantic results
  let similarityMap = $derived.by(() => {
    const map = new Map<number, number>();
    for (const result of clapState.semanticResults) {
      map.set(result.asset_id, result.similarity);
    }
    return map;
  });

  // Get semantic search assets with similarity scores
  let semanticAssets = $derived.by(() => {
    if (!isSemanticMode) return [];

    // Map semantic results to full assets with similarity
    return clapState.semanticResults
      .map(result => {
        const asset = displayedAssets.find(a => a.id === result.asset_id);
        if (asset) {
          return { ...asset, similarity: result.similarity };
        }
        // Asset not in current list, create minimal version
        return {
          id: result.asset_id,
          filename: result.filename,
          path: result.path,
          asset_type: 'audio' as const,
          format: result.filename.split('.').pop() || 'audio',
          file_size: 0,
          similarity: result.similarity
        } as Asset & { similarity: number };
      });
  });
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Tab Navigation -->
  <TabBar imageCount={assetCounts.images} audioCount={assetCounts.audio} />

  <!-- Toolbar (search, filters, view controls) -->
  <Toolbar />

  <!-- Main Content Area -->
  <main class="flex-1 overflow-hidden relative">
    {#if assetsState.isLoading}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        <Spinner size="lg" />
        <p class="text-secondary">Loading assets...</p>
      </div>
    {:else if displayedAssets.length === 0}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        <svg class="w-16 h-16 text-tertiary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
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
        {#if isSemanticMode}
          <AudioList assets={semanticAssets} showSimilarity={true} />
        {:else}
          <AudioList assets={displayedAssets} />
        {/if}
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
