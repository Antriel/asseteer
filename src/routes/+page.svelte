<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  // Redirect to library page
  onMount(() => {
    goto('/library', { replaceState: true });
  });
</script>

<div class="flex items-center justify-center h-full">
  <p class="text-secondary">Redirecting...</p>
</div>
