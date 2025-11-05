<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';

  interface Props {
    imageCount: number;
    audioCount: number;
  }

  let { imageCount, audioCount }: Props = $props();

  function switchTab(tab: 'images' | 'audio') {
    viewState.setActiveTab(tab);
    assetsState.loadAssets(tab === 'images' ? 'image' : 'audio');
  }
</script>

<div class="flex items-center gap-1 border-b border-default bg-secondary px-4">
  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 border-transparent font-medium text-secondary transition-all hover:text-primary"
    class:!text-accent={viewState.activeTab === 'images'}
    class:!border-accent={viewState.activeTab === 'images'}
    onclick={() => switchTab('images')}
  >
    Images
    <span class="text-xs px-2 py-0.5 bg-tertiary rounded-full">{imageCount}</span>
  </button>

  <button
    class="flex items-center gap-2 px-4 py-3 border-b-2 border-transparent font-medium text-secondary transition-all hover:text-primary"
    class:!text-accent={viewState.activeTab === 'audio'}
    class:!border-accent={viewState.activeTab === 'audio'}
    onclick={() => switchTab('audio')}
  >
    Audio
    <span class="text-xs px-2 py-0.5 bg-tertiary rounded-full">{audioCount}</span>
  </button>
</div>
