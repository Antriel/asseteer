<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';

  interface Props {
    audioPath: string;
    isActive?: boolean;
    onPlay?: () => void;
    onPause?: () => void;
  }

  let { audioPath, isActive = false, onPlay, onPause }: Props = $props();

  let audioElement: HTMLAudioElement;
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(1);

  // Convert file path to Tauri-compatible URL
  const audioSrc = $derived(convertFileSrc(audioPath));

  async function togglePlay() {
    if (isPlaying) {
      audioElement.pause();
      isPlaying = false;
      onPause?.();
    } else {
      try {
        await audioElement.play();
        isPlaying = true;
        onPlay?.();
      } catch (error) {
        console.error('Playback failed:', error);
      }
    }
  }

  function handleTimeUpdate() {
    currentTime = audioElement.currentTime;
  }

  function handleLoadedMetadata() {
    duration = audioElement.duration;
  }

  function handleEnded() {
    isPlaying = false;
    onPause?.();
  }

  function seek(e: MouseEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = x / rect.width;
    audioElement.currentTime = percentage * duration;
  }

  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
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
</script>

<div class="flex items-center gap-3">
  <audio
    bind:this={audioElement}
    src={audioSrc}
    ontimeupdate={handleTimeUpdate}
    onloadedmetadata={handleLoadedMetadata}
    onended={handleEnded}
  />

  <!-- Play/Pause button -->
  <button
    class="w-8 h-8 flex items-center justify-center bg-accent text-white border-none rounded-full cursor-pointer hover:opacity-90 transition-opacity flex-shrink-0"
    onclick={togglePlay}
  >
    {#if isPlaying}
      <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
        <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd" />
      </svg>
    {:else}
      <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clip-rule="evenodd" />
      </svg>
    {/if}
  </button>

  <!-- Progress bar -->
  <div class="flex-1 cursor-pointer" onclick={seek}>
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
</div>
