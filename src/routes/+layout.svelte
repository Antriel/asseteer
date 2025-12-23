<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import StatusBar from '$lib/components/layout/StatusBar.svelte';
  import ToastContainer from '$lib/components/shared/ToastContainer.svelte';
  import { processingState } from '$lib/state/tasks.svelte';

  let { children } = $props();

  // Initialize processing listeners globally (once)
  onMount(() => {
    processingState.initializeListeners();
    processingState.refreshPendingCount();

    return () => {
      processingState.cleanup();
    };
  });
</script>

<div class="h-screen flex flex-col bg-primary overflow-hidden">
  <!-- Main container with sidebar -->
  <div class="flex flex-1 overflow-hidden">
    <!-- Sidebar -->
    <Sidebar />

    <!-- Main content area -->
    <main class="flex-1 overflow-hidden">
      {@render children()}
    </main>
  </div>

  <!-- Status bar -->
  <StatusBar />

  <!-- Global toast notifications -->
  <ToastContainer />
</div>
