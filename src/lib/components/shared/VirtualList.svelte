<script lang="ts">
  import { onMount, type Snippet } from 'svelte';

  interface Props {
    items: any[];
    itemHeight: number;
    bufferItems?: number;
    children: Snippet<[{ visibleItems: any[], startIndex: number, offsetY: number, endIndex: number }]>;
  }

  let { items, itemHeight, bufferItems = 5, children }: Props = $props();

  let containerElement: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(0);

  // Calculate virtual scrolling parameters
  const totalItems = $derived(items.length);
  const totalHeight = $derived(totalItems * itemHeight);
  const visibleCount = $derived(Math.ceil(containerHeight / itemHeight) + 1);

  const startIndex = $derived(Math.max(0, Math.floor(scrollTop / itemHeight) - bufferItems));
  const endIndex = $derived(Math.min(totalItems, startIndex + visibleCount + bufferItems * 2));

  const visibleItems = $derived(items.slice(startIndex, endIndex));
  const offsetY = $derived(startIndex * itemHeight);

  function handleScroll(event: Event) {
    const target = event.target as HTMLDivElement;
    scrollTop = target.scrollTop;
  }

  function updateContainerHeight() {
    if (containerElement) {
      containerHeight = containerElement.clientHeight;
    }
  }

  onMount(() => {
    updateContainerHeight();

    const resizeObserver = new ResizeObserver(() => {
      updateContainerHeight();
    });

    if (containerElement) {
      resizeObserver.observe(containerElement);
    }

    return () => {
      resizeObserver.disconnect();
    };
  });
</script>

<div
  bind:this={containerElement}
  class="relative overflow-y-auto h-full"
  onscroll={handleScroll}
>
  <div style="height: {totalHeight}px; position: relative;">
    <div
      class="absolute w-full"
      style="transform: translateY({offsetY}px);"
    >
      {@render children({ visibleItems, startIndex, offsetY, endIndex })}
    </div>
  </div>
</div>
