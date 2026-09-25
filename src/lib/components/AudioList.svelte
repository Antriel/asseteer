<script lang="ts">
  import { untrack } from 'svelte';
  import type { Asset } from '$lib/types';
  import { getAssetDisplayPath, getAssetRelativeDirectory } from '$lib/types';
  import AudioPlayer from './AudioPlayer.svelte';
  import VirtualList from './shared/VirtualList.svelte';
  import AssetContextMenu from './shared/AssetContextMenu.svelte';
  import {
    FolderIcon,
    ExternalLinkIcon,
    SimilarIcon,
    PlayIcon,
    PauseIcon,
    PlayOnceIcon,
    SkipNextIcon,
    RepeatIcon,
  } from './icons';
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import { settings, type AudioEndMode } from '$lib/state/settings.svelte';
  import {
    showInFolder,
    openDirectory,
    dragOut,
    copyAssetFiles,
    isCopyFilesShortcut,
  } from '$lib/actions/assetActions';
  import { ListSelection } from '$lib/state/listSelection.svelte';
  import { formatDurationCompact, formatFileSize, formatSimilarity } from '$lib/utils/format';

  // Extended asset type with optional similarity score
  type AudioAsset = Asset & { similarity?: number; matched_query?: number };

  interface Props {
    assets: AudioAsset[];
    showSimilarity?: boolean;
  }

  let { assets, showSimilarity = false }: Props = $props();

  // The sound in the transport (the "current" row)…
  let selectedAsset = $state<Asset | null>(null);
  // …and the rows picked for drag / Copy Path, which usually is just that one
  const selection = new ListSelection();

  $effect(() => {
    const list = assets;
    untrack(() => {
      if (selection.size > 0) selection.retain(list.map((a) => a.id));
    });
  });
  let shouldAutoPlay = $state(false);
  let playKey = $state(0);
  let audioPlayerRef = $state<ReturnType<typeof AudioPlayer> | null>(null);
  let virtualListRef = $state<ReturnType<typeof VirtualList> | null>(null);
  // Track if audio should auto-play on navigation (true while playing or after natural end, false after manual pause)
  let shouldContinuePlaying = $state(false);
  let containerRef = $state<HTMLDivElement | null>(null);

  // Mirrored from the player so the selected row can show the playhead
  let playProgress = $state(0);
  let isPlaying = $state(false);

  // One line per sound: h-8, divider included
  const itemHeight = 32;

  const endModes: { mode: AudioEndMode; label: string; icon: typeof PlayOnceIcon }[] = [
    { mode: 'stop', label: 'Stop at end', icon: PlayOnceIcon },
    { mode: 'next', label: 'Play next', icon: SkipNextIcon },
    { mode: 'repeat', label: 'Repeat', icon: RepeatIcon },
  ];

  // Audition the loop point: repeat, starting just before the end
  function testLoop() {
    settings.setAudioEndMode('repeat');
    audioPlayerRef?.playFromEnd(5);
    containerRef?.focus();
  }

  function formatChannels(channels: number | null): string {
    if (!channels) return '';
    return channels === 1 ? 'Mono' : channels === 2 ? 'Stereo' : `${channels} ch`;
  }

  function handleRowClick(e: MouseEvent, asset: Asset) {
    if (e.ctrlKey || e.metaKey) {
      selection.toggle(asset.id);
      containerRef?.focus();
    } else if (e.shiftKey) {
      selection.extendTo(
        assets.map((a) => a.id),
        asset.id,
      );
      containerRef?.focus();
    } else {
      selection.only(asset.id);
      playAsset(asset);
    }
  }

  function playAsset(asset: Asset) {
    if (selectedAsset?.id === asset.id) {
      // Same asset - restart playback from beginning
      playKey++;
      shouldAutoPlay = true;
      shouldContinuePlaying = true;
    } else {
      selectedAsset = asset;
      shouldAutoPlay = true;
      shouldContinuePlaying = true;
    }
    // Refocus container so keyboard navigation continues working
    containerRef?.focus();
  }

  function getSelectedIndex(): number {
    if (!selectedAsset) return -1;
    return assets.findIndex((a) => a.id === selectedAsset!.id);
  }

  /**
   * Move the current row. `select`: `only` collapses the selection to it (arrow keys),
   * `extend` grows the range to it (Shift+arrows), `follow` leaves a multi-selection
   * alone (auto-advance at the end of a sound).
   */
  function navigateToIndex(newIndex: number, select: 'only' | 'extend' | 'follow' = 'only') {
    if (newIndex < 0 || newIndex >= assets.length) return;

    const newAsset = assets[newIndex];
    const wasPlaying = shouldContinuePlaying;

    selectedAsset = newAsset;
    if (select === 'extend') {
      selection.extendTo(
        assets.map((a) => a.id),
        newAsset.id,
      );
    } else if (select === 'only' || selection.size <= 1) {
      selection.only(newAsset.id);
    }

    // Scroll to make the item visible with 1 item buffer
    virtualListRef?.scrollToIndex(newIndex, 1);

    if (wasPlaying) {
      shouldAutoPlay = true;
      shouldContinuePlaying = true;
    }
  }

  // Context menu
  let contextMenu = $state<{ x: number; y: number; asset: AudioAsset } | null>(null);

  function handleContextMenu(e: MouseEvent, asset: AudioAsset) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY, asset };
  }

  async function findSimilar(asset: AudioAsset) {
    contextMenu = null;
    try {
      await clapState.searchBySimilarity(
        asset.id,
        asset.filename,
        undefined,
        assetsState.durationFilter,
        assetsState.folderLocation,
      );
    } catch (error) {
      showToast(`${error}`, 'error');
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    const currentIndex = getSelectedIndex();

    // Shift+arrows extend the selection; Shift+Tab is plain "up"
    const select = e.shiftKey && e.key.startsWith('Arrow') ? 'extend' : 'only';

    // Arrow Up / Shift+Tab - navigate up
    if (e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey)) {
      e.preventDefault();
      if (currentIndex <= 0) {
        // Already at top or no selection - select first item
        navigateToIndex(0, select);
      } else {
        navigateToIndex(currentIndex - 1, select);
      }
      return;
    }

    // Arrow Down / Tab - navigate down
    if (e.key === 'ArrowDown' || (e.key === 'Tab' && !e.shiftKey)) {
      e.preventDefault();
      if (currentIndex < 0) {
        // No selection - select first item
        navigateToIndex(0, select);
      } else if (currentIndex < assets.length - 1) {
        navigateToIndex(currentIndex + 1, select);
      }
      return;
    }

    // Ctrl+C - copy the selected files, for pasting into Explorer or a DAW
    if (isCopyFilesShortcut(e)) {
      e.preventDefault();
      copyAssetFiles(selection.items(assets));
      return;
    }

    // Escape - collapse a multi-selection back to the current sound
    if (e.key === 'Escape' && selection.size > 1) {
      e.preventDefault();
      if (selectedAsset) selection.only(selectedAsset.id);
      else selection.clearAt(null);
      return;
    }

    // Space - toggle play/pause
    if (e.key === ' ') {
      e.preventDefault();
      if (!selectedAsset && assets.length > 0) {
        // No selection - select and play first item
        selectedAsset = assets[0];
        selection.only(assets[0].id);
        shouldAutoPlay = true;
        shouldContinuePlaying = true;
      } else if (audioPlayerRef) {
        audioPlayerRef.toggle();
      }
      return;
    }

    // Arrow Left - seek backward 10%
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      if (audioPlayerRef && selectedAsset) {
        audioPlayerRef.seekByPercent(-0.1);
      }
      return;
    }

    // Arrow Right - seek forward 10%
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      if (audioPlayerRef && selectedAsset) {
        const result = audioPlayerRef.seekByPercent(0.1);
        if (result.stopped) {
          // Seeking past end stopped playback - but keep shouldContinuePlaying true
          // so navigation will auto-play next item
        }
      }
      return;
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="flex flex-col h-full overflow-hidden outline-none"
  bind:this={containerRef}
  tabindex="0"
  role="application"
  aria-label="Audio list player"
  onkeydown={handleKeyDown}
