<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { AudioIcon, ImageIcon } from '$lib/components/icons';

  type Tab = 'audio' | 'images';

  // Library totals run to hundreds of thousands, so they live in the tooltip, not the label
  const tabs: { tab: Tab; label: string; icon: typeof AudioIcon; noun: string }[] = [
    { tab: 'audio', label: 'Audio', icon: AudioIcon, noun: 'audio files' },
    { tab: 'images', label: 'Images', icon: ImageIcon, noun: 'images' },
  ];

  function switchTab(tab: Tab) {
    if (viewState.activeTab === tab) return;
    viewState.setActiveTab(tab);
    if (tab === 'images') {
      assetsState.setDurationFilter(null, null);
    }
    assetsState.loadAssets(tab === 'images' ? 'image' : 'audio');
  }
</script>

<div
  class="h-9 flex-shrink-0 flex items-center p-0.5 bg-primary border border-default rounded-md"
  role="radiogroup"
  aria-label="Asset type"
>
  {#each tabs as { tab, label, icon: Icon, noun } (tab)}
    {@const active = viewState.activeTab === tab}
    <button
      class="h-full flex items-center gap-1.5 px-2.5 text-sm font-medium rounded transition-colors {active
        ? 'bg-accent-light text-accent'
        : 'text-tertiary hover:text-primary'}"
      role="radio"
      aria-checked={active}
      aria-label={label}
      title="{label} — {viewState.assetCounts[tab].toLocaleString()} {noun} in the library"
      onclick={() => switchTab(tab)}
    >
      <Icon size="sm" />
      <span class="hidden @3xl:inline">{label}</span>
    </button>
  {/each}
</div>
