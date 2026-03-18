<script lang="ts">
  import { onMount, type Snippet } from 'svelte';

  interface Props {
    items: any[];
    itemHeight: number;
    bufferItems?: number;
    children: Snippet<
      [{ visibleItems: any[]; startIndex: number; offsetY: number; endIndex: number }]
    >;
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

  // Scroll to make an index visible, with optional buffer items above/below
  export function scrollToIndex(index: number, buffer: number = 1) {
    if (!containerElement || index < 0 || index >= totalItems) return;

    const itemTop = index * itemHeight;
    const itemBottom = itemTop + itemHeight;
    const viewTop = scrollTop;
    const viewBottom = scrollTop + containerHeight;

    // Calculate desired position with buffer
    const bufferPx = buffer * itemHeight;

    if (itemTop < viewTop + bufferPx) {
      // Item is above visible area (or too close to top) - scroll up
      const targetScroll = Math.max(0, itemTop - bufferPx);
      containerElement.scrollTop = targetScroll;
      scrollTop = targetScroll;
    } else if (itemBottom > viewBottom - bufferPx) {
      // Item is below visible area (or too close to bottom) - scroll down
      const targetScroll = Math.min(
        totalHeight - containerHeight,
        itemBottom - containerHeight + bufferPx,
      );
      containerElement.scrollTop = targetScroll;
      scrollTop = targetScroll;
    }
    // Otherwise item is already visible with buffer - do nothing
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

<div bind:this={containerElement} class="relative overflow-y-auto h-full" onscroll={handleScroll}>
  <div style="height: {totalHeight}px; position: relative;">
    <div class="absolute w-full" style="transform: translateY({offsetY}px);">
      {@render children({ visibleItems, startIndex, offsetY, endIndex })}
    </div>
  </div>
</div>
