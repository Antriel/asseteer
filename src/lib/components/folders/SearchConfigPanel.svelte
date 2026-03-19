<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getDatabase } from '$lib/database/connection';
  import { getSearchConfig, getTopLevelSubfolders, getSampleAssetPath } from '$lib/database/queries';
  import { showToast } from '$lib/state/ui.svelte';
  import type { SearchConfigEntry } from '$lib/types';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';
  import SearchIcon from '$lib/components/icons/SearchIcon.svelte';

  interface Props {
    folderId: number;
    folderPath: string;
  }

  let { folderId, folderPath }: Props = $props();

  let expanded = $state(false);
  let loading = $state(false);
  let saving = $state(false);
  let loaded = $state(false);
  let entries = $state<SearchConfigEntry[]>([]);
  let subfolders = $state<string[]>([]);
  let hasChanges = $state(false);
  let samplePath = $state<string | null>(null);

  // Track original state to detect changes
  let originalJson = $state('');

  $effect(() => {
    hasChanges = JSON.stringify(entries) !== originalJson;
  });

  async function load() {
    loading = true;
    try {
      const db = await getDatabase();
      const [config, subs, sample] = await Promise.all([
        getSearchConfig(db, folderId),
        getTopLevelSubfolders(db, folderId),
        getSampleAssetPath(db, folderId),
      ]);
      entries = config;
      originalJson = JSON.stringify(config);
      subfolders = subs;
      samplePath = sample ? (sample.rel_path ? sample.rel_path + '/' + sample.filename : sample.filename) : null;
      loaded = true;
    } catch (error) {
      showToast('Failed to load search config: ' + error, 'error');
    } finally {
      loading = false;
    }
  }

  function toggle() {
    expanded = !expanded;
    if (expanded && !loaded && !loading) {
      load();
    }
  }

  function addRule() {
    // Default: root rule with skip_depth 0
    const existingPrefixes = new Set(entries.map((e) => e.subfolder_prefix));
    // Find first available subfolder not already configured
    const availablePrefix = subfolders.find((s) => !existingPrefixes.has(s)) ?? '';
    if (existingPrefixes.has(availablePrefix)) {
      showToast('All subfolders already have rules configured', 'info');
      return;
    }
    entries = [...entries, { subfolder_prefix: availablePrefix, skip_depth: 1 }];
  }

  function removeRule(index: number) {
    entries = entries.filter((_, i) => i !== index);
  }

  function updatePrefix(index: number, value: string) {
    entries = entries.map((e, i) => (i === index ? { ...e, subfolder_prefix: value } : e));
  }

  function updateDepth(index: number, value: number) {
    entries = entries.map((e, i) => (i === index ? { ...e, skip_depth: Math.max(0, value) } : e));
  }

  async function save() {
    // Validate no duplicate prefixes
    const prefixes = entries.map((e) => e.subfolder_prefix);
    if (new Set(prefixes).size !== prefixes.length) {
      showToast('Duplicate subfolder prefixes are not allowed', 'error');
      return;
    }

    // Filter out rules with skip_depth 0 and empty prefix (they're no-ops)
    const effectiveEntries = entries.filter(
      (e) => !(e.subfolder_prefix === '' && e.skip_depth === 0),
    );

    saving = true;
    try {
      await invoke('update_search_config', {
        folderId,
        config: effectiveEntries,
      });
      entries = effectiveEntries;
      originalJson = JSON.stringify(effectiveEntries);
      showToast('Search settings saved and re-indexed', 'success');
    } catch (error) {
      showToast('Failed to save search config: ' + error, 'error');
    } finally {
      saving = false;
    }
  }

  function computePreview(prefix: string, skipDepth: number): string {
    if (!samplePath) return '';

    // Simulate compute_searchable_path logic
    let pathToUse = samplePath;

    const matches = prefix === '' || pathToUse === prefix || pathToUse.startsWith(prefix + '/');
    if (!matches) return pathToUse;

    // Strip the prefix
    let remainder: string;
    if (prefix === '') {
      remainder = pathToUse;
    } else if (pathToUse.length === prefix.length) {
      remainder = '';
    } else {
      remainder = pathToUse.substring(prefix.length + 1);
    }

    // Skip additional depth segments
    const segments = remainder.split('/').filter(Boolean);
    if (skipDepth >= segments.length) return '(all segments skipped)';
    return segments.slice(skipDepth).join(' / ');
  }
