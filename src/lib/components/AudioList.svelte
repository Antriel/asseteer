<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset } from '$lib/types';
  import AudioPlayer from './AudioPlayer.svelte';

  interface Props {
    assets: Asset[];
  }

  let { assets }: Props = $props();

  let selectedAsset = $state<Asset | null>(null);
  let isPlaying = $state(false);

  let containerElement: HTMLDivElement;
  let scrollTop = $state(0);
  let containerHeight = $state(0);

  // Item height: button with h-20 (80px) + gap-2 (8px) = 88px per item
  const itemHeight = 88;
  const bufferItems = 5; // Extra items above and below for smooth scrolling

  // Calculate virtual scrolling parameters
  const totalItems = $derived(assets.length);
  const totalHeight = $derived(totalItems * itemHeight);
  const visibleItems = $derived(Math.ceil(containerHeight / itemHeight) + 1);

  const startIndex = $derived(Math.max(0, Math.floor(scrollTop / itemHeight) - bufferItems));
  const endIndex = $derived(Math.min(totalItems, startIndex + visibleItems + bufferItems * 2));

  const visibleAssets = $derived(assets.slice(startIndex, endIndex));
  const offsetY = $derived(startIndex * itemHeight);

  function formatDuration(ms: number): string {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
  }

  function formatFileSize(bytes: number): string {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }

  function loadAsset(asset: Asset) {
    selectedAsset = asset;
  }

  function handleScroll(event: Event) {
    const target = event.target as HTMLDivElement;
    scrollTop = target.scrollTop;
  }

  function updateContainerHeight() {
    if (containerElement) {
      containerHeight = containerElement.clientHeight;
    }
  }

  onMount(() => {
    updateContainerHeight();

    // Update on window resize
    const resizeObserver = new ResizeObserver(() => {
      updateContainerHeight();
    });

    if (containerElement) {
      resizeObserver.observe(containerElement);
    }

    return () => {
      resizeObserver.disconnect();
    };
  });
</script>

<div class="flex flex-col gap-4 p-4 h-full overflow-hidden">
  <!-- Single player at the top -->
  {#if selectedAsset}
    <div class="p-4 bg-primary border border-default rounded-lg shadow-lg flex-shrink-0">
      <div class="flex items-center gap-4 mb-3">
        <div class="w-12 h-12 flex items-center justify-center bg-accent rounded-lg flex-shrink-0">
          <svg class="w-6 h-6 text-white" fill="currentColor" viewBox="0 0 20 20">
            <path d="M18 3a1 1 0 00-1.196-.98l-10 2A1 1 0 006 5v9.114A4.369 4.369 0 005 14c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V7.82l8-1.6v5.894A4.37 4.37 0 0015 12c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V3z" />
          </svg>
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-semibold text-primary whitespace-nowrap overflow-hidden text-ellipsis">
            {selectedAsset.filename}
          </p>
          <div class="flex gap-4 mt-1 text-xs text-secondary">
            {#if selectedAsset.duration_ms}
              <span>{formatDuration(selectedAsset.duration_ms)}</span>
            {/if}
            {#if selectedAsset.sample_rate}
              <span>{selectedAsset.sample_rate / 1000} kHz</span>
            {/if}
            {#if selectedAsset.channels}
              <span>{selectedAsset.channels === 1 ? 'Mono' : 'Stereo'}</span>
            {/if}
            <span>{selectedAsset.format.toUpperCase()}</span>
          </div>
        </div>
      </div>
      <AudioPlayer
        asset={selectedAsset}
        isActive={true}
        onPlay={() => isPlaying = true}
        onPause={() => isPlaying = false}
      />
    </div>
  {:else}
    <div class="p-6 bg-secondary border border-default rounded-lg text-center text-secondary flex-shrink-0">
      Select an audio file to play
    </div>
  {/if}

  <!-- List of audio assets with virtual scrolling -->
  <div
    bind:this={containerElement}
    class="relative overflow-y-auto flex-1"
    onscroll={handleScroll}
  >
    <div style="height: {totalHeight}px; position: relative;">
      <div
        class="flex flex-col gap-2 absolute w-full"
        style="transform: translateY({offsetY}px);"
      >
        {#each visibleAssets as asset, idx (asset.id)}
          {@const index = startIndex + idx}
          <button
            class="flex items-center gap-4 p-4 bg-secondary border border-default rounded-lg transition-all hover:border-accent cursor-pointer text-left h-20"
            class:!bg-accent-light={selectedAsset?.id === asset.id}
            class:!border-accent={selectedAsset?.id === asset.id}
            onclick={() => loadAsset(asset)}
          >
            <!-- Audio icon -->
            <div class="w-12 h-12 flex items-center justify-center bg-primary rounded-lg flex-shrink-0">
              <svg class="w-6 h-6 text-secondary" fill="currentColor" viewBox="0 0 20 20">
                <path d="M18 3a1 1 0 00-1.196-.98l-10 2A1 1 0 006 5v9.114A4.369 4.369 0 005 14c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V7.82l8-1.6v5.894A4.37 4.37 0 0015 12c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V3z" />
              </svg>
            </div>

            <!-- Audio metadata -->
            <div class="flex-1 min-w-0">
              <p class="font-semibold text-primary whitespace-nowrap overflow-hidden text-ellipsis">
                {asset.filename}
              </p>
              <div class="flex gap-4 mt-1 text-xs text-secondary">
                {#if asset.duration_ms}
                  <span>{formatDuration(asset.duration_ms)}</span>
                {/if}
                {#if asset.sample_rate}
                  <span>{asset.sample_rate / 1000} kHz</span>
                {/if}
                {#if asset.channels}
                  <span>{asset.channels === 1 ? 'Mono' : 'Stereo'}</span>
                {/if}
                <span>{asset.format.toUpperCase()}</span>
                <span>{formatFileSize(asset.file_size)}</span>
              </div>
            </div>

            <!-- Playing indicator -->
            {#if selectedAsset?.id === asset.id}
              <div class="flex-shrink-0">
                {#if isPlaying}
                  <svg class="w-5 h-5 text-accent" fill="currentColor" viewBox="0 0 20 20">
                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd" />
                  </svg>
                {:else}
                  <svg class="w-5 h-5 text-accent" fill="currentColor" viewBox="0 0 20 20">
                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clip-rule="evenodd" />
                  </svg>
                {/if}
              </div>
            {/if}
          </button>
        {/each}
      </div>
    </div>
  </div>
</div>
