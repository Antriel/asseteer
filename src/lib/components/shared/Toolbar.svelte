<script lang="ts">
  import { untrack } from 'svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import type { SearchColumn } from '$lib/database/queries';
  import { viewState } from '$lib/state/view.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import ViewModeToggle from './ViewModeToggle.svelte';
  import DurationFilter from './DurationFilter.svelte';
  import Spinner from './Spinner.svelte';
  import { SearchIcon, FolderIcon, CloseIcon } from '$lib/components/icons';

  let searchInput = $state(assetsState.searchText);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Saved search text from before entering similarity mode, for restoring on cancel
  let preSimilarityState: { searchText: string } | null = null;

  // Debounce delay in ms (shorter for FTS, longer for semantic)
  const FTS_DEBOUNCE_MS = 150;
  const SEMANTIC_DEBOUNCE_MS = 300;

  // Check if we're on the audio tab
  let isAudioTab = $derived(viewState.activeTab === 'audio');

  function handleSearch(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchInput = value;

    // In similarity mode, typing filters the similarity results client-side
    if (isSimilarityMode) {
      clapState.similarityFilterText = value;
      return;
    }

    // Clear any pending debounced search
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }

    const isSemanticMode = isAudioTab && clapState.semanticSearchEnabled;
    const debounceMs = isSemanticMode ? SEMANTIC_DEBOUNCE_MS : FTS_DEBOUNCE_MS;

    // Debounce the actual search
    debounceTimer = setTimeout(() => {
      if (isSemanticMode) {
        handleSemanticSearch(value);
      } else {
        assetsState.searchAssets(value, viewState.activeTab === 'images' ? 'image' : 'audio');
      }
    }, debounceMs);
  }

  async function handleSemanticSearch(query: string) {
    if (!query.trim()) {
      clapState.clearSearch();
      // Fall back to showing all audio
      assetsState.searchAssets('', 'audio');
      return;
    }

    try {
      // Pass duration filter to semantic search for pre-filtering before similarity computation
      await clapState.search(query, undefined, assetsState.durationFilter, assetsState.folderLocation);
    } catch (error) {
      showToast(`Semantic search failed: ${error}`, 'error');
      // Fall back to FTS
      clapState.semanticSearchEnabled = false;
      assetsState.searchAssets(query, 'audio');
    }
  }

  function toggleSemanticSearch() {
    clapState.toggleSemanticSearch();

    // Clear any pending debounced search
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }

    if (clapState.semanticSearchEnabled) {
      // Re-run search with semantic mode
      if (searchInput.trim()) {
        handleSemanticSearch(searchInput);
      }
    } else {
      // Switch back to FTS
      assetsState.searchAssets(searchInput, 'audio');
    }
  }

  function clearFolderFilter() {
    exploreState.selectedKey = null;
    exploreState.selectedLocation = null;
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.setFolderFilter(null, assetType);
  }

  // Folder display name from the selected explore node
  let folderDisplayName = $derived(() => {
    const loc = assetsState.folderLocation;
    if (!loc) return '';
    if (loc.type === 'zip') {
      const zipName = loc.zipFile;
      if (!loc.zipPrefix) return zipName;
      const prefixName = loc.zipPrefix.replace(/\/$/, '').split('/').pop();
      return `${zipName} / ${prefixName}`;
    }
    // Filesystem folder: show last segment of relPath, or the root name from the tree
    if (loc.relPath) {
      return loc.relPath.split('/').pop() || loc.relPath;
    }
    // Root folder — find name from explore roots
    const root = exploreState.roots.find((r) => r.location.type === 'folder' && r.location.folderId === loc.folderId);
    return root?.name || 'Folder';
  });

  // Check if semantic mode is active
  let isSemanticModeEnabled = $derived(isAudioTab && clapState.semanticSearchEnabled);

  // Check if similarity search is active
  let isSimilarityMode = $derived(isAudioTab && clapState.similarToAssetId !== null);

  // Re-run semantic/similarity search when the folder filter changes.
  // FTS is already handled by setFolderFilter → loadAssets().
  $effect(() => {
    assetsState.folderLocation; // reactive dependency
    untrack(() => {
      if (isSimilarityMode && clapState.similarToAssetId !== null && clapState.similarToFilename) {
        clapState
          .searchBySimilarity(clapState.similarToAssetId, clapState.similarToFilename, undefined, assetsState.durationFilter, assetsState.folderLocation)
          .catch((e) => showToast(`${e}`, 'error'));
      } else if (isSemanticModeEnabled && clapState.lastSearchQuery.trim()) {
        handleSemanticSearch(clapState.lastSearchQuery);
      }
    });
  });

  // Save search text and clear input when entering similarity mode
  $effect(() => {
    if (isSimilarityMode) {
      if (!preSimilarityState) {
        preSimilarityState = { searchText: searchInput };
      }
      searchInput = '';
    }
  });

  function cancelSimilaritySearch() {
    const saved = preSimilarityState;
    const wasSemanticEnabled = clapState.preSimilaritySemanticEnabled;
    preSimilarityState = null;

    if (searchInput) {
      // User typed something new — keep it, run FTS search
      clapState.clearSimilaritySearch();
      assetsState.searchAssets(searchInput, 'audio');
    } else if (saved) {
      // Input still empty — restore previous state
      clapState.clearSimilaritySearch();
      searchInput = saved.searchText;
      if (wasSemanticEnabled && saved.searchText.trim()) {
        clapState.semanticSearchEnabled = true;
        handleSemanticSearch(saved.searchText);
      } else if (saved.searchText.trim()) {
        assetsState.searchAssets(saved.searchText, 'audio');
      } else {
        assetsState.searchAssets('', 'audio');
      }
    } else {
      clapState.clearSimilaritySearch();
      assetsState.searchAssets('', 'audio');
    }
  }

  // Unified stats - what to show in the toolbar
  let activeResultCount = $derived(
    isSemanticModeEnabled ? clapState.semanticResults.length : assetsState.assets.length,
  );
  let hasActiveSearch = $derived(
    isSimilarityMode
      ? true
      : isSemanticModeEnabled
        ? !!clapState.lastSearchQuery?.trim()
        : !!assetsState.searchText?.trim(),
  );
  let hasMoreResults = $derived(
    isSemanticModeEnabled ? clapState.hasMoreResults : assetsState.hasMoreResults,
  );

  const searchColumnOptions: { value: SearchColumn; label: string }[] = [
    { value: 'anywhere', label: 'Anywhere' },
    { value: 'filename', label: 'Filename' },
    { value: 'path', label: 'Path' },
  ];

  function handleSearchColumnChange(e: Event) {
    const value = (e.target as HTMLSelectElement).value as SearchColumn;
    assetsState.searchColumn = value;
    // Re-run search if there's active text
    if (searchInput.trim() && !isSimilarityMode) {
      const isSemanticMode = isAudioTab && clapState.semanticSearchEnabled;
      if (!isSemanticMode) {
        assetsState.searchAssets(searchInput, viewState.activeTab === 'images' ? 'image' : 'audio');
      }
    }
  }

  // Placeholder text based on search mode
  let placeholderText = $derived(
    isSimilarityMode
      ? 'Filter results by filename...'
      : isSemanticModeEnabled
        ? 'Semantic search (e.g., "footsteps on wood")...'
        : `Search ${viewState.activeTab}...`,
  );
