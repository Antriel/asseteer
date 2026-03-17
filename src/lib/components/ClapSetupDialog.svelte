<script lang="ts">
  import { clapState } from '$lib/state/clap.svelte';

  interface Props {
    onComplete: () => void;
    onCancel: () => void;
  }

  let { onComplete, onCancel }: Props = $props();

  type StepDisplayStatus = 'idle' | 'running' | 'done' | 'error';

  // finalStep tracks the completion/error state set by runSetup()
  let finalStep = $state<'running' | 'done' | 'error'>('running');
  let errorMessage = $state('');
  let elapsedSeconds = $state(0);
  let timer: ReturnType<typeof setInterval> | null = null;

  // The actual setup is a single server start call — uv handles all the steps.
  // We derive the displayed step from elapsed time since we can't get granular
  // progress from the subprocess.
  let step = $derived.by(() => {
    if (finalStep === 'done') return 'done' as const;
    if (finalStep === 'error') return 'error' as const;
    if (elapsedSeconds >= 30) return 'downloading-model' as const;
    if (elapsedSeconds >= 10) return 'installing-python' as const;
    return 'downloading-tools' as const;
  });

  function startTimer() {
    timer = setInterval(() => { elapsedSeconds += 1; }, 1000);
  }

  function stopTimer() {
    if (timer) { clearInterval(timer); timer = null; }
  }

  async function runSetup() {
    elapsedSeconds = 0;
    finalStep = 'running';
    startTimer();

    const ok = await clapState.setup();

    stopTimer();

    if (ok) {
      finalStep = 'done';
      setTimeout(onComplete, 800);
    } else {
      finalStep = 'error';
      errorMessage = clapState.setupError ?? 'Unknown error during setup';
    }
  }

  // Start immediately
  runSetup();

  // Derive the index of the active time-based step (ignoring done/error)
  let activeStepIdx = $derived.by(() => {
    if (elapsedSeconds >= 30) return 2;
    if (elapsedSeconds >= 10) return 1;
    return 0;
  });

  function stepStatus(stepName: string): StepDisplayStatus {
    const steps = ['downloading-tools', 'installing-python', 'downloading-model'] as const;
    const stepIdx = steps.indexOf(stepName as typeof steps[number]);

    if (step === 'done') return 'done';
    if (step === 'error') return stepIdx <= activeStepIdx ? 'error' : 'idle';
    if (stepIdx < activeStepIdx) return 'done';
    if (stepIdx === activeStepIdx) return 'running';
    return 'idle';
  }
</script>

<!-- Backdrop -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onkeydown={() => {}}>
  <div class="w-[420px] rounded-xl border border-default bg-primary shadow-xl">
    <!-- Header -->
    <div class="px-6 pt-6 pb-2">
      <h2 class="text-lg font-semibold text-primary">Setting Up Semantic Search</h2>
      <p class="text-sm text-tertiary mt-1">This is a one-time setup. Future starts will be instant.</p>
    </div>

    <!-- Steps -->
    <div class="px-6 py-4 space-y-4">
      {#each [
        { key: 'downloading-tools', label: 'Downloading runtime tools', size: '~30 MB' },
        { key: 'installing-python', label: 'Installing Python environment', size: '~500 MB' },
        { key: 'downloading-model', label: 'Downloading AI model', size: '~1-2 GB' },
      ] as item (item.key)}
        {@const status = stepStatus(item.key)}
        <div class="flex items-center gap-3">
          <!-- Icon -->
          <div class="w-5 h-5 flex items-center justify-center">
            {#if status === 'done'}
              <svg class="w-5 h-5 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
              </svg>
            {:else if status === 'running'}
              <div class="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin"></div>
            {:else if status === 'error'}
              <svg class="w-5 h-5 text-error" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
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
          <span class="text-xs text-tertiary">{item.size}</span>
        </div>
      {/each}
    </div>

    <!-- Error message -->
    {#if step === 'error'}
      <div class="px-6 py-3 mx-6 mb-2 rounded-lg bg-error/10 border border-error/20">
        <p class="text-sm text-error">{errorMessage}</p>
      </div>
    {/if}

    <!-- Footer -->
    <div class="px-6 py-4 border-t border-default flex items-center justify-between">
      <span class="text-xs text-tertiary">
        {#if step === 'done'}
          Setup complete
        {:else if step === 'error'}
          Setup failed
        {:else}
          {elapsedSeconds}s elapsed
        {/if}
      </span>
      <div class="flex gap-2">
        {#if step === 'error'}
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
          {step === 'done' || step === 'error' ? 'Close' : 'Cancel'}
        </button>
      </div>
    </div>
  </div>
</div>
