<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { SearchIcon, FolderIcon } from '$lib/components/icons';
  import Badge from './Badge.svelte';

  interface Props {
    imageCount: number;
    audioCount: number;
  }

  let { imageCount, audioCount }: Props = $props();

  function switchTab(tab: 'images' | 'audio') {
    viewState.setActiveTab(tab);
    if (tab === 'images') {
      assetsState.setDurationFilter(null, null);
    }
    assetsState.loadAssets(tab === 'images' ? 'image' : 'audio');
  }
</script>

<div class="flex items-center gap-1 border-b border-default bg-secondary px-4">
  <!-- Asset type tabs -->
  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab === 'images' ? 'text-accent border-accent' : 'text-secondary border-transparent'}"
    onclick={() => switchTab('images')}
  >
    Images
    <Badge variant="count">{imageCount}</Badge>
  </button>

  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab === 'audio' ? 'text-accent border-accent' : 'text-secondary border-transparent'}"
    onclick={() => switchTab('audio')}
  >
    Audio
    <Badge variant="count">{audioCount}</Badge>
  </button>

  <!-- View mode switcher (Search / Explore) -->
  <div class="ml-auto flex items-center gap-1 bg-tertiary rounded-md p-0.5">
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded transition-colors {viewState.libraryView === 'search' ? 'bg-primary text-primary shadow-sm' : 'text-secondary hover:text-primary'}"
      onclick={() => viewState.setLibraryView('search')}
      title="Search view"
    >
      <SearchIcon size="sm" />
      <span>Search</span>
    </button>
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded transition-colors {viewState.libraryView === 'explore' ? 'bg-primary text-primary shadow-sm' : 'text-secondary hover:text-primary'}"
      onclick={() => viewState.setLibraryView('explore')}
      title="Explore folders"
    >
      <FolderIcon size="sm" />
      <span>Explore</span>
    </button>
  </div>
</div>
