<script lang="ts">
  import { untrack } from 'svelte';
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import type { Asset } from '$lib/types';
  import { PlayIcon, PauseIcon } from './icons';

  interface Props {
    asset: Asset;
    isActive?: boolean;
    autoPlay?: boolean;
    restartKey?: number;
    onPlay?: () => void;
    onPause?: () => void;
    onEnded?: () => void;
  }

  let { asset, isActive = false, autoPlay = false, restartKey = 0, onPlay, onPause, onEnded }: Props = $props();

  // Exported function to seek by percentage delta (e.g., 0.1 for +10%, -0.1 for -10%)
  // Returns { playing: boolean, stopped: boolean } indicating state after seek
  export function seekByPercent(delta: number): { playing: boolean; stopped: boolean } {
    if (!audioElement || !duration) return { playing: false, stopped: false };

    const newTime = currentTime + (delta * duration);

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
    const assetPath = asset.path;
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
            // Asset is inside a zip - need to extract it
            const bytes = await invoke<number[]>('get_asset_bytes', { assetId });
            const blob = new Blob([new Uint8Array(bytes)], { type: `audio/${assetFormat}` });
            const newBlobUrl = URL.createObjectURL(blob);

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

  function handleTimeUpdate() {
    if (!audioElement) return;
    currentTime = audioElement.currentTime;
  }

  function handleLoadedMetadata() {
    if (!audioElement) return;
    duration = audioElement.duration;
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

  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    if (seconds < 10) {
      const secs = seconds % 60;
      const wholeSecs = Math.floor(secs);
      const ms = Math.floor((secs - wholeSecs) * 1000);
      return `${mins}:${wholeSecs.toString().padStart(2, '0')}.${ms.toString().padStart(3, '0')}`;
    }
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
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
      audioElement.play().then(() => {
        isPlaying = true;
        onPlay?.();
      }).catch((error) => {
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
      audioElement.play().then(() => {
        isPlaying = true;
        onPlay?.();
      }).catch((error) => {
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
      ontimeupdate={handleTimeUpdate}
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
    role="button"
    tabindex="0"
    onclick={seek}
    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') seek(e as any); }}
  >
    <div class="h-1 bg-default rounded-sm overflow-hidden">
      <div
        class="h-full bg-accent transition-[width] duration-100"
        style="width: {duration ? (currentTime / duration) * 100 : 0}%"
      ></div>
    </div>
    <div class="flex justify-between mt-1 text-[0.625rem] text-secondary">
      <span>{formatTime(currentTime)}</span>
      <span>{formatTime(duration)}</span>
    </div>
  </div>

  <!-- Volume control -->
  <div class="flex items-center gap-2 flex-shrink-0">
    <svg class="w-4 h-4 text-secondary" fill="currentColor" viewBox="0 0 20 20">
      <path fill-rule="evenodd" d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217zM14.657 2.929a1 1 0 011.414 0A9.972 9.972 0 0119 10a9.972 9.972 0 01-2.929 7.071 1 1 0 01-1.414-1.414A7.971 7.971 0 0017 10c0-2.21-.894-4.208-2.343-5.657a1 1 0 010-1.414zm-2.829 2.828a1 1 0 011.415 0A5.983 5.983 0 0115 10a5.984 5.984 0 01-1.757 4.243 1 1 0 01-1.415-1.415A3.984 3.984 0 0013 10a3.983 3.983 0 00-1.172-2.828 1 1 0 010-1.415z" clip-rule="evenodd" />
    </svg>
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
