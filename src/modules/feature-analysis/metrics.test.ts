import { describe, expect, it } from 'vitest';
import { formatMetricValue, isNotApplicable, isSupported } from './metrics';

describe('formatMetricValue', () => {
  it('renders the FROZEN sentinel for an absent (undefined) metric', () => {
    expect(formatMetricValue(undefined)).toBe('Metric Not Supported');
  });

  it('renders N/A for a present-but-null metric', () => {
    expect(formatMetricValue(null)).toBe('N/A');
  });

  it('renders integers and big-integer strings as-is', () => {
    expect(formatMetricValue(6)).toBe('6');
    expect(formatMetricValue('16')).toBe('16');
  });

  it('renders floats to fixed precision', () => {
    expect(formatMetricValue(0.2857142857142857)).toBe('0.285714');
    expect(formatMetricValue(0.5)).toBe('0.500000');
  });

  it('renders arrays', () => {
    expect(formatMetricValue([1, 0, 1, 0, 0, 0, 0])).toBe('[1, 0, 1, 0, 0, 0, 0]');
  });

  it('truncates very long arrays with a count', () => {
    const long = Array.from({ length: 70 }, (_, i) => i % 2);
    const out = formatMetricValue(long);
    expect(out).toContain('… (70 items)');
  });

  it('renders objects as key: value pairs', () => {
    expect(formatMetricValue({ increases: 2, decreases: 5, same: 0 })).toBe(
      'increases: 2, decreases: 5, same: 0',
    );
  });
});

describe('metric support predicates', () => {
  it('distinguishes absent, N/A, and present', () => {
    expect(isSupported(undefined)).toBe(false);
    expect(isSupported(null)).toBe(true);
    expect(isSupported(6)).toBe(true);
    expect(isNotApplicable(null)).toBe(true);
    expect(isNotApplicable(6)).toBe(false);
    expect(isNotApplicable(undefined)).toBe(false);
  });
});
