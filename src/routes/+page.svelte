<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { getDatabase } from '$lib/database/connection';

  // Redirect to folders if no source folders configured, otherwise to library
  onMount(async () => {
    const db = await getDatabase();
    const result = await db.select<[{ count: number }]>(
      `SELECT COUNT(*) as count FROM source_folders WHERE status = 'active'`,
    );
    const hasFolders = result[0].count > 0;
    goto(hasFolders ? '/library' : '/folders', { replaceState: true });
  });
</script>

<div class="flex items-center justify-center h-full">
  <p class="text-secondary">Redirecting...</p>
</div>
