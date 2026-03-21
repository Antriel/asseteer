<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Asset } from '$lib/types';
  import { FolderIcon } from '$lib/components/icons';

  interface Props {
    x: number;
    y: number;
    asset: Asset;
    onclose: () => void;
    onShowInFolder: (asset: Asset) => void;
    onOpenDirectory: (asset: Asset) => void;
    extraItems?: Snippet;
  }

  let { x, y, asset, onclose, onShowInFolder, onOpenDirectory, extraItems }: Props = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-50"
  onclick={onclose}
  oncontextmenu={(e) => { e.preventDefault(); onclose(); }}
>
  <div
    class="absolute bg-elevated border border-default rounded-lg shadow-lg py-1 min-w-[180px]"
    style="left: {x}px; top: {y}px;"
  >
    {#if extraItems}
      {@render extraItems()}
    {/if}
    <button
      class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
      onclick={() => { onShowInFolder(asset); onclose(); }}
    >
      <FolderIcon size="sm" class="text-secondary" />
      Show in Folder
    </button>
    <button
      class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
      onclick={() => { onOpenDirectory(asset); onclose(); }}
    >
      <svg class="w-4 h-4 text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
      </svg>
      Open in File Explorer
    </button>
  </div>
</div>
