<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';

  // Only show for images tab
  let showToggle = $derived(viewState.activeTab === 'images');
</script>

{#if showToggle}
  <div class="flex items-center gap-2">
    <button
      class="p-2 rounded transition-colors border border-transparent"
      class:bg-accent={viewState.layoutMode === 'grid'}
      class:text-white={viewState.layoutMode === 'grid'}
      class:hover:bg-tertiary={viewState.layoutMode !== 'grid'}
      onclick={() => (viewState.layoutMode = 'grid')}
      title="Grid View"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M4 5a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1H5a1 1 0 01-1-1V5zM14 5a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1V5zM4 15a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1H5a1 1 0 01-1-1v-4zM14 15a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4z"
        />
      </svg>
    </button>

    <button
      class="p-2 rounded transition-colors border border-transparent"
      class:bg-accent={viewState.layoutMode === 'table'}
      class:text-white={viewState.layoutMode === 'table'}
      class:hover:bg-tertiary={viewState.layoutMode !== 'table'}
      onclick={() => (viewState.layoutMode = 'table')}
      title="Table View"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M3 10h18M3 14h18M3 6h18M3 18h18"
        />
      </svg>
    </button>

    <!-- Thumbnail size selector (for grid mode only) -->
    {#if viewState.layoutMode === 'grid'}
      <div class="flex items-center gap-1 ml-2 border border-default rounded overflow-hidden">
        <button
          class="px-2 py-1 text-xs font-medium transition-colors"
          class:bg-accent={viewState.thumbnailSize === 'small'}
          class:text-white={viewState.thumbnailSize === 'small'}
          class:hover:bg-tertiary={viewState.thumbnailSize !== 'small'}
          onclick={() => viewState.setThumbnailSize('small')}
        >
          S
        </button>
        <button
          class="px-2 py-1 text-xs font-medium transition-colors border-x border-default"
          class:bg-accent={viewState.thumbnailSize === 'medium'}
          class:text-white={viewState.thumbnailSize === 'medium'}
          class:hover:bg-tertiary={viewState.thumbnailSize !== 'medium'}
          onclick={() => viewState.setThumbnailSize('medium')}
        >
          M
        </button>
        <button
          class="px-2 py-1 text-xs font-medium transition-colors"
          class:bg-accent={viewState.thumbnailSize === 'large'}
          class:text-white={viewState.thumbnailSize === 'large'}
          class:hover:bg-tertiary={viewState.thumbnailSize !== 'large'}
          onclick={() => viewState.setThumbnailSize('large')}
        >
          L
        </button>
      </div>
    {/if}
  </div>
{/if}
