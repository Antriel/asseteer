<script lang="ts">
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import ViewModeToggle from './ViewModeToggle.svelte';
  import Spinner from './Spinner.svelte';
  import { SearchIcon } from '$lib/components/icons';

  let searchInput = $state('');

  // Check if we're on the audio tab
  let isAudioTab = $derived(viewState.activeTab === 'audio');

  function handleSearch(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchInput = value;

    if (isAudioTab && clapState.semanticSearchEnabled) {
      // Semantic search for audio
      handleSemanticSearch(value);
    } else {
      // Regular FTS search
      assetsState.searchAssets(value, viewState.activeTab === 'images' ? 'image' : 'audio');
    }
  }

  async function handleSemanticSearch(query: string) {
    if (!query.trim()) {
      clapState.clearSearch();
      // Fall back to showing all audio
      assetsState.searchAssets('', 'audio');
      return;
    }

    try {
      await clapState.search(query);
    } catch (error) {
      showToast(`Semantic search failed: ${error}`, 'error');
      // Fall back to FTS
      clapState.semanticSearchEnabled = false;
      assetsState.searchAssets(query, 'audio');
    }
  }

  function toggleSemanticSearch() {
    clapState.toggleSemanticSearch();

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

  // Placeholder text based on search mode
  let placeholderText = $derived.by(() => {
    if (isAudioTab && clapState.semanticSearchEnabled) {
      return 'Semantic search (e.g., "footsteps on wood")...';
    }
    return `Search ${viewState.activeTab}...`;
  });
</script>

<div class="flex items-center gap-4 px-4 py-3 bg-secondary border-b border-default">
  <!-- Search -->
  <div class="relative flex-1 max-w-[400px]">
    {#if clapState.isSearching}
      <div class="absolute left-2 top-1/2 -translate-y-1/2">
        <Spinner size="sm" />
      </div>
    {:else}
      <SearchIcon size="sm" class="absolute left-2 top-1/2 -translate-y-1/2 text-secondary pointer-events-none" />
    {/if}
    <input
      type="text"
      placeholder={placeholderText}
      value={searchInput}
      oninput={handleSearch}
      class="w-full py-2 px-2 pl-8 border border-default rounded-md bg-primary text-primary placeholder:text-secondary focus:outline-none focus:ring-2 focus:ring-accent"
      class:!border-purple-500={isAudioTab && clapState.semanticSearchEnabled}
      class:!ring-purple-500={isAudioTab && clapState.semanticSearchEnabled}
    />
  </div>

  <!-- Semantic search toggle (audio tab only) -->
  {#if isAudioTab}
    <button
      onclick={toggleSemanticSearch}
      class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-md transition-colors"
      class:bg-purple-500={clapState.semanticSearchEnabled}
      class:text-white={clapState.semanticSearchEnabled}
      class:bg-secondary={!clapState.semanticSearchEnabled}
      class:text-secondary={!clapState.semanticSearchEnabled}
      class:hover:bg-purple-600={clapState.semanticSearchEnabled}
      class:hover:bg-tertiary={!clapState.semanticSearchEnabled}
      title={clapState.semanticSearchEnabled ? 'Switch to text search' : 'Switch to semantic search'}
    >
      <!-- Brain/AI icon for semantic search -->
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
      </svg>
      <span>Semantic</span>
    </button>
  {/if}

  <!-- View mode toggle (images only) -->
  <ViewModeToggle />

  <!-- Stats -->
  <div class="ml-auto">
    {#if isAudioTab && clapState.semanticSearchEnabled && clapState.semanticResults.length > 0}
      <span class="text-sm text-purple-600 dark:text-purple-400">
        {clapState.semanticResults.length} matches
      </span>
    {:else}
      <span class="text-sm text-secondary">
        {assetsState.assets.length} {viewState.activeTab}
      </span>
    {/if}
  </div>
</div>
