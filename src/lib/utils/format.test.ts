import { describe, expect, it } from 'vitest';
import { formatDuration, formatDurationCompact, formatFileSize, formatSimilarity } from './format';

describe('formatDuration', () => {
  it('shows milliseconds under ten seconds', () => {
    expect(formatDuration(0)).toBe('0:00.000');
    expect(formatDuration(600)).toBe('0:00.600');
    expect(formatDuration(9999)).toBe('0:09.999');
  });

  it('does not lose a millisecond to float error', () => {
    // 4.3 % 60 - 4 === 0.2999999999999998 in floating point
    expect(formatDuration(4300)).toBe('0:04.300');
    expect(formatDuration(1100)).toBe('0:01.100');
  });

  it('drops milliseconds from ten seconds up', () => {
    expect(formatDuration(10_000)).toBe('0:10');
    expect(formatDuration(65_234)).toBe('1:05');
    expect(formatDuration(3_600_000)).toBe('60:00');
  });
});

describe('formatFileSize', () => {
  it('picks the unit by magnitude', () => {
    expect(formatFileSize(512)).toBe('512 B');
    expect(formatFileSize(1536)).toBe('1.5 KB');
    expect(formatFileSize(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(formatFileSize(2.5 * 1024 ** 3)).toBe('2.5 GB');
  });
});

describe('formatSimilarity', () => {
  it('rounds to a whole percentage', () => {
    expect(formatSimilarity(0.8765)).toBe('88%');
    expect(formatSimilarity(1)).toBe('100%');
  });
});

describe('formatDurationCompact', () => {
  it('shows hundredths of a second under ten seconds', () => {
    expect(formatDurationCompact(300)).toBe('0.30 s');
    expect(formatDurationCompact(349)).toBe('0.35 s');
    expect(formatDurationCompact(3000)).toBe('3.00 s');
    expect(formatDurationCompact(4300)).toBe('4.30 s');
  });

  it('shows tenths from ten seconds to a minute', () => {
    expect(formatDurationCompact(10_000)).toBe('10.0 s');
    expect(formatDurationCompact(12_540)).toBe('12.5 s');
  });

  it('rounds before picking the bracket', () => {
    expect(formatDurationCompact(9996)).toBe('10.0 s');
    expect(formatDurationCompact(59_960)).toBe('1:00');
  });

  it('switches to m:ss from a minute up', () => {
    expect(formatDurationCompact(60_000)).toBe('1:00');
    expect(formatDurationCompact(65_234)).toBe('1:05');
    expect(formatDurationCompact(3_600_000)).toBe('60:00');
  });
});
