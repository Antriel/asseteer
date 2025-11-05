<script lang="ts">
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import ViewModeToggle from './ViewModeToggle.svelte';

  let searchInput = $state('');

  function handleSearch(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchInput = value;
    assetsState.searchAssets(value, viewState.activeTab === 'images' ? 'image' : 'audio');
  }
</script>

<div class="flex items-center gap-4 px-4 py-3 bg-secondary border-b border-default">
  <!-- Search -->
  <div class="relative flex-1 max-w-[400px]">
    <svg class="absolute left-2 top-1/2 -translate-y-1/2 w-4 h-4 text-secondary pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
    </svg>
    <input
      type="text"
      placeholder="Search {viewState.activeTab}..."
      value={searchInput}
      oninput={handleSearch}
      class="w-full py-2 px-2 pl-8 border border-default rounded-md bg-primary text-primary placeholder:text-secondary focus:outline-none focus:ring-2 focus:ring-accent"
    />
  </div>

  <!-- View mode toggle (images only) -->
  <ViewModeToggle />

  <!-- Stats -->
  <div class="ml-auto">
    <span class="text-sm text-secondary">
      {assetsState.assets.length} {viewState.activeTab}
    </span>
  </div>
</div>
