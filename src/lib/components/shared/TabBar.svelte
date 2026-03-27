<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { FolderIcon } from '$lib/components/icons';
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
    const assetType = tab === 'images' ? 'image' : 'audio';
    assetsState.loadAssets(assetType);
  }
</script>

<div class="flex items-center gap-1 border-b border-default bg-secondary px-4">
  <!-- Asset type tabs -->
  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab ===
    'audio'
      ? 'text-accent border-accent'
      : 'text-secondary border-transparent'}"
    onclick={() => switchTab('audio')}
  >
    Audio
    <Badge variant="count">{audioCount}</Badge>
  </button>

  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab ===
    'images'
      ? 'text-accent border-accent'
      : 'text-secondary border-transparent'}"
    onclick={() => switchTab('images')}
  >
    Images
    <Badge variant="count">{imageCount}</Badge>
  </button>

  <!-- Folder sidebar toggle -->
  <div class="ml-auto">
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-md transition-colors {viewState.folderSidebarOpen
        ? 'bg-accent-muted text-accent'
        : 'text-secondary hover:text-primary hover:bg-tertiary'}"
      onclick={() => viewState.toggleFolderSidebar()}
      title={viewState.folderSidebarOpen ? 'Hide folder panel' : 'Show folder panel'}
    >
      <FolderIcon size="sm" />
      <span>Folders</span>
    </button>
  </div>
</div>
