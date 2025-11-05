<script lang="ts">
  import type { Asset } from '$lib/types';
  import { formatFileSize } from '$lib/state/assets.svelte';
  import AssetThumbnail from './AssetThumbnail.svelte';
  import Badge from './shared/Badge.svelte';

  interface Props {
    assets: Asset[];
    isLoading?: boolean;
  }

  let { assets, isLoading = false }: Props = $props();

  function formatDimensions(asset: Asset): string {
    if (asset.width && asset.height) {
      return `${asset.width} × ${asset.height}`;
    } else if (asset.duration_ms) {
      return `${(asset.duration_ms / 1000).toFixed(1)}s`;
    }
    return '—';
  }

  function formatLocation(asset: Asset): string {
    if (asset.zip_entry) {
      // Extract zip filename from path
      const zipName = asset.path.split(/[\\/]/).pop() || asset.path;
      return `${zipName}/${asset.zip_entry}`;
    }
    return asset.path;
  }
</script>

<div class="flex-1 flex flex-col overflow-auto">
  {#if isLoading}
    <div class="flex items-center justify-center h-full">
      <p class="text-secondary">Loading...</p>
    </div>
  {:else if assets.length === 0}
    <div class="flex items-center justify-center h-full">
      <p class="text-secondary">No assets found.</p>
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
        {#each assets as asset (asset.id)}
          <tr class="border-b border-default hover:bg-secondary">
            <td class="px-4 py-2">
              <AssetThumbnail assetId={asset.id} assetType={asset.asset_type} />
            </td>
            <td class="px-4 py-2 text-sm text-primary" title={formatLocation(asset)}>
              <div class="flex items-center gap-2">
                <span>{asset.filename}</span>
                {#if asset.zip_entry}
                  <Badge variant="info">ZIP</Badge>
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
