<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getDatabase } from '$lib/database/connection';
  import { getThumbnail } from '$lib/database/queries';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  interface Props {
    assetId: number;
    size?: 'small' | 'medium' | 'large';
  }

  let { assetId, size = 'medium' }: Props = $props();

  let thumbnailUrl = $state<string | null>(null);
  let isLoading = $state(true);
  let error = $state<string | null>(null);
  let containerElement: HTMLDivElement;
  let hasLoaded = $state(false);
  let destroyed = false;

  const sizeClasses = $derived.by(() => {
    switch (size) {
      case 'small': return 'h-32';
      case 'medium': return 'h-48';
      case 'large': return 'h-64';
    }
  });

  async function loadThumbnail() {
    if (hasLoaded) return;
    hasLoaded = true;

    try {
      const db = await getDatabase();
      const thumbnailData = await getThumbnail(db, assetId);

      if (destroyed) return;

      if (thumbnailData) {
        const blob = new Blob([thumbnailData], { type: 'image/webp' });
        thumbnailUrl = URL.createObjectURL(blob);
      } else {
        const bytes = await invoke<number[]>('get_asset_bytes', { assetId });
        if (destroyed) return;
        const blob = new Blob([new Uint8Array(bytes)]);
        thumbnailUrl = URL.createObjectURL(blob);
      }
    } catch (e) {
      if (!destroyed) {
        error = String(e);
      }
    } finally {
      if (!destroyed) {
        isLoading = false;
      }
    }
  }

  // Revoke blob URLs when they change or on component destroy
  $effect(() => {
    const url = thumbnailUrl;
    return () => {
      if (url) {
        URL.revokeObjectURL(url);
      }
    };
  });

  onMount(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            loadThumbnail();
            observer.unobserve(entry.target);
          }
        });
      },
      {
        rootMargin: '200px',
        threshold: 0.01
      }
    );

    if (containerElement) {
      observer.observe(containerElement);
    }

    return () => {
      destroyed = true;
      observer.disconnect();
    };
  });
</script>

<div bind:this={containerElement} class="w-full flex items-center justify-center bg-tertiary overflow-hidden {sizeClasses}">
  {#if isLoading}
    <div class="flex items-center justify-center w-full h-full">
      <Spinner size="md" />
    </div>
  {:else if error || !thumbnailUrl}
    <div class="flex items-center justify-center w-full h-full">
      <span class="text-xs text-secondary">No preview</span>
    </div>
  {:else}
    <img
      src={thumbnailUrl}
      alt="Thumbnail"
      class="w-full h-full object-cover"
    />
  {/if}
</div>
