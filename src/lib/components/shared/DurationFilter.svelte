<script lang="ts">
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { clapState } from '$lib/state/clap.svelte';

  let isOpen = $state(false);
  let minInput = $state('');
  let maxInput = $state('');

  // Quick presets for common duration ranges
  const presets = [
    { label: '< 100ms', min: null, max: 100, description: 'Very short sounds' },
    { label: '< 1s', min: null, max: 1000, description: 'Short sounds' },
    { label: '1-10s', min: 1000, max: 10000, description: 'Sound effects' },
    { label: '10-60s', min: 10000, max: 60000, description: 'Medium clips' },
    { label: '1-5 min', min: 60000, max: 300000, description: 'Longer audio' },
    { label: '> 1 min', min: 60000, max: null, description: 'Long form' },
  ];

  /**
   * Parse a duration string into milliseconds
   * Supports: "50ms", "1.5s", "2m", "2m30s", "1:30", or plain numbers (seconds)
   */
  function parseDuration(input: string): number | null {
    if (!input.trim()) return null;

    const value = input.trim().toLowerCase();

    // Handle time format like "1:30" (minutes:seconds)
    const timeMatch = value.match(/^(\d+):(\d+(?:\.\d+)?)$/);
    if (timeMatch) {
      const minutes = parseFloat(timeMatch[1]);
      const seconds = parseFloat(timeMatch[2]);
      return Math.round((minutes * 60 + seconds) * 1000);
    }

    // Handle combined format like "2m30s"
    const combinedMatch = value.match(/^(\d+(?:\.\d+)?)\s*m(?:in)?\s*(\d+(?:\.\d+)?)\s*s(?:ec)?$/);
    if (combinedMatch) {
      const minutes = parseFloat(combinedMatch[1]);
      const seconds = parseFloat(combinedMatch[2]);
      return Math.round((minutes * 60 + seconds) * 1000);
    }

    // Handle single unit formats
    const match = value.match(/^(\d+(?:\.\d+)?)\s*(ms|s|sec|m|min)?$/);
    if (!match) return null;

    const num = parseFloat(match[1]);
    const unit = match[2] || 's'; // Default to seconds

    switch (unit) {
      case 'ms':
        return Math.round(num);
      case 's':
      case 'sec':
        return Math.round(num * 1000);
      case 'm':
      case 'min':
        return Math.round(num * 60 * 1000);
      default:
        return Math.round(num * 1000);
    }
  }

  /**
   * Format milliseconds to a human-readable string
   */
  function formatDuration(ms: number | null): string {
    if (ms === null) return '';

    if (ms < 1000) {
      return `${ms}ms`;
    } else if (ms < 60000) {
      const seconds = ms / 1000;
      return seconds % 1 === 0 ? `${seconds}s` : `${seconds.toFixed(1)}s`;
    } else {
      const minutes = Math.floor(ms / 60000);
      const seconds = Math.round((ms % 60000) / 1000);
      if (seconds === 0) {
        return `${minutes}m`;
      }
      return `${minutes}m${seconds}s`;
    }
  }

  // Check if filter is active
  let isFilterActive = $derived(
    assetsState.durationFilter.minMs !== null || assetsState.durationFilter.maxMs !== null,
  );

  // Summary text for the button
  let filterSummary = $derived(() => {
    const { minMs, maxMs } = assetsState.durationFilter;
    if (minMs === null && maxMs === null) return 'Duration';
    if (minMs !== null && maxMs !== null) {
      return `${formatDuration(minMs)} - ${formatDuration(maxMs)}`;
    }
    if (minMs !== null) return `> ${formatDuration(minMs)}`;
    if (maxMs !== null) return `< ${formatDuration(maxMs)}`;
    return 'Duration';
  });

  function reloadWithFilter() {
    // If similarity search is active, re-run with the new filter
    if (clapState.similarToAssetId !== null && clapState.similarToFilename) {
      clapState.searchBySimilarity(clapState.similarToAssetId, clapState.similarToFilename, undefined, assetsState.durationFilter);
    } else if (clapState.semanticSearchEnabled && clapState.lastSearchQuery) {
      // If semantic search is active, re-run semantic search with the new filter
      clapState.search(clapState.lastSearchQuery, undefined, assetsState.durationFilter);
    } else {
      // Otherwise reload regular assets
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      assetsState.loadAssets(currentType);
    }
  }

  function applyFilter() {
    const minMs = parseDuration(minInput);
    const maxMs = parseDuration(maxInput);

    assetsState.setDurationFilter(minMs, maxMs);
    reloadWithFilter();

    isOpen = false;
  }

  function applyPreset(preset: (typeof presets)[0]) {
    assetsState.setDurationFilter(preset.min, preset.max);

    // Update inputs to show the preset values
    minInput = preset.min !== null ? formatDuration(preset.min) : '';
    maxInput = preset.max !== null ? formatDuration(preset.max) : '';

    reloadWithFilter();

    isOpen = false;
  }

  function clearFilter() {
    minInput = '';
    maxInput = '';
    assetsState.setDurationFilter(null, null);

    reloadWithFilter();

    isOpen = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      applyFilter();
    } else if (e.key === 'Escape') {
      isOpen = false;
    }
  }

  // Sync inputs when filter changes externally
  $effect(() => {
    if (!isOpen) {
      minInput = formatDuration(assetsState.durationFilter.minMs);
      maxInput = formatDuration(assetsState.durationFilter.maxMs);
    }
  });
