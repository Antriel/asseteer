import type { Asset, FolderLocation } from '$lib/types';
import { getAssetDirectoryPath } from '$lib/types';
import { openPath } from '@tauri-apps/plugin-opener';
import { viewState } from '$lib/state/view.svelte';
import { assetsState } from '$lib/state/assets.svelte';
import { exploreState } from '$lib/state/explore.svelte';

export async function showInFolder(asset: Asset, assetType: 'image' | 'audio') {
  viewState.openFolderSidebar();
  await exploreState.loadRoots();
  await exploreState.navigateToAsset(asset);
  let location: FolderLocation;
  if (asset.zip_file) {
    const zipParts = (asset.zip_entry ?? '').split('/').filter(Boolean);
    const zipDirParts = zipParts.slice(0, -1);
    const zipPrefix = zipDirParts.length > 0 ? zipDirParts.join('/') + '/' : '';
    location = { type: 'zip', folderId: asset.folder_id, relPath: asset.rel_path, zipFile: asset.zip_file, zipPrefix };
  } else {
    location = { type: 'folder', folderId: asset.folder_id, relPath: asset.rel_path };
  }
  assetsState.setFolderFilter(location, assetType);
}

export async function openDirectory(asset: Asset) {
  try {
    let dirPath: string;
    if (asset.zip_file) {
      dirPath = asset.rel_path
        ? asset.folder_path + '\\' + asset.rel_path.replace(/\//g, '\\')
        : asset.folder_path;
    } else {
      dirPath = getAssetDirectoryPath(asset).replace(/\//g, '\\');
    }
    await openPath(dirPath);
  } catch (error) {
    console.error('Failed to open directory:', error);
  }
}
