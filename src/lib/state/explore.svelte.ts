import { SvelteMap } from 'svelte/reactivity';
import { getDatabase } from '$lib/database/connection';
import { getDirectoryChildren, getZipDirectoryChildren, ZIP_SEP } from '$lib/database/queries';

export type { DirectoryNode } from '$lib/database/queries';

class ExploreState {
  // Currently selected directory path
  selectedPath = $state<string | null>(null);
  // Expanded directory paths
  expandedPaths = $state(new SvelteMap<string, boolean>());
  // Cached children per directory path (null key = root level)
  childrenCache = $state(new SvelteMap<string, import('$lib/database/queries').DirectoryNode[]>());
  // Root directories (top-level scan roots)
  roots = $state<import('$lib/database/queries').DirectoryNode[]>([]);
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

  async loadChildren(path: string) {
    if (this.childrenCache.has(path)) return;
    try {
      const db = await getDatabase();
      const sepIdx = path.indexOf(ZIP_SEP);
      let children: import('$lib/database/queries').DirectoryNode[];
      if (sepIdx !== -1) {
        // ZIP-internal path: browse inside the zip
        const zipPath = path.substring(0, sepIdx);
        const prefix = path.substring(sepIdx + ZIP_SEP.length);
        children = await getZipDirectoryChildren(db, zipPath, prefix);
      } else if (path.toLowerCase().endsWith('.zip')) {
        // ZIP file node: browse its root
        children = await getZipDirectoryChildren(db, path, '');
      } else {
        // Regular filesystem directory
        children = await getDirectoryChildren(db, path);
      }
      this.childrenCache.set(path, children);
    } catch (error) {
      console.error('[Explore] Failed to load children for', path, error);
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

  async navigateToAssetPath(assetPath: string) {
    // Expand all ancestor directories and select the directory containing this asset
    const sep = assetPath.includes('\\') ? '\\' : '/';
    const parts = assetPath.split(sep);

    // Expand each ancestor
    let current = '';
    for (let i = 0; i < parts.length; i++) {
      current = i === 0 ? parts[i] : current + sep + parts[i];
      if (i === 0 && current.endsWith(':')) {
        continue;
      }
      if (!this.expandedPaths.get(current)) {
        this.expandedPaths.set(current, true);
        await this.loadChildren(current);
      }
    }

    // Select the full path
    this.selectedPath = assetPath;
  }

  getChildren(path: string): import('$lib/database/queries').DirectoryNode[] {
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
