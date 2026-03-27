import type { Asset } from '$lib/types';

type AssetViewMode = 'images' | 'audio';
type LayoutMode = 'grid' | 'table';
type ThumbnailSize = 'small' | 'medium' | 'large';

const VIEW_STORAGE_KEY = 'asseteer-view';

interface PersistedViewState {
  sidebarCollapsed?: boolean;
  folderSidebarOpen?: boolean;
  assetCounts?: { images: number; audio: number };
}

function loadViewState(): PersistedViewState {
  try {
    const raw = localStorage.getItem(VIEW_STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch {}
  return {};
}

function saveViewState(data: PersistedViewState) {
  localStorage.setItem(VIEW_STORAGE_KEY, JSON.stringify(data));
}

class ViewState {
  activeTab = $state<AssetViewMode>('audio');
  layoutMode = $state<LayoutMode>('grid');
  thumbnailSize = $state<ThumbnailSize>('medium');
  folderSidebarOpen = $state(false);
  sidebarCollapsed = $state(false);
  folderPanelWidth = $state(280);

  // Cached asset counts (persisted to avoid 0-blink on load)
  assetCounts = $state({ images: 0, audio: 0 });

  constructor() {
    const stored = loadViewState();
    if (stored.sidebarCollapsed !== undefined) {
      this.sidebarCollapsed = stored.sidebarCollapsed;
    }
    if (stored.folderSidebarOpen !== undefined) {
      this.folderSidebarOpen = stored.folderSidebarOpen;
    }
    if (stored.assetCounts) {
      this.assetCounts = stored.assetCounts;
    }
  }

  // Lightbox state
  lightboxAsset = $state<Asset | null>(null);

  toggleFolderSidebar() {
    this.folderSidebarOpen = !this.folderSidebarOpen;
    this.#savePersistedState();
  }

  openFolderSidebar() {
    this.folderSidebarOpen = true;
    this.#savePersistedState();
  }

  toggleSidebarCollapsed() {
    this.sidebarCollapsed = !this.sidebarCollapsed;
    this.#savePersistedState();
  }

  setAssetCounts(counts: { images: number; audio: number }) {
    this.assetCounts = counts;
    this.#savePersistedState();
  }

  #savePersistedState() {
    saveViewState({
      sidebarCollapsed: this.sidebarCollapsed,
      folderSidebarOpen: this.folderSidebarOpen,
      assetCounts: this.assetCounts,
    });
  }

  setActiveTab(tab: AssetViewMode) {
    this.activeTab = tab;
    // Reset layout mode based on tab
    this.layoutMode = tab === 'images' ? 'grid' : 'table';
  }

  toggleLayoutMode() {
    this.layoutMode = this.layoutMode === 'grid' ? 'table' : 'grid';
  }

  setThumbnailSize(size: ThumbnailSize) {
    this.thumbnailSize = size;
  }

  openLightbox(asset: Asset) {
    this.lightboxAsset = asset;
  }

  closeLightbox() {
    this.lightboxAsset = null;
  }

  nextImage(assets: Asset[]) {
    if (!this.lightboxAsset) return;
    const currentIndex = assets.findIndex((a) => a.id === this.lightboxAsset!.id);
    if (currentIndex !== -1 && currentIndex < assets.length - 1) {
      this.lightboxAsset = assets[currentIndex + 1];
    }
  }

  prevImage(assets: Asset[]) {
    if (!this.lightboxAsset) return;
    const currentIndex = assets.findIndex((a) => a.id === this.lightboxAsset!.id);
    if (currentIndex > 0) {
      this.lightboxAsset = assets[currentIndex - 1];
    }
  }
}

export const viewState = new ViewState();
