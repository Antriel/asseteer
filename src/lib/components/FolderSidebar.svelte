<script lang="ts">
  import { onMount } from 'svelte';
  import { exploreState, type DirectoryNode } from '$lib/state/explore.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { ZIP_SEP } from '$lib/database/queries';
  import DirectoryTree from './DirectoryTree.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import { FolderIcon } from '$lib/components/icons';

  onMount(() => {
    if (exploreState.roots.length === 0) {
      exploreState.loadRoots();
    }
  });

  function selectFolder(node: DirectoryNode) {
    exploreState.selectedPath = node.path;
    // Build the folder filter string — ZIP nodes use the :: encoding
    let filterPath: string;
    if (node.zipPrefix !== undefined) {
      // ZIP node: encode as "zipFilePath::prefix"
      // For ZIP root nodes, path is the .zip file and zipPrefix is ''
      // For ZIP subdirectory nodes, path already contains the :: encoding
      if (node.path.includes(ZIP_SEP)) {
        filterPath = node.path;
      } else {
        filterPath = node.path + ZIP_SEP + node.zipPrefix;
      }
    } else {
      filterPath = node.path;
    }
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.setFolderFilter(filterPath, assetType);
  }

  function clearFolder() {
    exploreState.selectedPath = null;
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.setFolderFilter(null, assetType);
  }
</script>

<div class="w-64 flex-shrink-0 border-r border-default overflow-y-auto bg-secondary">
  <div class="flex items-center justify-between px-3 py-2 border-b border-default">
    <span class="text-xs font-semibold text-tertiary uppercase tracking-wider">Folders</span>
    {#if assetsState.folderPath}
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
    <div class="py-1">
      <DirectoryTree nodes={exploreState.roots} onSelect={selectFolder} />
    </div>
  {/if}
</div>
