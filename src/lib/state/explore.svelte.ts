import { SvelteMap } from 'svelte/reactivity';
import type { Asset } from '$lib/types';
import { getDatabase } from '$lib/database/connection';
import { getDirectoryChildren, getAssetsInDirectory } from '$lib/database/queries';

export interface DirectoryNode {
  path: string;
  name: string;
  childCount: number;
  assetCount: number;
}

class ExploreState {
  // Currently selected directory path
  selectedPath = $state<string | null>(null);
  // Assets in the selected directory
  assets = $state<Asset[]>([]);
  isLoading = $state(false);
  // Expanded directory paths
  expandedPaths = $state(new SvelteMap<string, boolean>());
  // Cached children per directory path (null key = root level)
  childrenCache = $state(new SvelteMap<string, DirectoryNode[]>());
  // Root directories (top-level scan roots)
  roots = $state<DirectoryNode[]>([]);
  isLoadingRoots = $state(false);

  async loadRoots() {
    if (this.isLoadingRoots) return;
    this.isLoadingRoots = true;
    try {
      const db = await getDatabase();
      this.roots = await getDirectoryChildren(db, null);
    } catch (error) {
      console.error('[Explore] Failed to load roots:', error);
    } finally {
      this.isLoadingRoots = false;
    }
  }

  async loadChildren(parentPath: string) {
    if (this.childrenCache.has(parentPath)) return;
    try {
      const db = await getDatabase();
      const children = await getDirectoryChildren(db, parentPath);
      this.childrenCache.set(parentPath, children);
    } catch (error) {
      console.error('[Explore] Failed to load children for', parentPath, error);
    }
  }

  async toggleExpanded(path: string) {
    const isExpanded = this.expandedPaths.get(path);
    if (isExpanded) {
      this.expandedPaths.set(path, false);
    } else {
      this.expandedPaths.set(path, true);
      await this.loadChildren(path);
    }
  }

  async selectDirectory(path: string) {
    this.selectedPath = path;
    this.isLoading = true;
    try {
      const db = await getDatabase();
      this.assets = await getAssetsInDirectory(db, path);
    } catch (error) {
      console.error('[Explore] Failed to load assets for', path, error);
    } finally {
      this.isLoading = false;
    }
  }

  async navigateToAssetPath(assetPath: string) {
    // Expand all ancestor directories and select the directory containing this asset
    // assetPath is the directory path (assets.path column) with native separators
    const sep = assetPath.includes('\\') ? '\\' : '/';
    const parts = assetPath.split(sep);

    // Expand each ancestor
    let current = '';
    for (let i = 0; i < parts.length; i++) {
      current = i === 0 ? parts[i] : current + sep + parts[i];
      // On Windows, first part might be like "C:" - need to handle drive letters
      if (i === 0 && current.endsWith(':')) {
        continue; // Skip bare drive letter, wait for next part
      }
      if (!this.expandedPaths.get(current)) {
        this.expandedPaths.set(current, true);
        await this.loadChildren(current);
      }
    }

    // Select the full path
    await this.selectDirectory(assetPath);
  }

  getChildren(path: string): DirectoryNode[] {
    return this.childrenCache.get(path) ?? [];
  }

  isExpanded(path: string): boolean {
    return this.expandedPaths.get(path) ?? false;
  }

  clearCache() {
    this.childrenCache.clear();
    this.roots = [];
  }
}

export const exploreState = new ExploreState();
