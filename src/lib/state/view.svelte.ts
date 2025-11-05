import type { Asset } from '$lib/types';

type AssetViewMode = 'images' | 'audio';
type LayoutMode = 'grid' | 'table';
type ThumbnailSize = 'small' | 'medium' | 'large';

class ViewState {
  activeTab = $state<AssetViewMode>('images');
  layoutMode = $state<LayoutMode>('grid');
  thumbnailSize = $state<ThumbnailSize>('medium');

  // Lightbox state
  lightboxAsset = $state<Asset | null>(null);
  lightboxIndex = $state(0);

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

  openLightbox(asset: Asset, index: number) {
    this.lightboxAsset = asset;
    this.lightboxIndex = index;
  }

  closeLightbox() {
    this.lightboxAsset = null;
  }

  nextImage(assets: Asset[]) {
    if (this.lightboxIndex < assets.length - 1) {
      this.lightboxIndex++;
      this.lightboxAsset = assets[this.lightboxIndex];
    }
  }

  prevImage(assets: Asset[]) {
    if (this.lightboxIndex > 0) {
      this.lightboxIndex--;
      this.lightboxAsset = assets[this.lightboxIndex];
    }
  }
}

export const viewState = new ViewState();
