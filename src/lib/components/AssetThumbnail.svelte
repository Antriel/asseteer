<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  interface Props {
    assetId: number;
    assetType: string;
  }

  let { assetId, assetType }: Props = $props();

  let thumbnailUrl = $state<string | null>(null);
  let isLoading = $state(true);
  let error = $state<string | null>(null);

  async function loadThumbnail() {
    if (assetType !== 'image') {
      isLoading = false;
      return;
    }

    try {
      const bytes = await invoke<number[]>('get_thumbnail', { assetId });

      // Convert array to Uint8Array
      const uint8Array = new Uint8Array(bytes);

      // Create blob from bytes
      const blob = new Blob([uint8Array], { type: 'image/jpeg' });

      // Create object URL
      thumbnailUrl = URL.createObjectURL(blob);
    } catch (err) {
      console.error('Failed to load thumbnail:', err);
      error = String(err);
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadThumbnail();

    // Cleanup object URL on unmount
    return () => {
      if (thumbnailUrl) {
        URL.revokeObjectURL(thumbnailUrl);
      }
    };
  });
</script>

<div class="flex items-center justify-center w-16 h-16 bg-secondary border border-default rounded overflow-hidden">
  {#if isLoading}
    <span class="text-xs text-secondary">...</span>
  {:else if error}
    <span class="text-xs text-red-500">Error</span>
  {:else if thumbnailUrl}
    <img src={thumbnailUrl} alt="Thumbnail" class="w-full h-full object-cover" />
  {:else if assetType === 'audio'}
    <span class="text-xs text-secondary">🎵</span>
  {:else}
    <span class="text-xs text-secondary">No preview</span>
  {/if}
</div>
