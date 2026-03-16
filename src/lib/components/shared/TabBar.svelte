<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import Badge from './Badge.svelte';

  interface Props {
    imageCount: number;
    audioCount: number;
  }

  let { imageCount, audioCount }: Props = $props();

  function switchTab(tab: 'images' | 'audio') {
    viewState.setActiveTab(tab);
    if (tab === 'images') {
      assetsState.setDurationFilter(null, null);
    }
    assetsState.loadAssets(tab === 'images' ? 'image' : 'audio');
  }
</script>

<div class="flex items-center gap-1 border-b border-default bg-secondary px-4">
  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab === 'images' ? 'text-accent border-accent' : 'text-secondary border-transparent'}"
    onclick={() => switchTab('images')}
  >
    Images
    <Badge variant="count">{imageCount}</Badge>
  </button>

  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 font-medium transition-all hover:text-primary {viewState.activeTab === 'audio' ? 'text-accent border-accent' : 'text-secondary border-transparent'}"
    onclick={() => switchTab('audio')}
  >
    Audio
    <Badge variant="count">{audioCount}</Badge>
  </button>
</div>
