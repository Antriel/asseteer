<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import Badge from './Badge.svelte';

  function switchTab(tab: 'images' | 'audio') {
    viewState.setActiveTab(tab);
    if (tab === 'images') {
      assetsState.setDurationFilter(null, null);
    }
    const assetType = tab === 'images' ? 'image' : 'audio';
    assetsState.loadAssets(assetType);
  }
</script>

<div class="flex items-center gap-1 border-b border-default bg-secondary px-4">
  <!-- Asset type tabs -->
  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab ===
    'audio'
      ? 'text-accent border-accent'
      : 'text-secondary border-transparent'}"
    onclick={() => switchTab('audio')}
  >
    Audio
    <Badge variant="count">{viewState.assetCounts.audio}</Badge>
  </button>

  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab ===
    'images'
      ? 'text-accent border-accent'
      : 'text-secondary border-transparent'}"
    onclick={() => switchTab('images')}
  >
    Images
    <Badge variant="count">{viewState.assetCounts.images}</Badge>
  </button>
</div>
