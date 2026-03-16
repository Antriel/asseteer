<script lang="ts">
  import { onMount } from 'svelte';
  import { getThumbnailUrl, hasThumbnailFailed, requestThumbnail } from '$lib/state/thumbnails.svelte';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  interface Props {
    assetId: number;
    size?: 'small' | 'medium' | 'large';
  }

  let { assetId, size = 'medium' }: Props = $props();

  let containerElement: HTMLDivElement;
  let isVisible = $state(false);

  const sizeClasses = $derived.by(() => {
    switch (size) {
      case 'small': return 'h-32';
      case 'medium': return 'h-48';
      case 'large': return 'h-64';
    }
  });

  // Request thumbnail when visible
  $effect(() => {
    if (isVisible) {
      requestThumbnail(assetId);
    }
  });

  let thumbnailUrl = $derived(getThumbnailUrl(assetId));
  let thumbnailFailed = $derived(hasThumbnailFailed(assetId));
  let isLoading = $derived(isVisible && !thumbnailUrl && !thumbnailFailed);

  onMount(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            isVisible = true;
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
      observer.disconnect();
    };
  });
</script>

<div bind:this={containerElement} class="w-full flex items-center justify-center bg-tertiary overflow-hidden {sizeClasses}">
  {#if isLoading}
    <div class="flex items-center justify-center w-full h-full">
      <Spinner size="md" />
    </div>
  {:else if thumbnailFailed || !thumbnailUrl}
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