</script>

<div class="flex flex-col">
  <div class="flex items-center gap-4 px-4 py-3 bg-secondary border-b border-default">
    <!-- Search -->
    <div class="relative flex-1 max-w-[400px]">
      {#if clapState.isSearching}
        <div class="absolute left-2 top-1/2 -translate-y-1/2">
          <Spinner size="sm" />
        </div>
      {:else}
        <SearchIcon
          size="sm"
          class="absolute left-2 top-1/2 -translate-y-1/2 text-secondary pointer-events-none"
        />
      {/if}
      <input
        type="text"
        placeholder={placeholderText}
        value={searchInput}
        oninput={handleSearch}
        class="w-full py-2 px-2 pl-8 {searchInput ? 'pr-8' : 'pr-2'} border border-default rounded-md bg-primary text-primary placeholder:text-secondary focus:outline-none focus:ring-2 focus:ring-accent"
        class:!border-purple-500={isSemanticModeEnabled && !isSimilarityMode}
        class:!ring-purple-500={isSemanticModeEnabled && !isSimilarityMode}
      />
      {#if searchInput}
        <button
          class="absolute right-2 top-1/2 -translate-y-1/2 p-0.5 rounded hover:bg-tertiary transition-colors"
          onclick={() => {
            searchInput = '';
            if (isSimilarityMode) {
              clapState.similarityFilterText = '';
            } else if (clapState.semanticSearchEnabled && isAudioTab) {
              clapState.clearSearch();
              assetsState.searchAssets('', 'audio');
            } else {
              assetsState.searchAssets('', viewState.activeTab === 'images' ? 'image' : 'audio');
            }
          }}
          title="Clear search"
        >
          <CloseIcon size="sm" class="text-secondary hover:text-primary" />
        </button>
      {/if}
    </div>

    <!-- Search column targeting -->
    {#if !isSimilarityMode && !isSemanticModeEnabled}
      <select
        value={assetsState.searchColumn}
        onchange={handleSearchColumnChange}
        class="py-2 px-2 text-sm border border-default rounded-md bg-primary text-primary focus:outline-none focus:ring-2 focus:ring-accent"
      >
        {#each searchColumnOptions as opt}
          <option value={opt.value}>{opt.label}</option>
        {/each}
      </select>
    {/if}

    <!-- Audio-specific filters (semantic search + duration filter) -->
    {#if isAudioTab}
      <button
        onclick={toggleSemanticSearch}
        disabled={isSimilarityMode}
        class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-md transition-colors"
        class:bg-purple-500={isSemanticModeEnabled && !isSimilarityMode}
        class:text-white={isSemanticModeEnabled && !isSimilarityMode}
        class:bg-secondary={!isSemanticModeEnabled || isSimilarityMode}
        class:text-secondary={!isSemanticModeEnabled || isSimilarityMode}
        class:hover:bg-purple-600={isSemanticModeEnabled && !isSimilarityMode}
        class:hover:bg-tertiary={!isSemanticModeEnabled && !isSimilarityMode}
        class:opacity-50={isSimilarityMode}
        class:cursor-not-allowed={isSimilarityMode}
        title={isSimilarityMode ? 'Exit similarity search first' : isSemanticModeEnabled ? 'Switch to text search' : 'Switch to semantic search'}
      >
        <!-- Brain/AI icon for semantic search -->
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z"
          />
        </svg>
        <span>Semantic</span>
      </button>

      <!-- Duration filter -->
      <DurationFilter />
    {/if}

    <!-- View mode toggle (images only) -->
    <ViewModeToggle />

    <!-- Stats -->
    <div class="ml-auto flex items-center gap-2">
      {#if activeResultCount > 0}
        <span
          class="text-sm"
          class:text-purple-600={isSemanticModeEnabled}
          class:dark:text-purple-400={isSemanticModeEnabled}
          class:text-secondary={!isSemanticModeEnabled}
        >
          {activeResultCount.toLocaleString()}
          {isSemanticModeEnabled ? 'matches' : viewState.activeTab}
        </span>
        {#if hasMoreResults}
          <span
            class="text-xs text-warning"
            title="Results are limited for performance. Refine your search to see more specific results."
          >
            (limit reached)
          </span>
        {/if}
      {:else if hasActiveSearch}
        <span class="text-sm text-secondary"> No results </span>
      {:else}
        <span class="text-sm text-tertiary"> Search to browse </span>
      {/if}
    </div>
  </div>

  <!-- Similarity search banner -->
  {#if isSimilarityMode}
    <div class="flex items-center gap-2 px-4 py-1.5 bg-purple-50 dark:bg-purple-900/20 border-b border-purple-200 dark:border-purple-800">
      <svg class="w-4 h-4 text-purple-500 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
      </svg>
      <span class="text-sm text-purple-700 dark:text-purple-300">
        Similar to: <strong class="font-semibold">"{clapState.similarToFilename}"</strong>
      </span>
      <button
        onclick={cancelSimilaritySearch}
        class="flex-shrink-0 p-0.5 rounded hover:bg-purple-100 dark:hover:bg-purple-800/40 transition-colors"
        title="Clear similarity search"
      >
        <CloseIcon size="sm" class="text-purple-500 hover:text-purple-700 dark:hover:text-purple-300" />
      </button>
    </div>
  {/if}

  <!-- Folder breadcrumb (when a folder filter is active) -->
  {#if assetsState.folderLocation}
    <div class="flex items-center gap-2 px-4 py-1.5 bg-tertiary border-b border-default">
      <FolderIcon size="sm" class="text-secondary flex-shrink-0" />
      <span class="text-sm text-primary truncate">{folderDisplayName()}</span>
      <button
        onclick={clearFolderFilter}
        class="flex-shrink-0 p-0.5 rounded hover:bg-secondary transition-colors"
        title="Clear folder filter"
      >
        <CloseIcon size="sm" class="text-secondary hover:text-primary" />
      </button>
    </div>
  {/if}
</div>
