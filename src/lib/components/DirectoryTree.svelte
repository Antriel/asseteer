<script lang="ts">
  import { exploreState, type DirectoryNode } from '$lib/state/explore.svelte';
  import { ChevronIcon, FolderIcon } from '$lib/components/icons';
  import { openLocationInExplorer } from '$lib/actions/assetActions';
  import DirectoryTree from './DirectoryTree.svelte';

  interface Props {
    nodes: DirectoryNode[];
    depth?: number;
    onSelect: (node: DirectoryNode) => void;
  }

  let { nodes, depth = 0, onSelect }: Props = $props();

  let contextMenu = $state<{ x: number; y: number; node: DirectoryNode } | null>(null);

  function handleContextMenu(e: MouseEvent, node: DirectoryNode) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY, node };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function openInExplorer(node: DirectoryNode) {
    const folderBase = exploreState.folderPaths.get(node.location.folderId);
    if (!folderBase) return;
    await openLocationInExplorer(folderBase, node.location);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
{#if contextMenu}
  <div
    class="fixed inset-0 z-50"
    onclick={closeContextMenu}
    oncontextmenu={(e) => {
      e.preventDefault();
      closeContextMenu();
    }}
  >
    <div
      class="absolute bg-elevated border border-default rounded-lg shadow-lg py-1 min-w-[160px]"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
    >
      <button
        class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
        onclick={() => {
          const n = contextMenu!.node;
          closeContextMenu();
          openInExplorer(n);
        }}
      >
        <svg class="w-4 h-4 text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
          />
        </svg>
        Open in File Explorer
      </button>
    </div>
  </div>
{/if}

{#each nodes as node (node.key)}
  {@const isExpanded = exploreState.isExpanded(node.key)}
  {@const isSelected = exploreState.selectedKey === node.key}
  {@const children = exploreState.getChildren(node.key)}
  {@const hasChildren = node.childCount > 0}
  {@const isZip = node.location.type === 'zip'}

  <div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="flex items-center w-full gap-1 px-2 py-1 text-sm rounded hover:bg-tertiary transition-colors group {isSelected
        ? 'bg-accent-muted text-accent'
        : 'text-primary'}"
      style="padding-left: {depth * 16 + 8}px"
      data-tree-key={node.key}
      data-selected={isSelected ? 'true' : undefined}
      oncontextmenu={(e) => handleContextMenu(e, node)}
    >
      <!-- Chevron: only this toggles expand/collapse -->
      <button
        class="w-4 h-4 flex items-center justify-center flex-shrink-0 {hasChildren
          ? 'cursor-pointer hover:text-primary'
          : ''}"
        onclick={(e) => {
          e.stopPropagation();
          if (hasChildren) {
            exploreState.toggleExpanded(node.key, node.directoryId, node.location.folderId);
          }
        }}
        tabindex={hasChildren ? 0 : -1}
        aria-label={hasChildren ? (isExpanded ? 'Collapse' : 'Expand') : undefined}
      >
        {#if hasChildren}
          <ChevronIcon size="sm" direction={isExpanded ? 'down' : 'right'} class="text-secondary" />
        {/if}
      </button>

      <!-- Folder name: selects folder (loads assets), does NOT toggle expand -->
      <button
        class="flex items-center gap-1 min-w-0 cursor-pointer"
        onclick={() => onSelect(node)}
      >
        {#if isZip}
          <svg
            class="w-4 h-4 flex-shrink-0 {isSelected ? 'text-accent' : 'text-secondary'}"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"
            />
          </svg>
        {:else}
          <FolderIcon size="sm" class={isSelected ? 'text-accent' : 'text-secondary'} />
        {/if}
        <span class="whitespace-nowrap text-left" title={node.name}>{node.name}</span>
      </button>

      <span class="text-xs text-tertiary flex-shrink-0 opacity-0 group-hover:opacity-100"
        >{node.assetCount}</span
      >
    </div>

    {#if isExpanded && children.length > 0}
      <DirectoryTree nodes={children} depth={depth + 1} {onSelect} />
    {/if}
  </div>
{/each}
