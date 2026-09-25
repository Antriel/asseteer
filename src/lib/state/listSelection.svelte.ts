import { SvelteSet } from 'svelte/reactivity';
import { actionTargets, rangeIds } from '$lib/utils/selection';

/**
 * Multi-selection for one list, with file-manager semantics: Ctrl+click toggles,
 * Shift+click selects a range from the anchor (the last plain or Ctrl click).
 *
 * Per list, not a singleton: each list component creates its own.
 */
export class ListSelection {
  readonly ids = new SvelteSet<number>();
  anchor = $state<number | null>(null);

  get size(): number {
    return this.ids.size;
  }

  has(id: number): boolean {
    return this.ids.has(id);
  }

  /** Select just `id` (plain click / keyboard navigation). */
  only(id: number) {
    this.ids.clear();
    this.ids.add(id);
    this.anchor = id;
  }

  /** Nothing selected, but remember `id` as where a later Shift+click range starts. */
  clearAt(id: number | null) {
    this.ids.clear();
    this.anchor = id;
  }

  toggle(id: number) {
    if (this.ids.has(id)) this.ids.delete(id);
    else this.ids.add(id);
    this.anchor = id;
  }

  /** Replace the selection with the range anchor..`id`; the anchor stays put. */
  extendTo(orderedIds: number[], id: number) {
    const range = rangeIds(orderedIds, this.anchor, id);
    this.ids.clear();
    for (const r of range) this.ids.add(r);
    if (this.anchor === null || !orderedIds.includes(this.anchor)) this.anchor = id;
  }

  /** Drop ids no longer in the list (after a new search, a filter change). */
  retain(orderedIds: number[]) {
    if (this.ids.size === 0) return;
    const present = new Set(orderedIds);
    for (const id of [...this.ids]) if (!present.has(id)) this.ids.delete(id);
    if (this.anchor !== null && !present.has(this.anchor)) this.anchor = null;
  }

  /** The selected items, in list order. */
  items<T extends { id: number }>(items: T[]): T[] {
    return this.ids.size === 0 ? [] : items.filter((i) => this.ids.has(i.id));
  }

  /** What an action (drag, Copy Path) on `item` applies to — see `actionTargets`. */
  targets<T extends { id: number }>(items: T[], item: T): T[] {
    return actionTargets(items, this.ids, item);
  }
}
