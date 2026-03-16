<script lang="ts">
  import { exploreState, type DirectoryNode } from '$lib/state/explore.svelte';
  import { ChevronIcon, FolderIcon } from '$lib/components/icons';
  import DirectoryTree from './DirectoryTree.svelte';

  interface Props {
    nodes: DirectoryNode[];
    depth?: number;
  }

  let { nodes, depth = 0 }: Props = $props();
</script>

{#each nodes as node (node.path)}
  {@const isExpanded = exploreState.isExpanded(node.path)}
  {@const isSelected = exploreState.selectedPath === node.path}
  {@const children = exploreState.getChildren(node.path)}
  {@const hasChildren = node.childCount > 0}

  <div>
    <button
      class="flex items-center w-full gap-1 px-2 py-1 text-sm rounded hover:bg-tertiary transition-colors group {isSelected ? 'bg-accent-muted text-accent' : 'text-primary'}"
      style="padding-left: {depth * 16 + 8}px"
      onclick={() => {
        exploreState.selectDirectory(node.path);
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
      <FolderIcon size="sm" class={isSelected ? 'text-accent' : 'text-secondary'} />
      <span class="truncate flex-1 text-left">{node.name}</span>
      <span class="text-xs text-tertiary flex-shrink-0 opacity-0 group-hover:opacity-100">{node.assetCount}</span>
    </button>

    {#if isExpanded && children.length > 0}
      <DirectoryTree nodes={children} depth={depth + 1} />
    {/if}
  </div>
{/each}
