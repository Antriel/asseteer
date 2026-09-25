<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Spinner from '$lib/components/shared/Spinner.svelte';

  /** `get_db_migration` in `src-tauri/src/database/migrate.rs` */
  interface MigrationProgress {
    message: string;
    done: number;
    /** 0 while the amount of work isn't known yet */
    total: number;
    error: string | null;
  }

  let progress = $state<MigrationProgress | null>(null);
  let dismissed = $state(false);

  let percent = $derived(
    progress && progress.total > 0 ? Math.floor((progress.done / progress.total) * 100) : 0,
  );

  // Polled rather than evented: the backend starts before this mounts, and a poll
  // can't miss the start or the end.
  onMount(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;

    async function poll() {
      try {
        progress = await invoke<MigrationProgress | null>('get_db_migration');
      } catch (error) {
        console.error('Failed to read database migration progress:', error);
        progress = null;
        return;
      }
      if (progress && !progress.error && !stopped) timer = setTimeout(poll, 250);
    }
    poll();

    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });
</script>

{#if progress && !dismissed}
  <div class="fixed inset-0 z-[300] flex items-center justify-center bg-black/60">
    <div
      class="bg-elevated border border-default rounded-xl shadow-xl w-full max-w-md mx-4 p-6"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="db-migration-title"
      aria-describedby="db-migration-message"
    >
      <h2 id="db-migration-title" class="text-base font-semibold text-primary mb-2">
        {progress.error ? 'Library update failed' : 'Updating your library'}
      </h2>

      {#if progress.error}
        <p id="db-migration-message" class="text-sm text-secondary mb-2">
          You can keep using the app; the update will be tried again the next time it starts.
        </p>
        <p class="text-xs text-error font-mono break-words mb-6">{progress.error}</p>
        <div class="flex justify-end">
          <button
            onclick={() => (dismissed = true)}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
          >
            Continue
          </button>
        </div>
      {:else}
        <p id="db-migration-message" class="text-sm text-secondary mb-4">
          A one-time update after installing a new version. Large libraries can take a few minutes.
        </p>
        <div class="flex items-center justify-between gap-3 text-xs mb-1.5">
          <span class="flex items-center gap-2 text-secondary min-w-0">
            <Spinner size="sm" />
            <span class="truncate">{progress.message}</span>
          </span>
          {#if progress.total > 0}
            <span class="text-tertiary tabular-nums flex-shrink-0">
              {progress.done.toLocaleString()} / {progress.total.toLocaleString()}
            </span>
          {/if}
        </div>
        <div class="w-full h-2 bg-track rounded-full overflow-hidden">
          <div
            class="h-full bg-accent transition-[width] duration-300"
            style="width: {percent}%"
          ></div>
        </div>
      {/if}
    </div>
  </div>
{/if}
