import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { APPENDIX_B_METRICS, makeTrajectory } from '@/test/fixtures';
import { Coral } from './Coral';
import type { CoralParams } from './coralPath';

afterEach(cleanup);

const PARAMS: CoralParams = { oddAngle: 18, evenAngle: -16, lineLength: 6, rotation: -90 };

function trajectoryWithParity(initial: string): ReturnType<typeof makeTrajectory> {
  return makeTrajectory([initial, '1'], { system_specific_metrics: APPENDIX_B_METRICS });
}

describe('Coral', () => {
  it('renders a canvas for a single trajectory', () => {
    const { container } = render(
      <Coral
        trajectories={[trajectoryWithParity('3')]}
        params={PARAMS}
        rule="relative"
        opacity={0.85}
        scale={1}
      />,
    );
    expect(container.querySelector('canvas')).not.toBeNull();
  });

  it('overlays many trajectories on one canvas', () => {
    const many = ['3', '7', '27', '97'].map(trajectoryWithParity);
    const { container } = render(
      <Coral trajectories={many} params={PARAMS} rule="aesthetic" opacity={0.6} scale={1} />,
    );
    const canvas = container.querySelector('canvas');
    expect(canvas).not.toBeNull();
    expect(canvas?.getAttribute('aria-label')).toContain('4 trajectories');
  });

  it('prompts to run when the overlay is empty', () => {
    const { container } = render(
      <Coral trajectories={[]} params={PARAMS} rule="relative" opacity={1} scale={1} />,
    );
    expect(container.querySelector('canvas')).toBeNull();
    expect(container.textContent).toContain('Run a trajectory');
  });

  it('falls back to "Metric Not Supported" when parity is absent (§5.2)', () => {
    const noParity = makeTrajectory(['3', '1'], { system_specific_metrics: {} });
    const { container } = render(
      <Coral trajectories={[noParity]} params={PARAMS} rule="relative" opacity={1} scale={1} />,
    );
    expect(container.textContent).toContain('Metric Not Supported');
    expect(container.querySelector('canvas')).toBeNull();
  });

  it('ignores trajectories missing parity while still drawing the rest', () => {
    const mixed = [
      trajectoryWithParity('3'),
      makeTrajectory(['9', '1'], { system_specific_metrics: {} }),
    ];
    const { container } = render(
      <Coral trajectories={mixed} params={PARAMS} rule="aesthetic" opacity={1} scale={1} />,
    );
    // One usable trajectory remains, so the canvas renders rather than falling back.
    expect(container.querySelector('canvas')).not.toBeNull();
  });
});
