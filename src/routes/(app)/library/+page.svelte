<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { getDatabase } from '$lib/database/connection';
  import { getAssetTypeCounts } from '$lib/database/queries';
  import type { CategoryProgress } from '$lib/types';

  import TabBar from '$lib/components/shared/TabBar.svelte';
  import Toolbar from '$lib/components/shared/Toolbar.svelte';
  import FolderSidebar from '$lib/components/FolderSidebar.svelte';
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
    console.log('[Library] onMount started');
    console.time('[Library] refreshAssetCounts');
    await refreshAssetCounts();
    console.timeEnd('[Library] refreshAssetCounts');

    // Load assets for the current tab (persists across navigation)
    const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
    console.time('[Library] loadAssets');
    assetsState.loadAssets(currentType).then(() => console.timeEnd('[Library] loadAssets'));

    // Listen for scan completion to refresh counts
    const unlistenScan = await listen('scan-complete', async () => {
      console.log('[Library] Scan complete, refreshing asset counts');
      await refreshAssetCounts();
      // Refresh folder tree if sidebar is open
      if (viewState.folderSidebarOpen) {
        exploreState.clearCache();
        exploreState.loadRoots();
      }
    });

    // Listen for category-specific processing completion to refresh counts
    const handleProcessingComplete = async () => {
      console.log('[Library] Processing complete, refreshing asset counts and reloading assets');
      await refreshAssetCounts();
      // Reload current tab's assets
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
    };

    const unlistenImageComplete = await listen<CategoryProgress>(
      'processing-complete-image',
      handleProcessingComplete,
    );
    const unlistenAudioComplete = await listen<CategoryProgress>(
      'processing-complete-audio',
      handleProcessingComplete,
    );

    unlistenFns.push(unlistenScan, unlistenImageComplete, unlistenAudioComplete);
  });

  onDestroy(() => {
    unlistenFns.forEach((fn) => fn());
  });

  // Check if we're in semantic search mode on audio tab
  let isSemanticModeEnabled = $derived(
    viewState.activeTab === 'audio' && clapState.semanticSearchEnabled,
  );

  // Semantic search results with Asset compatibility (add width/height as null)
  let semanticAssets = $derived(
    clapState.semanticResults.map((result) => ({
      ...result,
      width: null,
      height: null,
    })),
  );

  // Unified "active assets" - what should be displayed based on current mode
  let activeAssets = $derived(isSemanticModeEnabled ? semanticAssets : assetsState.assets);

  // Unified "has search" - whether there's an active search query
  let hasActiveSearch = $derived(
    isSemanticModeEnabled ? !!clapState.lastSearchQuery?.trim() : !!assetsState.searchText?.trim(),
  );

  // Whether any filter is active (search or folder)
  let hasAnyFilter = $derived(hasActiveSearch || !!assetsState.folderPath);

  // Unified "is loading" - whether a search is in progress
  let isLoading = $derived(isSemanticModeEnabled ? clapState.isSearching : assetsState.isLoading);

  // Unified "has more results" - whether results were truncated
  let hasMoreResults = $derived(
    isSemanticModeEnabled ? clapState.hasMoreResults : assetsState.hasMoreResults,
  );
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Tab Navigation (asset type + folder toggle) -->
  <TabBar imageCount={assetCounts.images} audioCount={assetCounts.audio} />

  <!-- Toolbar (search, filters, view controls) -->
  <Toolbar />

  <!-- Main Content Area -->
  <main class="flex-1 overflow-hidden relative flex">
    <!-- Folder Sidebar (collapsible) -->
    {#if viewState.folderSidebarOpen}
      <FolderSidebar />
    {/if}

    <!-- Content Panel -->
    <div class="flex-1 overflow-hidden">
      {#if isLoading}
        <div class="flex flex-col items-center justify-center h-full gap-4">
          <Spinner size="lg" />
          <p class="text-secondary">
            {isSemanticModeEnabled ? 'Searching...' : 'Loading assets...'}
          </p>
        </div>
      {:else if !hasAnyFilter && activeAssets.length === 0}
        <!-- Empty state: No search query and no folder selected -->
        <div class="flex flex-col items-center justify-center h-full gap-4">
          <svg
            class="w-16 h-16 text-tertiary"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.5"
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
          <p class="text-primary font-medium">Search your {viewState.activeTab}</p>
          <p class="text-sm text-secondary">
            {#if assetsState.totalMatchingCount > 0}
              You have {assetsState.totalMatchingCount.toLocaleString()}
              {viewState.activeTab} - type to search or open the folder panel
            {:else}
              No {viewState.activeTab} found - try scanning for assets first
            {/if}
          </p>
        </div>
      {:else if activeAssets.length === 0}
        <!-- Empty state: Filter active but no results -->
        <div class="flex flex-col items-center justify-center h-full gap-4">
          <svg
            class="w-16 h-16 text-tertiary"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.5"
              d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"
            />
          </svg>
          <p class="text-primary font-medium">No matching {viewState.activeTab}</p>
          <p class="text-sm text-secondary">
            {#if assetsState.folderPath && hasActiveSearch}
              Try adjusting your search or selecting a different folder
            {:else if assetsState.folderPath}
              No {viewState.activeTab} in this folder
            {:else}
              Try adjusting your search query
            {/if}
          </p>
        </div>
      {:else if viewState.activeTab === 'images'}
        {#if viewState.layoutMode === 'grid'}
          <ImageGrid assets={activeAssets} />
        {:else}
          <!-- Table view for images -->
          <AssetList assets={activeAssets} isLoading={assetsState.isLoading} />
        {/if}
      {:else}
        <AudioList assets={activeAssets} showSimilarity={isSemanticModeEnabled} />
      {/if}
    </div>
  </main>

  <!-- Lightbox Modal -->
  {#if viewState.lightboxAsset}
    <ImageLightbox
      asset={viewState.lightboxAsset}
      onClose={() => viewState.closeLightbox()}
      onNext={() => viewState.nextImage(activeAssets)}
      onPrev={() => viewState.prevImage(activeAssets)}
    />
  {/if}
</div>
