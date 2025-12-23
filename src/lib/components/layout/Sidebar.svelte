<script lang="ts">
  import { page } from '$app/stores';
  import { processingState, isAnyRunning } from '$lib/state/tasks.svelte';

  // Get pending count for badge
  const pendingTotal = $derived(processingState.pendingCount.total);
  const isProcessing = $derived(isAnyRunning(processingState));

  interface NavItem {
    href: string;
    label: string;
    icon: 'library' | 'processing' | 'scan';
  }

  const navItems: NavItem[] = [
    { href: '/library', label: 'Library', icon: 'library' },
    { href: '/processing', label: 'Processing', icon: 'processing' },
    { href: '/scan', label: 'Scan', icon: 'scan' },
  ];

  function isActive(href: string): boolean {
    return $page.url.pathname === href || $page.url.pathname.startsWith(href + '/');
  }
</script>

<aside class="w-56 h-full flex flex-col sidebar-gradient border-r border-default">
  <!-- Logo/Title -->
  <div class="p-4 border-b border-default">
    <h1 class="text-lg font-semibold text-primary tracking-tight">Asseteer</h1>
    <p class="text-xs text-tertiary mt-0.5">Asset Manager</p>
  </div>

  <!-- Navigation -->
  <nav class="flex-1 p-3 space-y-1">
    {#each navItems as item}
      {@const active = isActive(item.href)}
      <a
        href={item.href}
        class="flex items-center gap-3 px-3 py-2.5 rounded-lg transition-default
               {active ? 'bg-accent-muted border-l-2 border-accent text-primary' : 'text-secondary hover:bg-tertiary hover:text-primary'}"
      >
        <!-- Icon -->
        <div class="w-5 h-5 flex items-center justify-center">
          {#if item.icon === 'library'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
            </svg>
          {:else if item.icon === 'processing'}
            <svg class="w-5 h-5 {isProcessing ? 'animate-spin' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          {:else if item.icon === 'scan'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          {/if}
        </div>

        <!-- Label -->
        <span class="font-medium text-sm">{item.label}</span>

        <!-- Badge for processing -->
        {#if item.icon === 'processing' && (pendingTotal > 0 || isProcessing)}
          <span class="ml-auto px-1.5 py-0.5 text-xs font-medium rounded-full
                       {isProcessing ? 'bg-accent text-white' : 'bg-tertiary text-secondary'}">
            {isProcessing ? 'Active' : pendingTotal}
          </span>
        {/if}
      </a>
    {/each}
  </nav>

  <!-- Footer -->
  <div class="p-3 border-t border-default">
    <div class="text-xs text-tertiary text-center">v0.1.0</div>
  </div>
</aside>
