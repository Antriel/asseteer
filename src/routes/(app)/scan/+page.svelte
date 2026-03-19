<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  onMount(() => {
    goto('/folders', { replaceState: true });
  });
</script>
