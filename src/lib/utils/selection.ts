/**
 * Ids from `fromId` to `toId` inclusive, in list order (either direction). Falls back to
 * just `toId` when `fromId` is null or no longer in the list — a range from nowhere.
 */
export function rangeIds(orderedIds: number[], fromId: number | null, toId: number): number[] {
  const to = orderedIds.indexOf(toId);
  if (to < 0) return [];
  const from = fromId === null ? -1 : orderedIds.indexOf(fromId);
  if (from < 0) return [toId];
  const [lo, hi] = from <= to ? [from, to] : [to, from];
  return orderedIds.slice(lo, hi + 1);
}

/**
 * The items an action on `item` applies to: the whole selection, in list order, when
 * `item` is part of a multi-selection — otherwise just `item`. Right-click or drag an
 * unselected row and only that row is affected, as in a file manager.
 */
export function actionTargets<T extends { id: number }>(
  items: T[],
  selected: { has(id: number): boolean; size: number },
  item: T,
): T[] {
  if (selected.size < 2 || !selected.has(item.id)) return [item];
  return items.filter((i) => selected.has(i.id));
}
