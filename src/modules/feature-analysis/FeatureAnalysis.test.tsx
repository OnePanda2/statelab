import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { APPENDIX_B_METRICS, makeTrajectory } from '@/test/fixtures';
import { COLLATZ_METRICS } from './metrics';
import { FeatureAnalysis } from './FeatureAnalysis';

afterEach(cleanup);

function cell(container: HTMLElement, key: string): string {
  return container.querySelector(`[data-metric="${key}"]`)?.textContent ?? '';
}

describe('FeatureAnalysis', () => {
  it('renders every Collatz metric from the Trajectory (Appendix B n=3)', () => {
    const { container } = render(
      <FeatureAnalysis
        trajectory={makeTrajectory(['3', '10', '5', '16', '8', '4', '2', '1'], {
          system_specific_metrics: APPENDIX_B_METRICS,
        })}
      />,
    );
    // All 15 descriptors have a rendered cell.
    for (const m of COLLATZ_METRICS) {
      expect(container.querySelector(`[data-metric="${m.key}"]`)).not.toBeNull();
    }
    // Spot-check exact values, sourced only from the Trajectory.
    expect(cell(container, 'peak_value')).toBe('16');
    expect(cell(container, 'total_stopping_time')).toBe('7');
    expect(cell(container, 'parity_sequence')).toBe('[1, 0, 1, 0, 0, 0, 0]');
    expect(cell(container, 'binary_transition_statistics')).toBe(
      'increases: 2, decreases: 5, same: 0',
    );
    expect(cell(container, 'average_decline')).toBe('0.500000');
  });

  it('renders "Metric Not Supported" for a deliberately missing key (§7.5)', () => {
    // Remove one metric to exercise the fallback path.
    const partial = { ...APPENDIX_B_METRICS };
    delete partial['peak_value'];
    const { container } = render(
      <FeatureAnalysis
        trajectory={makeTrajectory(['3', '1'], { system_specific_metrics: partial })}
      />,
    );
    expect(cell(container, 'peak_value')).toBe('Metric Not Supported');
    // Neighbouring metrics still render normally — no blank, no crash.
    expect(cell(container, 'peak_index')).toBe('3');
  });

  it('renders N/A for a present-but-null metric', () => {
    const { container } = render(
      <FeatureAnalysis
        trajectory={makeTrajectory(['1', '4', '2', '1'], {
          system_specific_metrics: { ...APPENDIX_B_METRICS, stopping_time: null },
        })}
      />,
    );
    expect(cell(container, 'stopping_time')).toBe('N/A');
  });

  it('shows "Metric Not Supported" for every metric when none are present', () => {
    const { container } = render(
      <FeatureAnalysis trajectory={makeTrajectory(['1'], { system_specific_metrics: {} })} />,
    );
    for (const m of COLLATZ_METRICS) {
      expect(cell(container, m.key)).toBe('Metric Not Supported');
    }
  });
});
