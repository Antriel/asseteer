<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getDatabase } from '$lib/database/connection';
  import { getSearchExcludes, getDistinctRelPaths } from '$lib/database/queries';
  import { showToast } from '$lib/state/ui.svelte';
  import type { SearchExclude } from '$lib/types';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';
  import SearchIcon from '$lib/components/icons/SearchIcon.svelte';

  interface Props {
    folderId: number;
    folderPath: string;
  }

  let { folderId, folderPath }: Props = $props();

  /** A node in the search config tree */
  interface TreeNode {
    /** Display name (just the segment) */
    name: string;
    /** Cumulative path up to and including this segment */
    path: string;
    /** null for filesystem dirs, zip filename for zip-internal dirs */
    zipFile: string | null;
    /** Child nodes */
    children: TreeNode[];
    /** Whether this node is expanded in the UI */
    expanded: boolean;
    /** Whether this is a ZIP archive node (parent of zip-internal dirs) */
    isZipArchive: boolean;
  }

  let panelExpanded = $state(false);
  let loading = $state(false);
  let saving = $state(false);
  let loaded = $state(false);
  let tree = $state<TreeNode[]>([]);
  let excludedSet = $state(new Set<string>());
  let originalExcludedJson = $state('');
  let hasChanges = $state(false);

  /** Serialize the exclude set for change detection */
  function serializeExcludes(): string {
    return JSON.stringify([...excludedSet].sort());
  }

  $effect(() => {
    hasChanges = serializeExcludes() !== originalExcludedJson;
  });

  /** Create a key for the exclude set: "null:path" or "zipfile:path" */
  function excludeKey(zipFile: string | null, path: string): string {
    return (zipFile ?? '') + '\0' + path;
  }

  /** Build a tree from a flat list of paths */
  function buildTree(paths: string[], zipFile: string | null): TreeNode[] {
    const root: TreeNode[] = [];

    for (const fullPath of paths) {
      const segments = fullPath.split('/');
      let currentLevel = root;
      let cumulative = '';

      for (const segment of segments) {
        cumulative = cumulative ? cumulative + '/' + segment : segment;
        let existing = currentLevel.find((n) => n.name === segment && n.zipFile === zipFile);
        if (!existing) {
          existing = {
            name: segment,
            path: cumulative,
            zipFile,
            children: [],
            expanded: false,
            isZipArchive: false,
          };
          currentLevel.push(existing);
        }
        currentLevel = existing.children;
      }
    }

    return root;
  }

  /** Insert a ZIP archive node into the filesystem tree at the right location */
  function insertZipNode(fsTree: TreeNode[], relPath: string, zipFile: string, zipDirs: string[]) {
    // Find the parent filesystem node for this zip
    let parent = fsTree;
    if (relPath) {
      const segments = relPath.split('/');
      for (const seg of segments) {
        const node = parent.find((n) => n.name === seg && n.zipFile === null);
        if (node) {
          parent = node.children;
        } else {
          return; // parent path doesn't exist in tree, skip
        }
      }
    }

    // Build zip-internal tree
    const zipChildren = buildTree(zipDirs, zipFile);

    // Create the zip archive node
    const zipNode: TreeNode = {
      name: zipFile,
      path: zipFile,
      zipFile: null, // the archive itself is a filesystem entity
      children: zipChildren,
      expanded: false,
      isZipArchive: true,
    };

    parent.push(zipNode);
  }

  async function load() {
    loading = true;
    try {
      const db = await getDatabase();

      const [currentExcludes, relPaths, zipDirGroups] = await Promise.all([
        getSearchExcludes(db, folderId),
        getDistinctRelPaths(db, folderId),
        invoke<Array<{ rel_path: string; zip_file: string; dirs: string[] }>>(
          'get_zip_dir_trees',
          { folderId },
        ),
      ]);

      // Build the excludes set
      excludedSet = new Set(currentExcludes.map((e) => excludeKey(e.zip_file, e.excluded_path)));
      originalExcludedJson = serializeExcludes();

      // Build filesystem tree from rel_paths
      const fsTree = buildTree(relPaths, null);

      // Insert zip nodes into the tree (dirs already extracted by backend)
      for (const { rel_path, zip_file, dirs } of zipDirGroups) {
        insertZipNode(fsTree, rel_path, zip_file, dirs);
      }

      // Sort each level
      sortTree(fsTree);

      tree = fsTree;
      loaded = true;
    } catch (error) {
      showToast('Failed to load search config: ' + error, 'error');
    } finally {
      loading = false;
    }
  }

  function sortTree(nodes: TreeNode[]) {
    nodes.sort((a, b) => {
      // ZIP archives after directories
      if (a.isZipArchive !== b.isZipArchive) return a.isZipArchive ? 1 : -1;
      return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
    });
    for (const node of nodes) {
      if (node.children.length > 0) sortTree(node.children);
    }
  }

  function togglePanel() {
    panelExpanded = !panelExpanded;
    if (panelExpanded && !loaded && !loading) {
      load();
    }
  }

  function isExcluded(node: TreeNode): boolean {
    if (node.isZipArchive) return false; // zip archives themselves aren't excludable
    const key = excludeKey(node.zipFile, node.path);
    return excludedSet.has(key);
  }

  function toggleExclude(node: TreeNode) {
    if (node.isZipArchive) return;
    const key = excludeKey(node.zipFile, node.path);
    const newSet = new Set(excludedSet);
    if (newSet.has(key)) {
      newSet.delete(key);
    } else {
      newSet.add(key);
    }
    excludedSet = newSet;
  }

  function toggleExpand(node: TreeNode) {
    node.expanded = !node.expanded;
  }

  async function save() {
    saving = true;
    try {
      // Convert excludedSet back to SearchExclude[]
      const excludes: SearchExclude[] = [...excludedSet].map((key) => {
        const idx = key.indexOf('\0');
        const zipPart = key.substring(0, idx);
        const path = key.substring(idx + 1);
        return {
          zip_file: zipPart === '' ? null : zipPart,
          excluded_path: path,
        };
      });

      await invoke('update_search_excludes', { folderId, excludes });
      originalExcludedJson = serializeExcludes();
      showToast('Search settings saved and re-indexed', 'success');
    } catch (error) {
      showToast('Failed to save search config: ' + error, 'error');
    } finally {
      saving = false;
    }
  }
