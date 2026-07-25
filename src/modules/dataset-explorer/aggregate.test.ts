import { describe, expect, it } from 'vitest';
import type { DatasetSummaryRow } from '@/lib/invoke';
import { accumulate, compareDecimal, emptyAggregate, meanIterations } from './aggregate';

function row(partial: Partial<DatasetSummaryRow>): DatasetSummaryRow {
  return {
    initial_state: '1',
    iteration_count: 0,
    status: 'Converged',
    peak_value: null,
    stopping_time: null,
    total_stopping_time: null,
    odd_count: null,
    even_count: null,
    maximum_bit_length: null,
    ...partial,
  };
}

describe('compareDecimal', () => {
  it('orders by magnitude, not lexically', () => {
    expect(compareDecimal('9', '100')).toBe(-1);
    expect(compareDecimal('100', '99')).toBe(1);
    expect(compareDecimal('16', '16')).toBe(0);
  });
  it('ignores leading zeros', () => {
    expect(compareDecimal('007', '7')).toBe(0);
    expect(compareDecimal('050', '9')).toBe(1);
  });
  it('handles very large values beyond f64', () => {
    const big = '1267650600228229401496703205376'; // 2^100
    expect(compareDecimal(big, '9999999999999999')).toBe(1);
  });
});

describe('accumulate', () => {
  it('folds rows into fixed-size aggregate state', () => {
    const agg = emptyAggregate();
    accumulate(agg, row({ initial_state: '3', iteration_count: 7, status: 'Converged', peak_value: '16' }));
    accumulate(agg, row({ initial_state: '27', iteration_count: 111, status: 'Converged', peak_value: '9232' }));
    accumulate(agg, row({ initial_state: '9', iteration_count: 19, status: 'IterationLimitReached', peak_value: '52' }));

    expect(agg.count).toBe(3);
    expect(agg.converged).toBe(2);
    expect(agg.iterationLimit).toBe(1);
    expect(agg.cycleDetected).toBe(0);
    expect(agg.systemError).toBe(0);
    expect(agg.sumIterations).toBe(7 + 111 + 19);
    expect(agg.maxIterations).toBe(111);
    expect(agg.longestInitialState).toBe('27');
    expect(agg.largestPeak).toBe('9232');
    expect(meanIterations(agg)).toBeCloseTo((7 + 111 + 19) / 3, 10);
  });

  it('meanIterations is 0 for an empty aggregate', () => {
    expect(meanIterations(emptyAggregate())).toBe(0);
  });

  it('counts every status kind', () => {
    const agg = emptyAggregate();
    accumulate(agg, row({ status: 'Converged' }));
    accumulate(agg, row({ status: 'CycleDetected' }));
    accumulate(agg, row({ status: 'IterationLimitReached' }));
    accumulate(agg, row({ status: 'SystemError' }));
    expect([agg.converged, agg.cycleDetected, agg.iterationLimit, agg.systemError]).toEqual([1, 1, 1, 1]);
  });
});
