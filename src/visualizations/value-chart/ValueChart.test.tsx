import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { makeTrajectory } from '@/test/fixtures';
import { ValueChart } from './ValueChart';
import { LogChart } from '../log-chart/LogChart';

afterEach(cleanup);

describe('chart components', () => {
  it('ValueChart renders a canvas for the Appendix B n=3 trajectory', () => {
    const { container } = render(
      <ValueChart trajectory={makeTrajectory(['3', '10', '5', '16', '8', '4', '2', '1'])} />,
    );
    const canvas = container.querySelector('canvas');
    expect(canvas).not.toBeNull();
    expect(canvas?.getAttribute('aria-label')).toContain('3');
  });

  it('LogChart renders and does not crash on a single-state trajectory', () => {
    const { container } = render(<LogChart trajectory={makeTrajectory(['1'])} />);
    expect(container.querySelector('canvas')).not.toBeNull();
  });
});