>
  <!-- Transport strip: docked, same height whether or not something is selected -->
  <div
    class="@container h-[68px] px-4 flex flex-col justify-center bg-secondary border-b border-default flex-shrink-0"
  >
    {#if selectedAsset}
      <AudioPlayer
        bind:this={audioPlayerRef}
        bind:progress={playProgress}
        bind:playing={isPlaying}
        asset={selectedAsset}
        isActive={true}
        loop={settings.audioEndMode === 'repeat'}
        autoPlay={shouldAutoPlay}
        restartKey={playKey}
        onPlay={() => {
          shouldAutoPlay = false;
          shouldContinuePlaying = true;
        }}
        onPause={() => {
          // Manual pause (not from onEnded) - stop auto-playing on navigation
          shouldContinuePlaying = false;
        }}
        onEnded={() => {
          // Natural end - keep shouldContinuePlaying true so navigation auto-plays
          // Note: onPause is called before onEnded, so we need to restore it
          shouldContinuePlaying = true;
          if (settings.audioEndMode === 'next') {
            navigateToIndex(getSelectedIndex() + 1, 'follow');
          }
        }}
      >
        {#snippet info()}
          <div class="flex items-center gap-3 min-w-0 h-5">
            <p
              class="font-medium text-primary truncate"
              title={getAssetDisplayPath(selectedAsset!)}
            >
              {selectedAsset!.filename}
            </p>
            <p class="hidden @3xl:block text-xs text-tertiary whitespace-nowrap flex-shrink-0">
              {[
                selectedAsset!.sample_rate ? `${selectedAsset!.sample_rate / 1000} kHz` : '',
                formatChannels(selectedAsset!.channels),
                selectedAsset!.format.toUpperCase(),
                formatFileSize(selectedAsset!.file_size),
              ]
                .filter(Boolean)
                .join(' · ')}
            </p>
            <div class="flex items-center gap-0.5 ml-auto flex-shrink-0">
              {#if selection.size > 1}
                <span
                  class="mr-2 text-xs text-accent whitespace-nowrap"
                  title="Drag a selected row to take them all · Esc to clear"
                  >{selection.size} selected</span
                >
              {/if}
              <button
                class="w-6 h-6 flex items-center justify-center text-tertiary hover:text-purple-500 hover:bg-tertiary rounded transition-colors"
                onclick={() => findSimilar(selectedAsset! as AudioAsset)}
                title="Find similar sounds"
              >
                <SimilarIcon size="sm" />
              </button>
              <button
                class="w-6 h-6 flex items-center justify-center text-tertiary hover:text-primary hover:bg-tertiary rounded transition-colors"
                onclick={() => showInFolder(selectedAsset!, 'audio')}
                title="Show in folder"
              >
                <FolderIcon size="sm" />
              </button>
              <button
                class="w-6 h-6 flex items-center justify-center text-tertiary hover:text-primary hover:bg-tertiary rounded transition-colors"
                onclick={() => openDirectory(selectedAsset!)}
                title="Open in file explorer"
              >
                <ExternalLinkIcon size="sm" />
              </button>
            </div>
          </div>
        {/snippet}

        {#snippet controls()}
          <div class="flex items-center gap-2 flex-shrink-0 pl-3 @xl:pl-4 border-l border-default">
            <div
              class="flex items-center p-0.5 bg-primary border border-default rounded-md"
              role="radiogroup"
              aria-label="At end of track"
            >
              {#each endModes as { mode, label, icon: Icon } (mode)}
                {@const active = settings.audioEndMode === mode}
                <button
                  class="w-7 h-6 flex items-center justify-center rounded transition-colors {active
                    ? 'bg-accent-light text-accent'
                    : 'text-tertiary hover:text-primary'}"
                  role="radio"
                  aria-checked={active}
                  aria-label={label}
                  title="At end of track: {label}"
                  onclick={() => settings.setAudioEndMode(mode)}
                >
                  <Icon size="sm" />
                </button>
              {/each}
            </div>
            <button
              class="hidden @xl:block h-7 px-2 text-xs font-medium text-secondary hover:text-primary hover:bg-tertiary border border-default rounded-md transition-colors whitespace-nowrap"
              onclick={testLoop}
              title="Repeat, starting 5 s before the end — hear the loop point"
            >
              Test loop
            </button>
          </div>
        {/snippet}
      </AudioPlayer>
    {:else}
      <div class="flex items-center gap-4 text-sm text-tertiary">
        <div
          class="w-9 h-9 flex items-center justify-center rounded-full border border-default flex-shrink-0"
        >
          <PlayIcon size="sm" class="translate-x-px" />
        </div>
        <p>
          Pick a sound to play
          <span class="ml-3 text-xs hidden @2xl:inline">
            <kbd class="px-1 rounded border border-default font-sans">↑</kbd>
            <kbd class="px-1 rounded border border-default font-sans">↓</kbd> browse
            <span class="mx-1.5">·</span>
            <kbd class="px-1 rounded border border-default font-sans">Space</kbd> play / pause
            <span class="mx-1.5">·</span>
            <kbd class="px-1 rounded border border-default font-sans">←</kbd>
            <kbd class="px-1 rounded border border-default font-sans">→</kbd> seek
          </span>
        </p>
      </div>
    {/if}
  </div>

  <!-- One line per sound, virtualised -->
  <div class="flex-1 overflow-hidden @container">
    <VirtualList bind:this={virtualListRef} items={assets} {itemHeight} bufferItems={10}>
      {#snippet children({ visibleItems })}
        {#each visibleItems as asset (asset.id)}
          {@const selected = selectedAsset?.id === asset.id}
          <button
            class="group relative w-full h-8 flex items-center gap-3 pl-3 pr-4 text-left text-sm border-b border-subtle select-none focus:outline-none {selected ||
            selection.has(asset.id)
              ? 'bg-accent-light'
              : 'hover:bg-secondary'}"
            onclick={(e) => handleRowClick(e, asset)}
            oncontextmenu={(e) => handleContextMenu(e, asset)}
            {@attach dragOut(() => selection.targets(assets, asset))}
            tabindex="-1"
            title={getAssetDisplayPath(asset)}
          >
            {#if selected}
              <!-- Playhead wash: how far into the sound you are, where your eyes already are -->
              <span
                class="absolute inset-y-0 left-0 bg-accent opacity-10 pointer-events-none"
                style="width: {playProgress * 100}%"
              ></span>
              <span class="absolute inset-y-0 left-0 w-0.5 bg-accent"></span>
            {/if}

            <span
              class="relative w-4 flex items-center justify-center flex-shrink-0 {selected
                ? 'text-accent'
                : 'text-tertiary opacity-0 group-hover:opacity-100'}"
            >
              {#if selected && isPlaying}
                <PauseIcon class="w-3 h-3" />
              {:else}
                <PlayIcon class="w-3 h-3" />
              {/if}
            </span>

            <span
              class="relative truncate flex-shrink min-w-0 max-w-[60%] text-primary"
              class:font-medium={selected}
            >
              {asset.filename}
            </span>
            <span
              class="relative flex-1 min-w-0 truncate text-xs text-tertiary [direction:rtl] text-left"
            >
              <bdi>{getAssetRelativeDirectory(asset)}</bdi>
            </span>

            {#if asset.zip_entry}
              <span
                class="relative flex-shrink-0 px-1 text-[10px] font-semibold tracking-wide text-tertiary border border-default rounded"
                >ZIP</span
              >
            {/if}
            {#if showSimilarity && asset.matched_query !== undefined && clapState.lastQueries.length > 1}
              <!-- Several alternatives searched: which one this sound matched -->
              <span
                class="relative flex-shrink-0 max-w-32 truncate text-[11px] text-purple-600/80 dark:text-purple-400/80"
                title="Matched “{clapState.lastQueries[asset.matched_query]}”"
                >{clapState.lastQueries[asset.matched_query]}</span
              >
            {/if}
            {#if showSimilarity && asset.similarity !== undefined}
              <span
                class="relative flex-shrink-0 w-10 text-right text-xs font-medium tabular-nums text-purple-600 dark:text-purple-400"
              >
                {formatSimilarity(asset.similarity)}
              </span>
            {/if}

            <span
              class="relative w-16 flex-shrink-0 text-right text-xs tabular-nums text-secondary"
            >
              {asset.duration_ms ? formatDurationCompact(asset.duration_ms) : '—'}
            </span>
            <span
              class="relative w-16 flex-shrink-0 text-right text-xs tabular-nums text-tertiary hidden @3xl:block"
            >
              {asset.sample_rate ? `${asset.sample_rate / 1000} kHz` : ''}
            </span>
            <span class="relative w-12 flex-shrink-0 text-xs text-tertiary hidden @3xl:block">
              {formatChannels(asset.channels)}
            </span>
            <span class="relative w-9 flex-shrink-0 text-xs text-tertiary uppercase">
              {asset.format}
            </span>
            <span
              class="relative w-16 flex-shrink-0 text-right text-xs tabular-nums text-tertiary hidden @xl:block"
            >
              {formatFileSize(asset.file_size)}
            </span>
          </button>
        {/each}
      {/snippet}
    </VirtualList>
  </div>
</div>

<!-- Context menu -->
{#if contextMenu}
  <AssetContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    asset={contextMenu.asset}
    targets={selection.targets(assets, contextMenu.asset)}
    onclose={() => (contextMenu = null)}
    onShowInFolder={(a) => showInFolder(a, viewState.activeTab === 'images' ? 'image' : 'audio')}
    onOpenDirectory={openDirectory}
  >
    {#snippet extraItems()}
      <button
        class="w-full px-3 py-2 text-sm text-left text-primary hover:bg-tertiary flex items-center gap-2 transition-colors"
        onclick={() => findSimilar(contextMenu!.asset)}
      >
        <SimilarIcon size="sm" class="text-purple-500" />
        Find Similar Sounds
      </button>
    {/snippet}
  </AssetContextMenu>
{/if}
