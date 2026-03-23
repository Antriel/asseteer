<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { processingState, isAnyRunning } from '$lib/state/tasks.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { assetsState } from '$lib/state/assets.svelte';

  // Get pending count for badge
  const pendingTotal = $derived(processingState.pendingCount.total);
  const isProcessing = $derived(isAnyRunning(processingState));
  const collapsed = $derived(viewState.sidebarCollapsed);

  onMount(() => {
    if (exploreState.roots.length === 0) {
      exploreState.loadRoots();
    }
  });

  function selectFolder(
    folderId: number,
    folderKey: string,
    location: import('$lib/types').FolderLocation,
  ) {
    const assetType = viewState.activeTab === 'images' ? 'image' : 'audio';
    assetsState.setFolderFilter(location, assetType);
    exploreState.selectedKey = folderKey;
    exploreState.selectedLocation = location;
    goto('/library');
  }

  function isFolderSelected(folderId: number): boolean {
    const loc = assetsState.folderLocation;
    return loc !== null && loc.folderId === folderId && loc.relPath === '';
  }

  interface NavItem {
    href: string;
    label: string;
    icon: 'library' | 'processing' | 'folders' | 'settings';
  }

  const navItems: NavItem[] = [
    { href: '/library', label: 'Library', icon: 'library' },
    { href: '/folders', label: 'Folders', icon: 'folders' },
    { href: '/processing', label: 'Processing', icon: 'processing' },
  ];

  const bottomNavItems: NavItem[] = [{ href: '/settings', label: 'Settings', icon: 'settings' }];

  function isActive(href: string): boolean {
    return $page.url.pathname === href || $page.url.pathname.startsWith(href + '/');
  }
</script>

<aside
  class="h-full flex flex-col sidebar-gradient border-r border-default transition-all duration-200 {collapsed
    ? 'w-14'
    : 'w-56'}"
>
  <!-- Logo/Title -->
  <div class="border-b border-default {collapsed ? 'p-2' : 'p-4'}">
    {#if collapsed}
      <div class="flex items-center justify-center">
        <span class="text-lg font-bold text-primary">A</span>
      </div>
    {:else}
      <h1 class="text-lg font-semibold text-primary tracking-tight">Asseteer</h1>
      <p class="text-xs text-tertiary mt-0.5">Asset Manager</p>
    {/if}
  </div>

  <!-- Navigation -->
  <nav class="flex-1 p-2 space-y-1">
    {#each navItems as item}
      {@const active = isActive(item.href)}
      <a
        href={item.href}
        class="flex items-center rounded-lg transition-default
               {collapsed ? 'justify-center p-2.5' : 'gap-3 px-3 py-2.5'}
               {active
          ? 'bg-accent-muted border-l-2 border-accent text-primary'
          : 'text-secondary hover:bg-tertiary hover:text-primary'}"
        title={collapsed ? item.label : undefined}
      >
        <!-- Icon -->
        <div class="w-5 h-5 flex items-center justify-center flex-shrink-0">
          {#if item.icon === 'library'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z"
              />
            </svg>
          {:else if item.icon === 'folders'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M8 14h8m-8-3h4"
              />
            </svg>
          {:else if item.icon === 'processing'}
            <svg
              class="w-5 h-5 {isProcessing ? 'animate-spin' : ''}"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
          {:else if item.icon === 'settings'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
              />
            </svg>
          {/if}
        </div>

        <!-- Label (hidden when collapsed) -->
        {#if !collapsed}
          <span class="font-medium text-sm">{item.label}</span>

          <!-- Badge for processing -->
          {#if item.icon === 'processing' && (pendingTotal > 0 || isProcessing)}
            <span
              class="ml-auto px-1.5 py-0.5 text-xs font-medium rounded-full
                         {isProcessing ? 'bg-accent text-white' : 'bg-tertiary text-secondary'}"
            >
              {isProcessing ? 'Active' : pendingTotal}
            </span>
          {/if}
        {/if}
      </a>
    {/each}
  </nav>

  <!-- Folder list section -->
  {#if exploreState.roots.length > 0}
    <div class="border-t border-default {collapsed ? 'px-2 py-2' : 'px-2 pt-2 pb-1'}">
      {#if !collapsed}
        <p class="px-2 pb-1 text-xs font-semibold text-tertiary uppercase tracking-wider">
          Folders
        </p>
      {/if}
      <div class="space-y-0.5">
        {#each exploreState.roots as folder (folder.key)}
          {@const active = isFolderSelected(folder.location.folderId)}
          <button
            onclick={() => selectFolder(folder.location.folderId, folder.key, folder.location)}
            class="flex items-center w-full rounded-lg transition-default text-left
                   {collapsed ? 'justify-center p-2' : 'gap-2.5 px-2 py-1.5'}
                   {active
              ? 'bg-accent-muted text-primary'
              : 'text-secondary hover:bg-tertiary hover:text-primary'}"
            title={collapsed ? folder.name : undefined}
          >
            <div class="w-4 h-4 flex items-center justify-center flex-shrink-0">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="1.5"
                  d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                />
              </svg>
            </div>
            {#if !collapsed}
              <span class="text-xs font-medium truncate">{folder.name}</span>
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Bottom nav -->
  <div class="p-2 border-t border-default space-y-1">
    {#each bottomNavItems as item}
      {@const active = isActive(item.href)}
      <a
        href={item.href}
        class="flex items-center rounded-lg transition-default
               {collapsed ? 'justify-center p-2' : 'gap-3 px-3 py-2'}
               {active
          ? 'bg-accent-muted border-l-2 border-accent text-primary'
          : 'text-tertiary hover:bg-tertiary hover:text-primary'}"
        title={collapsed ? item.label : undefined}
      >
        <div class="w-5 h-5 flex items-center justify-center flex-shrink-0">
          {#if item.icon === 'settings'}
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="1.5"
                d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
              />
            </svg>
          {/if}
        </div>
        {#if !collapsed}
          <span class="text-sm">{item.label}</span>
        {/if}
      </a>
    {/each}

    <!-- Collapse/Expand toggle -->
    <button
      onclick={() => viewState.toggleSidebarCollapsed()}
      class="flex items-center w-full rounded-lg transition-default text-tertiary hover:bg-tertiary hover:text-primary {collapsed
        ? 'justify-center p-2'
        : 'gap-3 px-3 py-2'}"
      title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
    >
      <div class="w-5 h-5 flex items-center justify-center flex-shrink-0">
        <svg
          class="w-4 h-4 transition-transform duration-200 {collapsed ? 'rotate-180' : ''}"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="1.5"
            d="M11 19l-7-7 7-7m8 14l-7-7 7-7"
          />
        </svg>
      </div>
      {#if !collapsed}
        <span class="text-sm">Collapse</span>
      {/if}
    </button>
  </div>
</aside>
