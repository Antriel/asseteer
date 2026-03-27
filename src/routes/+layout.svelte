<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import StatusBar from '$lib/components/layout/StatusBar.svelte';
  import FolderSidebar from '$lib/components/FolderSidebar.svelte';
  import ToastContainer from '$lib/components/shared/ToastContainer.svelte';
  import ConfirmDialog from '$lib/components/shared/ConfirmDialog.svelte';
  import { processingState } from '$lib/state/tasks.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { viewState } from '$lib/state/view.svelte';

  let { children } = $props();

  let isLibraryPage = $derived($page.url.pathname.startsWith('/library'));

  // Resize state
  let isResizing = $state(false);
  let resizeStartX = $state(0);
  let resizeStartWidth = $state(0);

  function onResizeStart(e: MouseEvent) {
    isResizing = true;
    resizeStartX = e.clientX;
    resizeStartWidth = viewState.folderPanelWidth;
    e.preventDefault();
  }

  function onResizeMove(e: MouseEvent) {
    if (!isResizing) return;
    const delta = e.clientX - resizeStartX;
    const newWidth = Math.min(500, Math.max(200, resizeStartWidth + delta));
    viewState.folderPanelWidth = newWidth;
  }

  function onResizeEnd() {
    isResizing = false;
  }

  // Initialize processing listeners globally (once)
  onMount(() => {
    processingState.initializeListeners();
    processingState.refreshPendingCount();
    clapState.initialize();

    return () => {
      processingState.cleanup();
      clapState.stopHealthMonitor();
    };
  });
</script>

<svelte:window
  onmousemove={isResizing ? onResizeMove : undefined}
  onmouseup={isResizing ? onResizeEnd : undefined}
/>

<div class="h-screen flex flex-col bg-primary overflow-hidden">
  <!-- Main container with sidebar -->
  <div class="flex flex-1 overflow-hidden">
    <!-- Sidebar -->
    <Sidebar />

    <!-- Folder Panel (always visible on library page, collapsed or expanded) -->
    {#if isLibraryPage}
      <div
        class="relative flex-shrink-0 flex h-full transition-all duration-200"
        style="width: {viewState.folderSidebarOpen ? viewState.folderPanelWidth : 48}px"
      >
        <FolderSidebar />
        {#if viewState.folderSidebarOpen}
          <!-- Resize handle (only when expanded) -->
          <div
            class="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize z-10 hover:bg-accent/30 transition-colors {isResizing
              ? 'bg-accent/40'
              : ''}"
            onmousedown={onResizeStart}
            role="separator"
            aria-orientation="vertical"
            tabindex="-1"
          ></div>
        {/if}
      </div>
    {/if}

    <!-- Main content area -->
    <main class="flex-1 overflow-hidden">
      {@render children()}
    </main>
  </div>

  <!-- Status bar -->
  <StatusBar />

  <!-- Global toast notifications -->
  <ToastContainer />

  <!-- Global confirm dialog -->
  <ConfirmDialog />
</div>

{#if isResizing}
  <div class="fixed inset-0 z-50 cursor-col-resize"></div>
{/if}
