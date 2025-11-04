<script lang="ts">
  import ScanControl from '$lib/components/ScanControl.svelte';
  import AssetList from '$lib/components/AssetList.svelte';
</script>

<div class="flex flex-col h-screen bg-primary">
  <header class="flex items-center justify-between px-6 py-4 border-b border-default bg-secondary">
    <h1 class="text-xl font-semibold text-primary">Asset Manager</h1>
  </header>

  <ScanControl />

  <AssetList />
</div>
