<script lang="ts">
  import { exploreState, type DirectoryNode } from '$lib/state/explore.svelte';
  import { ChevronIcon, FolderIcon } from '$lib/components/icons';
  import DirectoryTree from './DirectoryTree.svelte';

  interface Props {
    nodes: DirectoryNode[];
    depth?: number;
    onSelect: (node: DirectoryNode) => void;
  }

  let { nodes, depth = 0, onSelect }: Props = $props();
</script>

{#each nodes as node (node.path)}
  {@const isExpanded = exploreState.isExpanded(node.path)}
  {@const isSelected = exploreState.selectedPath === node.path}
  {@const children = exploreState.getChildren(node.path)}
  {@const hasChildren = node.childCount > 0}
  {@const isZip = node.zipPrefix !== undefined}

  <div>
    <button
      class="flex items-center w-full gap-1 px-2 py-1 text-sm rounded hover:bg-tertiary transition-colors group {isSelected ? 'bg-accent-muted text-accent' : 'text-primary'}"
      style="padding-left: {depth * 16 + 8}px"
      onclick={() => {
        onSelect(node);
        if (hasChildren) {
          exploreState.toggleExpanded(node.path);
        }
      }}
    >
      <span class="w-4 h-4 flex items-center justify-center flex-shrink-0">
        {#if hasChildren}
          <ChevronIcon size="sm" direction={isExpanded ? 'down' : 'right'} class="text-secondary" />
        {/if}
      </span>
      {#if isZip}
        <svg class="w-4 h-4 flex-shrink-0 {isSelected ? 'text-accent' : 'text-secondary'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" />
        </svg>
      {:else}
        <FolderIcon size="sm" class={isSelected ? 'text-accent' : 'text-secondary'} />
      {/if}
      <span class="truncate flex-1 text-left">{node.name}</span>
      <span class="text-xs text-tertiary flex-shrink-0 opacity-0 group-hover:opacity-100">{node.assetCount}</span>
    </button>

    {#if isExpanded && children.length > 0}
      <DirectoryTree nodes={children} depth={depth + 1} {onSelect} />
    {/if}
  </div>
{/each}
