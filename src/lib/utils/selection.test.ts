import { describe, expect, it } from 'vitest';
import { actionTargets, rangeIds } from './selection';

describe('rangeIds', () => {
  const ids = [10, 20, 30, 40, 50];

  it('spans anchor to target in list order, either direction', () => {
    expect(rangeIds(ids, 20, 40)).toEqual([20, 30, 40]);
    expect(rangeIds(ids, 40, 20)).toEqual([20, 30, 40]);
    expect(rangeIds(ids, 30, 30)).toEqual([30]);
  });

  it('without a usable anchor selects only the target', () => {
    expect(rangeIds(ids, null, 30)).toEqual([30]);
    expect(rangeIds(ids, 99, 30)).toEqual([30]);
  });

  it('ignores a target that is not in the list', () => {
    expect(rangeIds(ids, 20, 99)).toEqual([]);
  });
});

describe('actionTargets', () => {
  const items = [{ id: 1 }, { id: 2 }, { id: 3 }, { id: 4 }];

  it('uses the whole selection, in list order, when the item is in it', () => {
    expect(actionTargets(items, new Set([4, 2]), items[3])).toEqual([items[1], items[3]]);
  });

  it('uses just the item when it is outside the selection', () => {
    expect(actionTargets(items, new Set([2, 4]), items[0])).toEqual([items[0]]);
  });

  it('uses just the item for a single selection', () => {
    expect(actionTargets(items, new Set([2]), items[1])).toEqual([items[1]]);
  });
});
