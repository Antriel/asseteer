<script lang="ts">
  import type { Asset } from '$lib/types';
  import AudioPlayer from './AudioPlayer.svelte';

  interface Props {
    assets: Asset[];
  }

  let { assets }: Props = $props();

  let currentlyPlaying = $state<number | null>(null);

  function formatDuration(ms: number): string {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
  }

  function formatFileSize(bytes: number): string {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }
</script>

<div class="flex flex-col gap-2 p-4">
  {#each assets as asset (asset.id)}
    <div
      class="flex items-center gap-4 p-4 bg-secondary border border-default rounded-lg transition-all hover:border-accent"
      class:!bg-accent-light={currentlyPlaying === asset.id}
      class:!border-accent={currentlyPlaying === asset.id}
    >
      <!-- Audio icon placeholder -->
      <div class="w-12 h-12 flex items-center justify-center bg-primary rounded-lg flex-shrink-0">
        <svg class="w-6 h-6 text-secondary" fill="currentColor" viewBox="0 0 20 20">
          <path d="M18 3a1 1 0 00-1.196-.98l-10 2A1 1 0 006 5v9.114A4.369 4.369 0 005 14c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V7.82l8-1.6v5.894A4.37 4.37 0 0015 12c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V3z" />
        </svg>
      </div>

      <!-- Audio metadata -->
      <div class="flex-1 min-w-0">
        <p class="font-semibold text-primary whitespace-nowrap overflow-hidden text-ellipsis">
          {asset.filename}
        </p>
        <div class="flex gap-4 mt-1 text-xs text-secondary">
          {#if asset.duration_ms}
            <span>{formatDuration(asset.duration_ms)}</span>
          {/if}
          {#if asset.sample_rate}
            <span>{asset.sample_rate / 1000} kHz</span>
          {/if}
          {#if asset.channels}
            <span>{asset.channels === 1 ? 'Mono' : 'Stereo'}</span>
          {/if}
          <span>{asset.format.toUpperCase()}</span>
          <span>{formatFileSize(asset.file_size)}</span>
        </div>
      </div>

      <!-- Inline player -->
      <div class="w-[300px] flex-shrink-0">
        <AudioPlayer
          asset={asset}
          isActive={currentlyPlaying === asset.id}
          onPlay={() => currentlyPlaying = asset.id}
          onPause={() => currentlyPlaying = null}
        />
      </div>
    </div>
  {/each}
</div>
