<script lang="ts">
  import { untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { assetsState } from '$lib/state/assets.svelte';
  import type { SearchColumn } from '$lib/database/queries';
  import { viewState } from '$lib/state/view.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import ViewModeToggle from './ViewModeToggle.svelte';
  import AssetTypeSwitch from './AssetTypeSwitch.svelte';
  import DurationFilter from './DurationFilter.svelte';
  import Spinner from './Spinner.svelte';
  import {
    SearchIcon,
    FolderIcon,
    CloseIcon,
    BrainIcon,
    GearIcon,
    SimilarIcon,
  } from '$lib/components/icons';

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
      await clapState.search(
        query,
        undefined,
        assetsState.durationFilter,
        assetsState.folderLocation,
      );
    } catch (error) {
      showToast(`Semantic search failed: ${error}`, 'error');
      // Fall back to FTS
      clapState.semanticSearchEnabled = false;
      assetsState.searchAssets(query, 'audio');
    }
  }

  // True once we know CLAP has never been set up (no uv, no cache)
  let clapNotConfigured = $derived(
    clapState.setupKnown && clapState.setupStatus === 'not-configured',
  );

  function toggleSemanticSearch() {
    if (clapNotConfigured) {
      goto('/settings');
      return;
    }
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

  // Folder scope chip: the deepest segment as the label, the whole location as the tooltip
  let folderChip = $derived.by(() => {
    const loc = assetsState.folderLocation;
    if (!loc) return null;
    const root = exploreState.roots.find(
      (r) => r.location.type === 'folder' && r.location.folderId === loc.folderId,
    );
    const parts = [root?.name || 'Folder'];
    if (loc.relPath) parts.push(...loc.relPath.split('/').filter(Boolean));
    if (loc.type === 'zip') {
      parts.push(loc.zipFile);
      if (loc.zipPrefix) parts.push(...loc.zipPrefix.split('/').filter(Boolean));
    }
    return { label: parts[parts.length - 1], title: parts.join(' / ') };
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
          .searchBySimilarity(
            clapState.similarToAssetId,
            clapState.similarToFilename,
            undefined,
            assetsState.durationFilter,
            assetsState.folderLocation,
          )
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

  // Optional restrictions; with neither pressed the search covers both ('anywhere')
  const searchColumnOptions: { value: SearchColumn; label: string; title: string }[] = [
    { value: 'filename', label: 'Name', title: 'Only match filenames' },
    { value: 'path', label: 'Path', title: 'Only match folder paths' },
  ];

  function clearSearchText() {
    searchInput = '';
    if (isSimilarityMode) {
      clapState.similarityFilterText = '';
    } else if (clapState.semanticSearchEnabled && isAudioTab) {
      clapState.clearSearch();
      assetsState.searchAssets('', 'audio');
    } else {
      assetsState.searchAssets('', viewState.activeTab === 'images' ? 'image' : 'audio');
    }
  }

  // Backspace in an empty field removes the nearest chip, like a tag input
  function handleSearchKeyDown(e: KeyboardEvent) {
    if (e.key !== 'Backspace' || searchInput) return;
    if (assetsState.folderLocation) {
      e.preventDefault();
      clearFolderFilter();
    } else if (isSimilarityMode) {
      e.preventDefault();
      cancelSimilaritySearch();
    }
  }

  function setSearchColumn(value: SearchColumn) {
    if (assetsState.searchColumn === value) return;
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
        : assetsState.searchColumn === 'filename'
          ? `Search ${viewState.activeTab} names…`
          : assetsState.searchColumn === 'path'
            ? `Search ${viewState.activeTab} paths…`
            : `Search ${viewState.activeTab}…`,
  );
</script>

<div class="@container">
  <div
    class="flex flex-wrap items-center gap-x-3 @3xl:gap-x-4 gap-y-2 px-4 py-3 bg-secondary border-b border-default"
  >
    <!-- Folder panel toggle -->
    <button
      class="h-9 flex-shrink-0 flex items-center gap-1.5 px-2.5 text-sm font-medium rounded-md transition-colors {viewState.folderSidebarOpen
        ? 'bg-accent-muted text-accent'
        : 'text-secondary hover:text-primary hover:bg-tertiary'}"
      onclick={() => viewState.toggleFolderSidebar()}
      title={viewState.folderSidebarOpen ? 'Collapse folder panel' : 'Expand folder panel'}
    >
      <FolderIcon size="sm" />
      <span class="hidden @5xl:inline">Folders</span>
    </button>

    <AssetTypeSwitch />

    <!-- Search: one field for everything that decides what matches — similarity source,
         folder scope, the query, and the name/path restriction. Never squeezed below a
         usable width: the toolbar wraps to a second row instead. -->
    <div
      class="flex-1 min-w-72 max-w-[520px] h-9 overflow-hidden flex items-center gap-1.5 pl-2 pr-1 bg-primary border rounded-md focus-within:ring-2 {isSemanticModeEnabled &&
      !isSimilarityMode
        ? 'border-purple-500 focus-within:ring-purple-500'
        : 'border-default focus-within:ring-accent/60'}"
    >
      {#if clapState.isSearching}
        <Spinner size="sm" />
      {:else}
        <SearchIcon size="sm" class="text-secondary flex-shrink-0" />
      {/if}

      {#if isSimilarityMode}
        <span
          class="h-6 max-w-36 flex-shrink-0 overflow-hidden flex items-center gap-1 pl-1.5 pr-0.5 text-xs font-medium rounded bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300"
          title="Similar to {clapState.similarToFilename}"
        >
          <SimilarIcon size="sm" class="w-3.5 h-3.5 flex-shrink-0" />
          <span class="min-w-0 truncate">{clapState.similarToFilename}</span>
          <button
            class="flex-shrink-0 p-0.5 rounded hover:bg-purple-200 dark:hover:bg-purple-800/60 transition-colors"
            onclick={cancelSimilaritySearch}
            aria-label="Clear similarity search"
          >
            <CloseIcon size="sm" class="w-3 h-3" />
          </button>
        </span>
      {/if}

      {#if folderChip}
        <span
          class="h-6 max-w-36 flex-shrink-0 overflow-hidden flex items-center gap-1 pl-1.5 pr-0.5 text-xs font-medium rounded bg-tertiary text-primary"
          title="In {folderChip.title}"
        >
          <FolderIcon size="sm" class="w-3.5 h-3.5 flex-shrink-0 text-secondary" />
          <span class="min-w-0 truncate">{folderChip.label}</span>
          <button
            class="flex-shrink-0 p-0.5 rounded text-secondary hover:text-primary hover:bg-elevated transition-colors"
            onclick={clearFolderFilter}
            aria-label="Clear folder filter"
          >
            <CloseIcon size="sm" class="w-3 h-3" />
          </button>
        </span>
      {/if}

      <input
        type="text"
        placeholder={placeholderText}
        value={searchInput}
        oninput={handleSearch}
        onkeydown={handleSearchKeyDown}
        class="flex-1 min-w-16 h-full bg-transparent text-primary placeholder:text-tertiary outline-none"
      />

      {#if searchInput}
        <button
          class="flex-shrink-0 p-0.5 rounded text-secondary hover:text-primary hover:bg-tertiary transition-colors"
          onclick={clearSearchText}
          title="Clear search"
        >
          <CloseIcon size="sm" />
        </button>
      {/if}

      <!-- Restrict matching to one column; neither pressed = both -->
      {#if !isSimilarityMode && !isSemanticModeEnabled}
        <div
          class="flex-shrink-0 flex items-center gap-0.5 pl-1 border-l border-default"
          role="group"
          aria-label="Only match"
        >
          {#each searchColumnOptions as opt (opt.value)}
            {@const active = assetsState.searchColumn === opt.value}
            <button
              class="h-6 px-1.5 text-xs font-medium rounded transition-colors {active
                ? 'bg-accent-light text-accent'
                : 'text-tertiary hover:text-primary hover:bg-tertiary'}"
              aria-pressed={active}
              title={active ? 'Search names and paths' : opt.title}
              onclick={() => setSearchColumn(active ? 'anywhere' : opt.value)}
            >
              {opt.label}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Audio-specific filters (semantic search + duration filter) -->
    {#if isAudioTab}
      <button
        onclick={toggleSemanticSearch}
        disabled={isSimilarityMode}
        class="h-9 flex-shrink-0 flex items-center gap-2 px-2.5 @3xl:px-3 text-sm font-medium rounded-md transition-colors"
        class:bg-purple-500={isSemanticModeEnabled && !isSimilarityMode}
        class:text-white={isSemanticModeEnabled && !isSimilarityMode}
        class:bg-secondary={!isSemanticModeEnabled || isSimilarityMode}
        class:text-secondary={!isSemanticModeEnabled || isSimilarityMode}
        class:hover:bg-purple-600={isSemanticModeEnabled && !isSimilarityMode}
        class:hover:bg-tertiary={!isSemanticModeEnabled && !isSimilarityMode && !clapNotConfigured}
        class:hover:bg-secondary={clapNotConfigured}
        class:opacity-50={isSimilarityMode || clapNotConfigured}
        class:cursor-not-allowed={isSimilarityMode}
        title={isSimilarityMode
          ? 'Exit similarity search first'
          : clapNotConfigured
            ? 'Semantic search requires one-time setup — click to go to Settings'
            : isSemanticModeEnabled
              ? 'Switch to text search'
              : 'Switch to semantic search'}
      >
        <!-- Brain/AI icon for semantic search -->
        <BrainIcon size="sm" />
        <span class="hidden @4xl:inline">Semantic</span>
        {#if clapNotConfigured}
          <GearIcon size="sm" class="w-3 h-3 opacity-70" />
        {/if}
      </button>

      <!-- Duration filter -->
      <DurationFilter />
    {/if}

    <!-- View mode toggle (images only) -->
    <ViewModeToggle />

    <!-- Stats -->
    <div class="ml-auto hidden @2xl:flex items-center gap-2 flex-shrink-0 whitespace-nowrap">
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
</div>