</script>

<div class="relative">
  <!-- Toggle Button -->
  <button
    onclick={() => (isOpen = !isOpen)}
    class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-md transition-colors"
    class:bg-blue-500={isFilterActive}
    class:text-white={isFilterActive}
    class:bg-secondary={!isFilterActive}
    class:text-secondary={!isFilterActive}
    class:hover:bg-blue-600={isFilterActive}
    class:hover:bg-tertiary={!isFilterActive}
    title="Filter by duration"
  >
    <!-- Clock icon -->
    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
      />
    </svg>
    <span class="max-w-[120px] truncate">{filterSummary()}</span>
    {#if isFilterActive}
      <span
        role="button"
        tabindex="0"
        onclick={(e) => {
          e.stopPropagation();
          clearFilter();
        }}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.stopPropagation();
            clearFilter();
          }
        }}
        class="ml-1 hover:bg-blue-600 rounded p-0.5 cursor-pointer"
        title="Clear filter"
      >
        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M6 18L18 6M6 6l12 12"
          />
        </svg>
      </span>
    {/if}
  </button>

  <!-- Popover -->
  {#if isOpen}
    <!-- Backdrop -->
    <button
      class="fixed inset-0 z-40"
      onclick={() => (isOpen = false)}
      onkeydown={(e) => e.key === 'Escape' && (isOpen = false)}
      aria-label="Close duration filter"
    ></button>

    <!-- Popover content -->
    <div
      class="absolute top-full left-0 mt-2 z-50 w-72 bg-elevated border border-default rounded-lg shadow-lg p-4"
    >
      <h3 class="text-sm font-medium text-primary mb-3">Filter by Duration</h3>

      <!-- Custom range inputs -->
      <div class="flex items-center gap-2 mb-4">
        <div class="flex-1">
          <label for="duration-min" class="block text-xs text-secondary mb-1">Min</label>
          <input
            id="duration-min"
            type="text"
            bind:value={minInput}
            onkeydown={handleKeydown}
            placeholder="e.g. 50ms, 1s"
            class="w-full px-2 py-1.5 text-sm border border-default rounded bg-primary text-primary placeholder:text-tertiary focus:outline-none focus:ring-2 focus:ring-accent"
          />
        </div>
        <span class="text-secondary mt-5">-</span>
        <div class="flex-1">
          <label for="duration-max" class="block text-xs text-secondary mb-1">Max</label>
          <input
            id="duration-max"
            type="text"
            bind:value={maxInput}
            onkeydown={handleKeydown}
            placeholder="e.g. 5s, 2m"
            class="w-full px-2 py-1.5 text-sm border border-default rounded bg-primary text-primary placeholder:text-tertiary focus:outline-none focus:ring-2 focus:ring-accent"
          />
        </div>
      </div>

      <!-- Format hint -->
      <p class="text-xs text-tertiary mb-3">Formats: 50ms, 1.5s, 2m, 2m30s, 1:30</p>

      <!-- Apply button -->
      <button
        onclick={applyFilter}
        class="w-full mb-4 px-3 py-1.5 text-sm font-medium bg-accent text-white rounded hover:bg-accent-hover transition-colors"
      >
        Apply
      </button>

      <!-- Quick presets -->
      <div class="border-t border-default pt-3">
        <p class="text-xs text-secondary mb-2">Quick presets</p>
        <div class="flex flex-wrap gap-1.5">
          {#each presets as preset}
            <button
              onclick={() => applyPreset(preset)}
              class="px-2 py-1 text-xs rounded border border-default bg-secondary text-primary hover:bg-tertiary transition-colors"
              title={preset.description}
            >
              {preset.label}
            </button>
          {/each}
        </div>
      </div>

      <!-- Clear button -->
      {#if isFilterActive}
        <button
          onclick={clearFilter}
          class="w-full mt-3 px-3 py-1.5 text-sm text-secondary hover:text-primary transition-colors"
        >
          Clear filter
        </button>
      {/if}
    </div>
  {/if}
</div>
