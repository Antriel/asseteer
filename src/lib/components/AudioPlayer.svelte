<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
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
    /** Playback position 0–1, for callers that mirror the playhead elsewhere. */
    progress?: number;
    playing?: boolean;
    /** First line of the strip, above the scrubber (name, metadata, actions). */
    info?: Snippet;
    /** Right-hand controls, before the volume slider. */
    controls?: Snippet;
  }

  let {
    asset,
    isActive = false,
    autoPlay = false,
    restartKey = 0,
    onPlay,
    onPause,
    onEnded,
    progress = $bindable(0),
    playing = $bindable(false),
    info,
    controls,
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
  // Bumped per load; a zip blob that resolves after a newer asset was picked is dropped
  let loadToken = 0;

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
      const token = ++loadToken;
      (async () => {
        try {
          if (zipEntry) {
            const newBlobUrl = await loadAssetBlobUrl(assetId, `audio/${assetFormat}`);
            if (token !== loadToken) {
              URL.revokeObjectURL(newBlobUrl);
              return;
            }

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
          if (token !== loadToken) return;
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

  let scrubbing = $state(false);

  function seekToPointer(e: PointerEvent) {
    if (!audioElement || !duration) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    audioElement.currentTime = fraction * duration;
    currentTime = audioElement.currentTime;
  }

  function handleScrubStart(e: PointerEvent) {
    if (e.button !== 0) return;
    scrubbing = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    seekToPointer(e);
  }

  function handleScrubMove(e: PointerEvent) {
    if (scrubbing) seekToPointer(e);
  }

  function handleScrubEnd() {
    scrubbing = false;
  }

  // Keep the paused playhead in sync after seeks (the RAF loop only runs while playing)
  function handleTimeUpdate() {
    if (audioElement && !isPlaying) currentTime = audioElement.currentTime;
  }

  $effect(() => {
    progress = duration ? Math.min(1, currentTime / duration) : 0;
  });

  $effect(() => {
    playing = isPlaying;
  });

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

{#if audioSrc}
  <audio
    bind:this={audioElement}
    src={audioSrc}
    onloadedmetadata={handleLoadedMetadata}
    oncanplay={handleCanPlay}
    ontimeupdate={handleTimeUpdate}
    onended={handleEnded}
  ></audio>
{/if}

<div class="flex items-center gap-3 @xl:gap-4 min-w-0">
  <button
    class="w-9 h-9 flex items-center justify-center bg-accent text-white rounded-full flex-shrink-0 transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-40 disabled:cursor-default"
    onclick={togglePlay}
    disabled={!audioSrc}
    title={isPlaying ? 'Pause (Space)' : 'Play (Space)'}
    aria-label={isPlaying ? 'Pause' : 'Play'}
  >
    {#if isPlaying}
      <PauseIcon size="sm" />
    {:else}
      <PlayIcon size="sm" class="translate-x-px" />
    {/if}
  </button>

  <div class="flex-1 min-w-0 flex flex-col gap-1">
    {@render info?.()}

    <div class="flex items-center gap-2.5 h-4 text-[11px] tabular-nums text-tertiary">
      {#if showLoading}
        <span>Loading audio…</span>
      {:else if !loading && !audioSrc}
        <span class="text-error">Failed to load audio</span>
      {:else}
        <span class="w-14 flex-shrink-0" class:text-secondary={currentTime > 0}>
          {formatDuration(currentTime * 1000)}
        </span>
        <div
          class="group relative flex-1 h-4 flex items-center cursor-pointer touch-none"
          role="slider"
          tabindex="0"
          aria-valuemin={0}
          aria-valuemax={Math.round(duration)}
          aria-valuenow={Math.round(currentTime)}
          aria-label="Seek audio"
          onpointerdown={handleScrubStart}
          onpointermove={handleScrubMove}
          onpointerup={handleScrubEnd}
          onpointercancel={handleScrubEnd}
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
          <div
            class="w-full bg-track rounded-full overflow-hidden transition-[height] duration-100 {scrubbing
              ? 'h-1.5'
              : 'h-1 group-hover:h-1.5'}"
          >
            <div class="h-full bg-accent" style="width: {progress * 100}%"></div>
          </div>
          <div
            class="absolute top-1/2 w-3 h-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent shadow-sm transition-opacity duration-100 {scrubbing
              ? 'opacity-100'
              : 'opacity-0 group-hover:opacity-100'}"
            style="left: {progress * 100}%"
          ></div>
        </div>
        <span class="w-14 flex-shrink-0 text-right">{formatDuration(duration * 1000)}</span>
      {/if}
    </div>
  </div>

  {@render controls?.()}

  <div class="hidden @2xl:flex items-center gap-1.5 flex-shrink-0" title="Volume">
    <VolumeIcon size="sm" class="text-tertiary" />
    <input
      type="range"
      min="0"
      max="1"
      step="0.05"
      bind:value={volume}
      oninput={() => audioElement && (audioElement.volume = volume)}
      class="w-16 h-1 cursor-pointer accent-[var(--color-accent)]"
      aria-label="Volume"
    />
  </div>
</div>
