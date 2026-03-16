import type { Asset } from '$lib/types';

type AssetViewMode = 'images' | 'audio';
type LayoutMode = 'grid' | 'table';
type ThumbnailSize = 'small' | 'medium' | 'large';
export type LibraryView = 'search' | 'explore';

class ViewState {
  activeTab = $state<AssetViewMode>('images');
  layoutMode = $state<LayoutMode>('grid');
  thumbnailSize = $state<ThumbnailSize>('medium');
  libraryView = $state<LibraryView>('search');

  // Lightbox state
  lightboxAsset = $state<Asset | null>(null);

  setLibraryView(view: LibraryView) {
    this.libraryView = view;
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