</script>

{#snippet treeNode(node: TreeNode, depth: number)}
  {@const excluded = isExcluded(node)}
  {@const hasChildren = node.children.length > 0}
  <div style="padding-left: {depth * 16}px">
    <div
      class="flex items-center gap-1 py-0.5 group text-xs"
      class:text-tertiary={excluded}
      class:text-primary={!excluded}
    >
      <!-- Expand/collapse toggle -->
      {#if hasChildren}
        <button
          onclick={() => toggleExpand(node)}
          class="p-0.5 rounded hover:bg-tertiary/50 transition-colors flex-shrink-0"
        >
          <ChevronIcon size="sm" direction={node.expanded ? 'down' : 'right'} />
        </button>
      {:else}
        <span class="w-5 flex-shrink-0"></span>
      {/if}

      <!-- Checkbox (not for ZIP archive nodes) -->
      {#if !node.isZipArchive}
        <button
          onclick={() => toggleExclude(node)}
          class="w-4 h-4 flex-shrink-0 rounded border flex items-center justify-center transition-colors {excluded
            ? 'border-tertiary bg-tertiary/20'
            : 'border-accent bg-accent/10'}"
          title={excluded ? 'Include in search' : 'Exclude from search'}
        >
          {#if !excluded}
            <svg
              class="w-2.5 h-2.5 text-accent"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="3"
                d="M5 13l4 4L19 7"
              />
            </svg>
          {/if}
        </button>
      {:else}
        <span class="w-4 flex-shrink-0"></span>
      {/if}

      <!-- Name -->
      <button
        onclick={() => (hasChildren ? toggleExpand(node) : toggleExclude(node))}
        class="truncate text-left hover:text-accent transition-colors {node.isZipArchive
          ? 'font-medium'
          : ''}"
        class:line-through={excluded}
      >
        {#if node.isZipArchive}
          <span class="text-warning/70">{node.name}</span>
        {:else}
          {node.name}
        {/if}
      </button>

      {#if excluded}
        <span class="text-[10px] text-tertiary/60 ml-auto flex-shrink-0">excluded</span>
      {/if}
    </div>
  </div>

  {#if hasChildren && node.expanded}
    {#each node.children as child}
      {@render treeNode(child, depth + 1)}
    {/each}
  {/if}
{/snippet}

<div class="border-t border-default">
  <button
    onclick={togglePanel}
    class="w-full flex items-center gap-2 px-4 py-2.5 text-xs text-secondary hover:text-primary hover:bg-tertiary/50 transition-colors"
  >
    <ChevronIcon size="sm" direction={panelExpanded ? 'down' : 'right'} />
    <SearchIcon size="sm" />
    <span>Search indexing</span>
  </button>

  {#if panelExpanded}
    <div class="px-4 pb-4">
      {#if loading}
        <div class="flex items-center gap-2 py-3">
          <Spinner size="sm" />
          <span class="text-xs text-tertiary">Loading directory tree...</span>
        </div>
      {:else}
        <p class="text-xs text-tertiary mb-2">
          Uncheck directories to exclude them from search. Checked segments are indexed.
        </p>

        {#if tree.length === 0}
          <p class="text-xs text-tertiary italic mb-3">
            No directory structure found (all assets are at the root level).
          </p>
        {:else}
          <div
            class="mb-3 max-h-64 overflow-y-auto rounded border border-default bg-secondary/30 py-1 px-1"
          >
            {#each tree as node}
              {@render treeNode(node, 0)}
            {/each}
          </div>
        {/if}

        {#if hasChanges}
          <div class="flex justify-end">
            <button
              onclick={save}
              disabled={saving}
              class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-accent hover:bg-accent/90 rounded-lg transition-colors disabled:opacity-50"
            >
              {#if saving}
                <Spinner size="sm" />
                Re-indexing...
              {:else}
                Save & re-index
              {/if}
            </button>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>
