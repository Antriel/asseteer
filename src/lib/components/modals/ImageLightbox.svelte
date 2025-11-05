<script lang="ts">
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Asset } from '$lib/types';

  interface Props {
    asset: Asset;
    onClose: () => void;
    onNext?: () => void;
    onPrev?: () => void;
  }

  let { asset, onClose, onNext, onPrev }: Props = $props();

  let zoom = $state(1);
  let showMetadata = $state(false);

  // Convert file path to Tauri-compatible URL
  const imageSrc = $derived(convertFileSrc(asset.path));

  function handleKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'Escape':
        onClose();
        break;
      case 'ArrowLeft':
        onPrev?.();
        break;
      case 'ArrowRight':
        onNext?.();
        break;
      case '+':
      case '=':
        zoom = Math.min(zoom + 0.5, 5);
        break;
      case '-':
        zoom = Math.max(zoom - 0.5, 0.5);
        break;
      case '0':
        zoom = 1;
        break;
      case 'i':
      case 'I':
        showMetadata = !showMetadata;
        break;
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
    return () => document.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class="fixed inset-0 bg-black/90 flex items-center justify-center z-[1000]" onclick={onClose}>
  <div class="relative w-[90vw] h-[90vh] flex flex-col" onclick={(e) => e.stopPropagation()}>
    <!-- Close button -->
    <button
      class="absolute top-4 right-4 w-12 h-12 bg-black/50 text-white border-none rounded-full text-3xl cursor-pointer z-10 hover:bg-black/70 transition-colors"
      onclick={onClose}
    >
      ×
    </button>

    <!-- Navigation -->
    {#if onPrev}
      <button
        class="absolute top-1/2 -translate-y-1/2 left-4 w-12 h-12 bg-black/50 text-white border-none rounded-full text-3xl cursor-pointer hover:bg-black/70 transition-colors"
        onclick={onPrev}
      >
        ‹
      </button>
    {/if}
    {#if onNext}
      <button
        class="absolute top-1/2 -translate-y-1/2 right-4 w-12 h-12 bg-black/50 text-white border-none rounded-full text-3xl cursor-pointer hover:bg-black/70 transition-colors"
        onclick={onNext}
      >
        ›
      </button>
    {/if}

    <!-- Image display -->
    <div class="flex-1 flex items-center justify-center overflow-auto">
      <img
        src={imageSrc}
        alt={asset.filename}
        style="transform: scale({zoom})"
        class="max-w-full max-h-full object-contain transition-transform duration-200"
      />
    </div>

    <!-- Controls -->
    <div class="flex justify-between items-center p-4 bg-black/80 text-white">
      <div>
        <p class="font-medium">{asset.filename}</p>
        {#if asset.width && asset.height}
          <p class="text-sm text-gray-300">{asset.width} × {asset.height} • {(asset.file_size / 1024).toFixed(0)} KB</p>
        {/if}
      </div>

      <div class="flex gap-2 items-center">
        <button class="px-3 py-1 bg-white/10 rounded hover:bg-white/20 transition-colors" onclick={() => zoom = Math.max(zoom - 0.5, 0.5)}>−</button>
        <span class="min-w-[4rem] text-center">{Math.round(zoom * 100)}%</span>
        <button class="px-3 py-1 bg-white/10 rounded hover:bg-white/20 transition-colors" onclick={() => zoom = Math.min(zoom + 0.5, 5)}>+</button>
        <button class="px-3 py-1 bg-white/10 rounded hover:bg-white/20 transition-colors" onclick={() => zoom = 1}>Reset</button>
        <button class="px-3 py-1 bg-white/10 rounded hover:bg-white/20 transition-colors" onclick={() => showMetadata = !showMetadata}>Info</button>
      </div>
    </div>

    <!-- Metadata panel -->
    {#if showMetadata}
      <div class="absolute top-16 right-4 w-[300px] p-4 bg-black/90 text-white rounded-lg">
        <h3 class="text-lg font-semibold mb-3">Image Details</h3>
        <dl class="space-y-2">
          <div>
            <dt class="text-sm text-gray-400">Filename:</dt>
            <dd class="text-sm">{asset.filename}</dd>
          </div>

          <div>
            <dt class="text-sm text-gray-400">Path:</dt>
            <dd class="text-sm break-all">{asset.path}</dd>
          </div>

          {#if asset.width && asset.height}
            <div>
              <dt class="text-sm text-gray-400">Dimensions:</dt>
              <dd class="text-sm">{asset.width} × {asset.height} px</dd>
            </div>
          {/if}

          <div>
            <dt class="text-sm text-gray-400">Format:</dt>
            <dd class="text-sm">{asset.format.toUpperCase()}</dd>
          </div>

          <div>
            <dt class="text-sm text-gray-400">File Size:</dt>
            <dd class="text-sm">{(asset.file_size / 1024).toFixed(2)} KB</dd>
          </div>
        </dl>
      </div>
    {/if}
  </div>
</div>
