<script lang="ts">
  import { assetsState, formatFileSize } from '$lib/state/assets.svelte';
  import AssetThumbnail from './AssetThumbnail.svelte';
  import { onMount } from 'svelte';

  onMount(() => {
    assetsState.loadAssets();
  });

  function formatDimensions(asset: any): string {
    if (asset.width && asset.height) {
      return `${asset.width} × ${asset.height}`;
    } else if (asset.duration_ms) {
      return `${(asset.duration_ms / 1000).toFixed(1)}s`;
    }
    return '—';
  }

  function formatLocation(asset: any): string {
    if (asset.zip_entry) {
      // Extract zip filename from path
      const zipName = asset.path.split(/[\\/]/).pop() || asset.path;
      return `${zipName}/${asset.zip_entry}`;
    }
    return asset.path;
  }
</script>

<div class="flex-1 flex flex-col overflow-hidden">
  <!-- Search bar -->
  <div class="p-4 border-b border-default bg-secondary">
    <input
      type="text"
      placeholder="Search assets..."
      value={assetsState.searchText}
      oninput={(e) => assetsState.searchAssets(e.currentTarget.value)}
      class="input w-full"
    />
  </div>

  <!-- Asset table -->
  <div class="flex-1 overflow-auto">
    {#if assetsState.isLoading}
      <div class="flex items-center justify-center h-full">
        <p class="text-secondary">Loading...</p>
      </div>
    {:else if assetsState.assets.length === 0}
      <div class="flex items-center justify-center h-full">
        <p class="text-secondary">No assets found. Scan a folder to get started.</p>
      </div>
    {:else}
      <table class="w-full">
        <thead class="sticky top-0 bg-secondary border-b border-default">
          <tr>
            <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Preview</th>
            <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Name</th>
            <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Type</th>
            <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Dimensions</th>
            <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Size</th>
          </tr>
        </thead>
        <tbody>
          {#each assetsState.assets as asset (asset.id)}
            <tr class="border-b border-default hover:bg-secondary">
              <td class="px-4 py-2">
                <AssetThumbnail assetId={asset.id} assetType={asset.asset_type} />
              </td>
              <td class="px-4 py-2 text-sm text-primary" title={formatLocation(asset)}>
                <div class="flex items-center gap-2">
                  <span>{asset.filename}</span>
                  {#if asset.zip_entry}
                    <span class="text-xs px-1.5 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300">
                      ZIP
                    </span>
                  {/if}
                </div>
              </td>
              <td class="px-4 py-2 text-sm text-secondary">
                {asset.asset_type}
              </td>
              <td class="px-4 py-2 text-sm text-secondary">
                {formatDimensions(asset)}
              </td>
              <td class="px-4 py-2 text-sm text-secondary">
                {formatFileSize(asset.file_size)}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>
