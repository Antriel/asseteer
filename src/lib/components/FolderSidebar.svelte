<script lang="ts">
  import { onMount } from 'svelte';
  import { exploreState, type DirectoryNode } from '$lib/state/explore.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import DirectoryTree from './DirectoryTree.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import { FolderIcon } from '$lib/components/icons';

  onMount(() => {
    if (exploreState.roots.length === 0) {
      exploreState.loadRoots();
    }
  });

  function selectFolder(node: DirectoryNode) {
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.setFolderFilter(node.location, assetType);
    exploreState.selectedKey = node.key;
    exploreState.selectedLocation = node.location;
  }

  function clearFolder() {
    exploreState.selectedKey = null;
    exploreState.selectedLocation = null;
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.setFolderFilter(null, assetType);
  }
</script>

<div class="w-full h-full border-r border-default bg-secondary flex flex-col">
  <div class="flex items-center justify-between px-3 py-2 border-b border-default flex-shrink-0">
    <div class="flex items-center gap-2">
      <span class="text-xs font-semibold text-tertiary uppercase tracking-wider">Folders</span>
      {#if exploreState.isNavigating}
        <Spinner size="sm" />
      {/if}
    </div>
    {#if assetsState.folderLocation}
      <button
        class="text-xs text-secondary hover:text-primary transition-colors"
        onclick={clearFolder}
        title="Clear folder filter"
      >
        Clear
      </button>
    {/if}
  </div>
  {#if exploreState.isLoadingRoots}
    <div class="flex items-center justify-center py-8">
      <Spinner size="sm" />
    </div>
  {:else if exploreState.roots.length === 0}
    <div class="flex flex-col items-center justify-center py-8 px-4 text-center">
      <FolderIcon size="lg" class="text-tertiary mb-2" />
      <p class="text-sm text-secondary">No folders found</p>
      <p class="text-xs text-tertiary mt-1">Scan folders to see them here</p>
    </div>
  {:else}
    <div class="flex-1 overflow-auto py-1">
      <div class="min-w-fit">
        <DirectoryTree nodes={exploreState.roots} onSelect={selectFolder} />
      </div>
    </div>
  {/if}
</div>
