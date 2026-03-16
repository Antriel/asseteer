<script lang="ts">
  import { onMount } from 'svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import DirectoryTree from './DirectoryTree.svelte';
  import ImageGrid from './ImageGrid.svelte';
  import AssetList from './AssetList.svelte';
  import AudioList from './AudioList.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import { FolderIcon } from '$lib/components/icons';

  onMount(() => {
    if (exploreState.roots.length === 0) {
      exploreState.loadRoots();
    }
  });

  let imageAssets = $derived(exploreState.assets.filter(a => a.asset_type === 'image'));
  let audioAssets = $derived(exploreState.assets.filter(a => a.asset_type === 'audio'));
  let hasAssets = $derived(exploreState.assets.length > 0);

  // Show the selected directory name
  let selectedDirName = $derived(() => {
    if (!exploreState.selectedPath) return '';
    const segments = exploreState.selectedPath.split('/');
    return segments[segments.length - 1] || exploreState.selectedPath;
  });
</script>

<div class="flex h-full overflow-hidden">
  <!-- Directory Tree Panel -->
  <div class="w-64 flex-shrink-0 border-r border-default overflow-y-auto bg-secondary">
    <div class="px-3 py-2 text-xs font-semibold text-tertiary uppercase tracking-wider border-b border-default">
      Folders
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
        <DirectoryTree nodes={exploreState.roots} />
      </div>
    {/if}
  </div>

  <!-- Content Panel -->
  <div class="flex-1 overflow-hidden">
    {#if exploreState.isLoading}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        <Spinner size="lg" />
        <p class="text-secondary">Loading files...</p>
      </div>
    {:else if !exploreState.selectedPath}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        <FolderIcon size="lg" class="text-tertiary w-16 h-16" />
        <p class="text-primary font-medium">Select a folder</p>
        <p class="text-sm text-secondary">Choose a folder from the tree to browse its contents</p>
      </div>
    {:else if !hasAssets}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        <svg class="w-16 h-16 text-tertiary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
        </svg>
        <p class="text-primary font-medium">No assets in this folder</p>
        <p class="text-sm text-secondary">This folder doesn't contain any recognized assets</p>
      </div>
    {:else}
      <!-- Directory header -->
      <div class="flex items-center gap-2 px-4 py-2 bg-secondary border-b border-default">
        <FolderIcon size="sm" class="text-secondary" />
        <span class="text-sm text-primary font-medium truncate">{selectedDirName()}</span>
        <span class="text-xs text-tertiary ml-auto">
          {exploreState.assets.length} {exploreState.assets.length === 1 ? 'file' : 'files'}
          {#if imageAssets.length > 0 && audioAssets.length > 0}
            ({imageAssets.length} images, {audioAssets.length} audio)
          {/if}
        </span>
      </div>

      <!-- Mixed content: show images and audio together -->
      <div class="flex-1 overflow-auto h-[calc(100%-40px)]">
        {#if imageAssets.length > 0 && audioAssets.length === 0}
          {#if viewState.layoutMode === 'grid'}
            <ImageGrid assets={imageAssets} />
          {:else}
            <AssetList assets={imageAssets} isLoading={false} />
          {/if}
        {:else if audioAssets.length > 0 && imageAssets.length === 0}
          <AudioList assets={audioAssets} />
        {:else}
          <!-- Mixed: show images first, then audio -->
          <div class="flex flex-col h-full">
            {#if imageAssets.length > 0}
              <div class="px-4 py-1 text-xs font-semibold text-tertiary uppercase border-b border-default">Images ({imageAssets.length})</div>
              <div class="flex-1 min-h-0">
                <AssetList assets={imageAssets} isLoading={false} />
              </div>
            {/if}
            {#if audioAssets.length > 0}
              <div class="px-4 py-1 text-xs font-semibold text-tertiary uppercase border-b border-default">Audio ({audioAssets.length})</div>
              <div class="flex-1 min-h-0">
                <AudioList assets={audioAssets} />
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
