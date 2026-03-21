<script lang="ts">
  import { untrack } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { loadAssetBlobUrl } from '$lib/utils/assetBlob';
  import { type Asset, getAssetFilePath } from '$lib/types';
  import { PlayIcon, PauseIcon, VolumeIcon } from './icons';
  import { formatDuration } from '$lib/utils/format';

  interface Props {
    asset: Asset;
    isActive?: boolean;
    autoPlay?: boolean;
    restartKey?: number;
    onPlay?: () => void;
    onPause?: () => void;
    onEnded?: () => void;
  }

  let {
    asset,
    isActive = false,
    autoPlay = false,
    restartKey = 0,
    onPlay,
    onPause,
    onEnded,
  }: Props = $props();

  // Exported function to seek by percentage delta (e.g., 0.1 for +10%, -0.1 for -10%)
  // Returns { playing: boolean, stopped: boolean } indicating state after seek
  export function seekByPercent(delta: number): { playing: boolean; stopped: boolean } {
    if (!audioElement || !duration) return { playing: false, stopped: false };

    const newTime = currentTime + delta * duration;

    if (newTime >= duration) {
      // Seeking past end - stop playback
      audioElement.currentTime = duration;
      audioElement.pause();
      isPlaying = false;
      onPause?.();
      onEnded?.();
      return { playing: false, stopped: true };
    } else if (newTime <= 0) {
      // Seeking before start - clamp to 0 and keep playing if was playing
      audioElement.currentTime = 0;
      return { playing: isPlaying, stopped: false };
    } else {
      audioElement.currentTime = newTime;
      return { playing: isPlaying, stopped: false };
    }
  }

  // Exported function to toggle play/pause from parent
  export function toggle(): void {
    togglePlay();
  }

  // Exported getter for current playing state
  export function getIsPlaying(): boolean {
    return isPlaying;
  }

  let audioElement = $state<HTMLAudioElement>();
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(1);
  let audioSrc = $state<string>('');
  let rafId: number | null = null;
  let blobUrl = $state<string | null>(null);
  let loading = $state(true);
  let showLoading = $state(false);
  let loadingTimer: ReturnType<typeof setTimeout> | null = null;
  let shouldAutoPlay = $state(false);

  // Load audio when asset changes - track only asset properties
  $effect(() => {
    // Track the asset properties (this is what triggers the effect)
    const assetId = asset.id;
    const zipEntry = asset.zip_entry;
    const assetPath = getAssetFilePath(asset);
    const assetFormat = asset.format;

    // Use untrack to prevent state updates from re-triggering the effect
    untrack(() => {
      // Stop current playback immediately
      if (audioElement) {
        audioElement.pause();
        audioElement.currentTime = 0;
      }
      isPlaying = false;
      currentTime = 0;
      duration = 0;

      // Clean up previous blob URL if exists
      if (blobUrl) {
        URL.revokeObjectURL(blobUrl);
        blobUrl = null;
      }

      loading = true;
      showLoading = false;
      shouldAutoPlay = false; // Reset - will be set after new src is loaded
      if (loadingTimer) clearTimeout(loadingTimer);
      loadingTimer = setTimeout(() => {
        if (loading) showLoading = true;
      }, 100);

      // Load the new asset
      (async () => {
        try {
          if (zipEntry) {
            const newBlobUrl = await loadAssetBlobUrl(assetId, `audio/${assetFormat}`);

            untrack(() => {
              blobUrl = newBlobUrl;
              audioSrc = newBlobUrl;
              loading = false;
              showLoading = false;
              if (loadingTimer) clearTimeout(loadingTimer);
              shouldAutoPlay = autoPlay;
            });
          } else {
            // Regular file - use convertFileSrc
            const src = convertFileSrc(assetPath);

            untrack(() => {
              audioSrc = src;
              loading = false;
              showLoading = false;
              if (loadingTimer) clearTimeout(loadingTimer);
              shouldAutoPlay = autoPlay;
            });
          }
        } catch (error) {
          console.error('Failed to load audio:', error);
          untrack(() => {
            audioSrc = '';
            loading = false;
            showLoading = false;
            if (loadingTimer) clearTimeout(loadingTimer);
            shouldAutoPlay = false;
          });
        }
      })();
    });
  });

  // Cleanup on unmount
  $effect(() => {
    return () => {
      untrack(() => {
        if (blobUrl) {
          URL.revokeObjectURL(blobUrl);
        }
        if (loadingTimer) {
          clearTimeout(loadingTimer);
        }
        if (rafId) {
          cancelAnimationFrame(rafId);
        }
      });
    };
  });

  async function togglePlay() {
    if (!audioElement) return;
    if (isPlaying) {
      audioElement.pause();
      isPlaying = false;
      onPause?.();
    } else {
      try {
        await audioElement.play();
        isPlaying = true;
        onPlay?.();
      } catch (error: any) {
        // Ignore AbortError - it's expected when source changes rapidly
        if (error.name !== 'AbortError') {
          console.error('Playback failed:', error);
        }
        isPlaying = false;
      }
    }
  }

  // RAF-based time updates for smooth progress bar (only runs while playing)
  function updateTime() {
    if (audioElement && isPlaying) {
      currentTime = audioElement.currentTime;
      rafId = requestAnimationFrame(updateTime);
    }
  }

  // Start/stop RAF loop based on playing state
  $effect(() => {
    if (isPlaying) {
      rafId = requestAnimationFrame(updateTime);
    } else if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
  });

  function handleLoadedMetadata() {
    if (!audioElement) return;
    duration = audioElement.duration;
    audioElement.volume = volume;
  }

  function handleEnded() {
    isPlaying = false;
    onPause?.();
    onEnded?.();
  }

  function seek(e: MouseEvent) {
    if (!audioElement) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = x / rect.width;
    audioElement.currentTime = percentage * duration;
  }

  // Pause if another player becomes active
  $effect(() => {
    if (!isActive && isPlaying) {
      audioElement?.pause();
      isPlaying = false;
    }
  });

  // Restart playback when restartKey changes (skip initial value of 0)
  $effect(() => {
    if (restartKey > 0 && audioElement && audioSrc) {
      audioElement.currentTime = 0;
      audioElement
        .play()
        .then(() => {
          isPlaying = true;
          onPlay?.();
        })
        .catch((error) => {
          if (error.name !== 'AbortError') {
            console.error('Restart play failed:', error);
          }
        });
    }
  });

  // Handle canplay event - this is when the audio is actually ready to play
  function handleCanPlay() {
    if (shouldAutoPlay && audioElement) {
      shouldAutoPlay = false;
      audioElement
        .play()
        .then(() => {
          isPlaying = true;
          onPlay?.();
        })
        .catch((error) => {
          // Ignore AbortError - it's expected when source changes rapidly
          if (error.name !== 'AbortError') {
            console.error('Auto-play failed:', error);
          }
          isPlaying = false;
        });
    }
  }