</script>

<div class="border-t border-default">
  <button
    onclick={toggle}
    class="w-full flex items-center gap-2 px-4 py-2.5 text-xs text-secondary hover:text-primary hover:bg-tertiary/50 transition-colors"
  >
    <ChevronIcon size="sm" direction={expanded ? 'down' : 'right'} />
    <SearchIcon size="sm" />
    <span>Search depth settings</span>
  </button>

  {#if expanded}
    <div class="px-4 pb-4">
      {#if loading}
        <div class="flex items-center gap-2 py-3">
          <Spinner size="sm" />
          <span class="text-xs text-tertiary">Loading...</span>
        </div>
      {:else}
        <!-- Info text -->
        <p class="text-xs text-tertiary mb-3">
          Skip organizational path segments from search indexing. Deeper content remains searchable.
        </p>

        <!-- Rules -->
        {#if entries.length === 0}
          <p class="text-xs text-tertiary italic mb-3">
            No rules configured — all path segments are indexed.
          </p>
        {:else}
          <div class="space-y-2 mb-3">
            {#each entries as entry, i}
              <div class="flex items-center gap-2 rounded-lg bg-tertiary/30 p-2">
                <!-- Prefix selector -->
                <div class="flex-1 min-w-0">
                  <span class="text-[10px] text-tertiary uppercase tracking-wide" aria-hidden="true">Prefix</span>
                  {#if subfolders.length > 0}
                    <select
                      aria-label="Subfolder prefix"
                      value={entry.subfolder_prefix}
                      onchange={(e) => updatePrefix(i, (e.target as HTMLSelectElement).value)}
                      class="w-full text-xs bg-primary border border-default rounded px-2 py-1 text-primary mt-0.5"
                    >
                      <option value="">(root)</option>
                      {#each subfolders as sub}
                        <option value={sub}>{sub}</option>
                      {/each}
                    </select>
                  {:else}
                    <input
                      aria-label="Subfolder prefix"
                      type="text"
                      value={entry.subfolder_prefix}
                      oninput={(e) => updatePrefix(i, (e.target as HTMLInputElement).value)}
                      placeholder="(root)"
                      class="w-full text-xs bg-primary border border-default rounded px-2 py-1 text-primary mt-0.5"
                    />
                  {/if}
                </div>

                <!-- Skip depth -->
                <div class="w-20 flex-shrink-0">
                  <span class="text-[10px] text-tertiary uppercase tracking-wide" aria-hidden="true">Skip</span>
                  <input
                    aria-label="Skip depth"
                    type="number"
                    min="0"
                    max="10"
                    value={entry.skip_depth}
                    oninput={(e) => updateDepth(i, parseInt((e.target as HTMLInputElement).value) || 0)}
                    class="w-full text-xs bg-primary border border-default rounded px-2 py-1 text-primary mt-0.5"
                  />
                </div>

                <!-- Remove button -->
                <button
                  onclick={() => removeRule(i)}
                  class="p-1 rounded text-tertiary hover:text-error hover:bg-error/10 transition-colors mt-3"
                  title="Remove rule"
                >
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              <!-- Preview for this rule -->
              {#if samplePath}
                {@const preview = computePreview(entry.subfolder_prefix, entry.skip_depth)}
                {#if preview}
                  <div class="text-[10px] text-tertiary pl-2 flex items-center gap-1.5">
                    <span class="text-tertiary/60">Indexed as:</span>
                    <span class="font-mono text-secondary">{preview}</span>
                  </div>
                {/if}
              {/if}
            {/each}
          </div>
        {/if}

        <!-- Actions -->
        <div class="flex items-center gap-2">
          <button
            onclick={addRule}
            class="text-xs text-accent hover:text-accent/80 transition-colors"
          >
            + Add rule
          </button>
          <div class="flex-1"></div>
          {#if hasChanges}
            <button
              onclick={save}
              disabled={saving}
              class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-accent hover:bg-accent/90 rounded-lg transition-colors disabled:opacity-50"
            >
              {#if saving}
                <Spinner size="sm" />
                Re-indexing...
              {:else}
                Save & re-index
              {/if}
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
