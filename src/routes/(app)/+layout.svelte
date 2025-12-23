<script lang="ts">
  let { children } = $props();
</script>

<div class="h-full flex flex-col overflow-hidden">
  {@render children()}
</div>
