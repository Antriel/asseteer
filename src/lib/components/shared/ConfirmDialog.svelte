<script lang="ts">
  import { uiState, resolveConfirm } from '$lib/state/ui.svelte';

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') resolveConfirm(false);
    if (e.key === 'Enter') resolveConfirm(true);
  }
</script>

{#if uiState.confirm}
  <div
    class="fixed inset-0 z-[200] flex items-center justify-center bg-black/50"
    role="button"
    tabindex="-1"
    onclick={() => resolveConfirm(false)}
    onkeydown={handleKeydown}
  >
    <div
      class="bg-elevated border border-default rounded-xl shadow-xl w-full max-w-sm mx-4 p-6"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="confirm-title"
      aria-describedby="confirm-message"
      tabindex="0"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h2 id="confirm-title" class="text-base font-semibold text-primary mb-2">
        {uiState.confirm.title}
      </h2>
      <p id="confirm-message" class="text-sm text-secondary mb-6">
        {uiState.confirm.message}
      </p>
      <div class="flex justify-end gap-2">
        <button
          onclick={() => resolveConfirm(false)}
          class="px-4 py-2 text-sm font-medium rounded-lg border border-default text-secondary hover:text-primary hover:bg-tertiary transition-colors"
        >
          Cancel
        </button>
        <button
          onclick={() => resolveConfirm(true)}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
        >
          {uiState.confirm.confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
