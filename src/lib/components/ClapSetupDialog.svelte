<script lang="ts">
  import { clapState, type ClapStartupPhase } from '$lib/state/clap.svelte';
  import { checkClapSetupState } from '$lib/database/queries';

  interface Props {
    onComplete: () => void;
    onCancel: () => void;
  }

  let { onComplete, onCancel }: Props = $props();

  type StepDisplayStatus = 'idle' | 'running' | 'done' | 'error';

  let setupFinished = $state<'running' | 'done' | 'error'>('running');
  let errorMessage = $state('');

  // Whether to show the "downloading tools" step (only on first setup)
  let showDownloadStep = $state(false);
  let isFirstTimeSetup = $state(true);
  let stepsReady = $state(false);

  // Phase ordering for step status derivation
  const phaseOrder: ClapStartupPhase[] = [
    'downloading-uv',
    'starting-process',
    'waiting-for-server',
    'loading-model',
    'ready',
  ];

  // Build steps dynamically based on what's already installed
  let steps = $derived.by(() => {
    const items: { key: string; label: string; hint: string }[] = [];
    if (showDownloadStep) {
      items.push({ key: 'downloading-uv', label: 'Downloading runtime tools', hint: '~30 MB' });
    }
    items.push(
      { key: 'starting-process', label: 'Starting Python server', hint: isFirstTimeSetup ? 'may take 20+ min (GPU: ~8 GB)' : '' },
      { key: 'loading-model', label: 'Loading AI model', hint: isFirstTimeSetup ? '~1-2 GB first time' : '' },
    );
    return items;
  });

  function getPhaseIndex(phase: ClapStartupPhase | null): number {
    if (!phase) return -1;
    // Map waiting-for-server to same visual step as starting-process
    const mapped = phase === 'waiting-for-server' ? 'starting-process' : phase;
    return phaseOrder.indexOf(mapped);
  }

  function stepStatus(stepKey: string): StepDisplayStatus {
    if (setupFinished === 'done') return 'done';

    const currentPhase = clapState.startupPhase;
    const currentIdx = getPhaseIndex(currentPhase);
    const stepIdx = phaseOrder.indexOf(stepKey as ClapStartupPhase);

    if (setupFinished === 'error') {
      return stepIdx <= currentIdx ? 'error' : 'idle';
    }

    // Map waiting-for-server to starting-process for visual purposes
    const effectiveCurrentKey =
      currentPhase === 'waiting-for-server' ? 'starting-process' : currentPhase;

    if (stepKey === effectiveCurrentKey) return 'running';
    if (stepIdx < currentIdx) return 'done';
    return 'idle';
  }

  async function runSetup() {
    setupFinished = 'running';

    // Check what's already installed to determine which steps to show
    try {
      const state = await checkClapSetupState();
      showDownloadStep = !state.uv_installed;
      isFirstTimeSetup = !state.cache_exists;
    } catch {
      showDownloadStep = true;
      isFirstTimeSetup = true;
    }
    stepsReady = true;

    const ok = await clapState.setup();

    if (ok) {
      setupFinished = 'done';
      setTimeout(onComplete, 800);
    } else {
      setupFinished = 'error';
      errorMessage = clapState.setupError ?? 'Unknown error during setup';
    }
  }

  // Start immediately
  runSetup();
</script>

<!-- Backdrop -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onkeydown={() => {}}>
  <div class="w-[420px] rounded-xl border border-default bg-primary shadow-xl">
    <!-- Header -->
    <div class="px-6 pt-6 pb-2">
      <h2 class="text-lg font-semibold text-primary">
        {isFirstTimeSetup ? 'Setting Up Semantic Search' : 'Starting Semantic Search'}
      </h2>
      {#if isFirstTimeSetup}
        <p class="text-sm text-tertiary mt-1">
          This is a one-time setup. Future starts will be instant.
        </p>
      {/if}
    </div>

    <!-- Steps -->
    {#if stepsReady}
      <div class="px-6 py-4 space-y-4">
        {#each steps as item (item.key)}
          {@const status = stepStatus(item.key)}
          <div class="flex items-center gap-3">
            <!-- Icon -->
            <div class="w-5 h-5 flex items-center justify-center">
              {#if status === 'done'}
                <svg
                  class="w-5 h-5 text-success"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M5 13l4 4L19 7"
                  />
                </svg>
              {:else if status === 'running'}
                <div
                  class="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin"
                ></div>
              {:else if status === 'error'}
                <svg
                  class="w-5 h-5 text-error"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              {:else}
                <div class="w-4 h-4 rounded-full border-2 border-default"></div>
              {/if}
            </div>

            <!-- Label -->
            <div class="flex-1">
              <span class="text-sm {status === 'idle' ? 'text-tertiary' : 'text-primary'}">
                {item.label}
              </span>
            </div>

            <!-- Size hint -->
            {#if item.hint}
              <span class="text-xs text-tertiary">{item.hint}</span>
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="px-6 py-4 flex items-center gap-2">
        <div
          class="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin"
        ></div>
        <span class="text-sm text-secondary">Checking setup state...</span>
      </div>
    {/if}

    <!-- Download-in-progress notice -->
    {#if setupFinished === 'running' && isFirstTimeSetup && (clapState.startupPhase === 'waiting-for-server' || clapState.startupPhase === 'starting-process')}
      <div class="px-6 pb-3">
        <p class="text-xs text-tertiary">
          Keep this app open while downloading — closing it will cancel the download.
        </p>
      </div>
    {/if}

    <!-- Error message -->
    {#if setupFinished === 'error'}
      <div class="px-6 py-3 mx-6 mb-2 rounded-lg bg-error/10 border border-error/20">
        <p class="text-sm text-error">{errorMessage}</p>
      </div>
    {/if}

    <!-- Footer -->
    <div class="px-6 py-4 border-t border-default flex items-center justify-between">
      <span class="text-xs text-tertiary">
        {#if setupFinished === 'done'}
          Setup complete
        {:else if setupFinished === 'error'}
          Setup failed
        {:else if clapState.startupDetail}
          {clapState.startupDetail}
        {:else}
          Preparing...
        {/if}
      </span>
      <div class="flex gap-2">
        {#if setupFinished === 'error'}
          <button
            onclick={() => runSetup()}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
          >
            Retry
          </button>
        {/if}
        <button
          onclick={onCancel}
          class="px-4 py-2 text-sm font-medium rounded-lg border border-default text-secondary hover:text-primary hover:bg-tertiary transition-colors"
        >
          {setupFinished === 'done' || setupFinished === 'error' ? 'Close' : 'Cancel'}
        </button>
      </div>
    </div>
  </div>
</div>
