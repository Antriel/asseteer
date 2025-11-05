<script lang="ts">
  import { onMount } from 'svelte';
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

      if (thumbnailData) {
        const blob = new Blob([thumbnailData], { type: 'image/jpeg' });
        thumbnailUrl = URL.createObjectURL(blob);
      }
    } catch (e) {
      error = String(e);
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    // Create intersection observer with root margin for preloading
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
        rootMargin: '200px', // Load 200px before entering viewport
        threshold: 0.01
      }
    );

    if (containerElement) {
      observer.observe(containerElement);
    }

    return () => {
      observer.disconnect();
      if (thumbnailUrl) {
        URL.revokeObjectURL(thumbnailUrl);
      }
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
