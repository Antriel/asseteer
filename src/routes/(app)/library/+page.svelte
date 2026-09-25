<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { getDatabase } from '$lib/database/connection';
  import { getAssetTypeCounts, getPendingClapCount } from '$lib/database/queries';
  import { parseSearchQuery, splitAlternatives } from '$lib/database/searchQuery';
  import type { CategoryProgress } from '$lib/types';

  import Toolbar from '$lib/components/shared/Toolbar.svelte';
  // FolderSidebar is now rendered in the root layout
  import ImageGrid from '$lib/components/ImageGrid.svelte';
  import AudioList from '$lib/components/AudioList.svelte';
  import AssetList from '$lib/components/AssetList.svelte';
  import ImageLightbox from '$lib/components/modals/ImageLightbox.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import { SearchIcon, InboxIcon } from '$lib/components/icons';

  let pendingClapCount = $state(0);
  let unlistenFns: UnlistenFn[] = [];

  async function refreshAssetCounts() {
    const db = await getDatabase();
    const counts = await getAssetTypeCounts(db);
    viewState.setAssetCounts(counts);
  }

  async function refreshPendingClapCount() {
    pendingClapCount = await getPendingClapCount();
  }

  onMount(async () => {
    await Promise.all([refreshAssetCounts(), refreshPendingClapCount()]);

    // Load assets for the current tab (persists across navigation)
    const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.loadAssets(currentType);

    // Listen for scan completion to refresh counts
    const unlistenScan = await listen('scan-complete', async () => {
      await refreshAssetCounts();
      // Refresh folder tree if sidebar is open
      if (viewState.folderSidebarOpen) {
        exploreState.clearCache();
        exploreState.loadRoots();
      }
    });

    // Listen for category-specific processing completion to refresh counts
    const handleProcessingComplete = async () => {
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
    const unlistenClapComplete = await listen<CategoryProgress>(
      'processing-complete-clap',
      refreshPendingClapCount,
    );

    unlistenFns.push(
      unlistenScan,
      unlistenImageComplete,
      unlistenAudioComplete,
      unlistenClapComplete,
    );
  });

  onDestroy(() => {
    unlistenFns.forEach((fn) => fn());
  });

  // Check if we're in semantic search mode on audio tab
  let isSemanticModeEnabled = $derived(
    viewState.activeTab === 'audio' && clapState.semanticSearchEnabled,
  );

  // Apply client-side filename filter when in similarity mode
  let semanticAssets = $derived.by(() => {
    let results = clapState.semanticResults;
    const filter = clapState.similarityFilterText.trim().toLowerCase();
    if (filter) {
      results = results.filter((r) => r.filename.toLowerCase().includes(filter));
    }
    return results;
  });

  // Unified "active assets" - what should be displayed based on current mode
  let activeAssets = $derived(isSemanticModeEnabled ? semanticAssets : assetsState.assets);

  // Unified "has search" - whether there's an active search query
  let hasActiveSearch = $derived(
    isSemanticModeEnabled ? !!clapState.lastSearchQuery?.trim() : !!assetsState.searchText?.trim(),
  );

  // Whether any filter is active (search, folder, or similarity)
  let hasAnyFilter = $derived(
    hasActiveSearch || !!assetsState.folderLocation || !!clapState.similarToAssetId,
  );

  // Unified "is loading" - whether a search is in progress
  let isLoading = $derived(isSemanticModeEnabled ? clapState.isSearching : assetsState.isLoading);

  // Unified "has more results" - whether results were truncated
  let hasMoreResults = $derived(
    isSemanticModeEnabled ? clapState.hasMoreResults : assetsState.hasMoreResults,
  );

  // All words typed, none found together: offer the same words as alternatives
  let orSuggestion = $derived(isSemanticModeEnabled ? null : assetsState.orSuggestion);
</script>

<!-- A query rendered the way the search field shows it: chips with "or" between -->
{#snippet queryChips(text: string, semantic = false)}
  {#each splitAlternatives(text) as alt, i (i)}
    {#if i > 0}<span class="text-xs text-tertiary">or</span>{/if}
    <span
      class="h-6 px-1.5 flex items-center text-xs font-medium rounded {semantic
        ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300'
        : 'bg-tertiary text-primary'}">{alt.trim()}</span
    >
  {/each}
{/snippet}

{#snippet orSuggestionHint(suggestion: { text: string; count: number })}
  {@const words = parseSearchQuery(assetsState.searchText)[0] ?? []}
  <p class="text-primary font-medium">No {viewState.activeTab} match all {words.length} words</p>
  <p class="text-sm text-secondary text-center">
    Words separated by spaces must all match. Separate them with commas to find any of them:
  </p>
  <button
    class="flex items-center gap-1.5 pl-2 pr-3 h-9 rounded-md border border-default bg-primary hover:border-accent transition-colors"
    onclick={() =>
      assetsState.searchAssets(
        suggestion.text,
        viewState.activeTab === 'images' ? 'image' : 'audio',
      )}
  >
    <SearchIcon size="sm" class="text-secondary" />
    {@render queryChips(suggestion.text)}
    <span class="ml-2 text-sm text-accent whitespace-nowrap">
      {suggestion.count.toLocaleString()}
      {suggestion.count === 1 ? viewState.activeTab.replace(/s$/, '') : viewState.activeTab} →
    </span>
  </button>
{/snippet}

<div class="flex flex-col h-full overflow-hidden">
  <!-- Toolbar (asset type, search, filters, view controls) -->
  <Toolbar />

  <!-- Main Content Area -->
  <main class="flex-1 overflow-hidden relative flex">
    <!-- Content Panel -->
    <div class="flex-1 overflow-hidden relative">
      {#if isLoading && activeAssets.length > 0}
        <!-- Loading overlay when we have previous results to keep visible -->
        <div
          class="absolute top-2 left-1/2 -translate-x-1/2 z-10 flex items-center gap-2 px-3 py-1.5 bg-elevated rounded-full shadow-md border border-default"
        >
          <Spinner size="sm" />
          <span class="text-xs text-secondary">Loading...</span>
        </div>
      {/if}
      {#if isLoading && activeAssets.length === 0}
        <div class="flex flex-col items-center justify-center h-full gap-4">
          <Spinner size="lg" />
          <p class="text-secondary">
            {isSemanticModeEnabled ? 'Searching...' : 'Loading assets...'}
          </p>
        </div>
      {:else if !hasAnyFilter && activeAssets.length === 0}
        <!-- Empty state: No search query and no folder selected -->
        <div class="flex flex-col items-center justify-center h-full gap-4">
          <SearchIcon size="xl" class="text-tertiary" />
          <p class="text-primary font-medium">Search your {viewState.activeTab}</p>
          <p class="text-sm text-secondary">
            {#if assetsState.totalMatchingCount > 0}
              You have {assetsState.totalMatchingCount.toLocaleString()}
              {viewState.activeTab} - type to search or open the folder panel
            {:else}
              No {viewState.activeTab} found - try scanning for assets first
            {/if}
          </p>
          {#if assetsState.totalMatchingCount > 0}
            <dl
              class="mt-2 grid grid-cols-[auto_auto] items-center gap-x-4 gap-y-2 text-xs text-secondary"
            >
              {#if isSemanticModeEnabled}
                <dt class="justify-self-end">
                  <kbd class="px-1.5 py-0.5 rounded bg-tertiary text-primary font-sans"
                    >footsteps on wood</kbd
                  >
                </dt>
                <dd>describe the sound</dd>
                <dt class="justify-self-end flex items-center gap-1.5">
                  {@render queryChips('footsteps, door creak', true)}
                </dt>
                <dd>any of these sounds — separate with commas</dd>
              {:else}
                <dt class="justify-self-end">
                  <kbd class="px-1.5 py-0.5 rounded bg-tertiary text-primary font-sans">gun shot</kbd>
                </dt>
                <dd>all of the words</dd>
                <dt class="justify-self-end flex items-center gap-1.5">
                  {@render queryChips('gun, laser')}
                </dt>
                <dd>any of them — separate with commas</dd>
              {/if}
            </dl>
          {/if}
        </div>
      {:else if activeAssets.length === 0}
        <!-- Empty state: Filter active but no results -->
        <div class="flex flex-col items-center justify-center h-full gap-4">
          <InboxIcon size="xl" class="text-tertiary" />
          {#if orSuggestion}
            {@render orSuggestionHint(orSuggestion)}
          {:else if assetsState.folderLocation && hasActiveSearch}
            <p class="text-primary font-medium">No results found</p>
            <p class="text-sm text-secondary text-center">
              No {viewState.activeTab} matching your search in this folder
            </p>
            <div class="flex gap-2">
              <button
                class="px-3 py-1.5 text-sm rounded-md bg-tertiary text-primary hover:bg-elevated transition-colors"
                onclick={() => {
                  assetsState.searchAssets(
                    '',
                    viewState.activeTab === 'images' ? 'image' : 'audio',
                  );
                }}
              >
                Clear search
              </button>
              <button
                class="px-3 py-1.5 text-sm rounded-md bg-tertiary text-primary hover:bg-elevated transition-colors"
                onclick={() => {
                  exploreState.selectedKey = null;
                  exploreState.selectedLocation = null;
                  assetsState.setFolderFilter(
                    null,
                    viewState.activeTab === 'images' ? 'image' : 'audio',
                  );
                }}
              >
                Clear folder
              </button>
            </div>
          {:else if assetsState.folderLocation}
            <p class="text-primary font-medium">No {viewState.activeTab} in this folder</p>
            <p class="text-sm text-secondary">
              This folder doesn't contain any {viewState.activeTab}
            </p>
          {:else if isSemanticModeEnabled && pendingClapCount === viewState.assetCounts.audio}
            <p class="text-primary font-medium">No embeddings generated yet</p>
            <p class="text-sm text-secondary text-center">
              None of your {viewState.assetCounts.audio.toLocaleString()} audio files have been processed
              for semantic search. Head to the
              <a href="/processing" class="text-accent hover:underline">Processing tab</a> to generate
              embeddings.
            </p>
          {:else if isSemanticModeEnabled && pendingClapCount > 0}
            <p class="text-primary font-medium">No matching audio</p>
            <p class="text-sm text-secondary text-center">
              {(viewState.assetCounts.audio - pendingClapCount).toLocaleString()} of {viewState.assetCounts.audio.toLocaleString()}
              audio files have embeddings. Try adjusting your query, or process more in the
              <a href="/processing" class="text-accent hover:underline">Processing tab</a>.
            </p>
          {:else}
            <p class="text-primary font-medium">No matching {viewState.activeTab}</p>
            <p class="text-sm text-secondary">Try adjusting your search query</p>
          {/if}
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