</script>

<div class="flex items-center gap-3">
  {#if showLoading}
    <div class="h-8 flex items-center text-sm text-secondary">Loading audio...</div>
  {:else if !loading && !audioSrc}
    <div class="h-8 flex items-center text-sm text-red-500">Failed to load audio</div>
  {:else if audioSrc}
    <audio
      bind:this={audioElement}
      src={audioSrc}
      onloadedmetadata={handleLoadedMetadata}
      oncanplay={handleCanPlay}
      onended={handleEnded}
    ></audio>

    <!-- Play/Pause button -->
    <button
      class="w-8 h-8 flex items-center justify-center bg-accent text-white border-none rounded-full cursor-pointer hover:opacity-90 transition-opacity flex-shrink-0"
      onclick={togglePlay}
    >
      {#if isPlaying}
        <PauseIcon size="sm" circled />
      {:else}
        <PlayIcon size="sm" circled />
      {/if}
    </button>

    <!-- Progress bar -->
    <div
      class="flex-1 cursor-pointer"
      role="slider"
      tabindex="0"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(currentTime)}
      aria-label="Seek audio"
      onclick={seek}
      onkeydown={(e) => {
        if (!audioElement) return;
        if (e.key === 'ArrowRight') {
          e.preventDefault();
          audioElement.currentTime = Math.min(duration, currentTime + duration * 0.05);
        } else if (e.key === 'ArrowLeft') {
          e.preventDefault();
          audioElement.currentTime = Math.max(0, currentTime - duration * 0.05);
        }
      }}
    >
      <div class="h-1 bg-default rounded-sm overflow-hidden">
        <div
          class="h-full bg-accent transition-[width] duration-40"
          style="width: {duration ? (currentTime / duration) * 100 : 0}%"
        ></div>
      </div>
      <div class="flex justify-between mt-1 text-[0.625rem] text-secondary">
        <span>{formatDuration(currentTime * 1000)}</span>
        <span>{formatDuration(duration * 1000)}</span>
      </div>
    </div>

    <!-- Volume control -->
    <div class="flex items-center gap-2 flex-shrink-0">
      <VolumeIcon size="sm" class="text-secondary" />
      <input
        type="range"
        min="0"
        max="1"
        step="0.1"
        bind:value={volume}
        oninput={() => audioElement && (audioElement.volume = volume)}
        class="w-[60px]"
      />
    </div>
  {/if}
</div>
