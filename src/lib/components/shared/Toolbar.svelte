<script lang="ts">
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import ViewModeToggle from './ViewModeToggle.svelte';
  import DurationFilter from './DurationFilter.svelte';
  import Spinner from './Spinner.svelte';
  import { SearchIcon, FolderIcon, CloseIcon } from '$lib/components/icons';
  import { ZIP_SEP } from '$lib/database/queries';

  let searchInput = $state(assetsState.searchText);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Debounce delay in ms (shorter for FTS, longer for semantic)
  const FTS_DEBOUNCE_MS = 150;
  const SEMANTIC_DEBOUNCE_MS = 300;

  // Check if we're on the audio tab
  let isAudioTab = $derived(viewState.activeTab === 'audio');

  function handleSearch(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchInput = value;

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
      await clapState.search(query, undefined, assetsState.durationFilter);
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
    exploreState.selectedPath = null;
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.setFolderFilter(null, assetType);
  }

  // Folder display name: handles both filesystem and ZIP-internal paths
  let folderDisplayName = $derived(() => {
    if (!assetsState.folderPath) return '';
    const sepIdx = assetsState.folderPath.indexOf(ZIP_SEP);
    if (sepIdx !== -1) {
      const zipFile = assetsState.folderPath.substring(0, sepIdx);
      const prefix = assetsState.folderPath.substring(sepIdx + ZIP_SEP.length);
      const zipName = zipFile.split(/[\\/]/).pop() || zipFile;
      if (!prefix) return zipName;
      const prefixName = prefix.replace(/\/$/, '').split('/').pop();
      return `${zipName} / ${prefixName}`;
    }
    const segments = assetsState.folderPath.split(/[\\/]/);
    return segments[segments.length - 1] || assetsState.folderPath;
  });

  // Check if semantic mode is active
  let isSemanticModeEnabled = $derived(isAudioTab && clapState.semanticSearchEnabled);

  // Unified stats - what to show in the toolbar
  let activeResultCount = $derived(
    isSemanticModeEnabled ? clapState.semanticResults.length : assetsState.assets.length,
  );
  let hasActiveSearch = $derived(
    isSemanticModeEnabled ? !!clapState.lastSearchQuery?.trim() : !!assetsState.searchText?.trim(),
  );
  let hasMoreResults = $derived(
    isSemanticModeEnabled ? clapState.hasMoreResults : assetsState.hasMoreResults,
  );

  // Placeholder text based on search mode
  let placeholderText = $derived(
    isSemanticModeEnabled
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
        class:!border-purple-500={isSemanticModeEnabled}
        class:!ring-purple-500={isSemanticModeEnabled}
      />
      {#if searchInput}
        <button
          class="absolute right-2 top-1/2 -translate-y-1/2 p-0.5 rounded hover:bg-tertiary transition-colors"
          onclick={() => {
            searchInput = '';
            if (clapState.semanticSearchEnabled && isAudioTab) {
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

    <!-- Audio-specific filters (semantic search + duration filter) -->
    {#if isAudioTab}
      <button
        onclick={toggleSemanticSearch}
        class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-md transition-colors"
        class:bg-purple-500={isSemanticModeEnabled}
        class:text-white={isSemanticModeEnabled}
        class:bg-secondary={!isSemanticModeEnabled}
        class:text-secondary={!isSemanticModeEnabled}
        class:hover:bg-purple-600={isSemanticModeEnabled}
        class:hover:bg-tertiary={!isSemanticModeEnabled}
        title={isSemanticModeEnabled ? 'Switch to text search' : 'Switch to semantic search'}
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

  <!-- Folder breadcrumb (when a folder filter is active) -->
  {#if assetsState.folderPath}
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
