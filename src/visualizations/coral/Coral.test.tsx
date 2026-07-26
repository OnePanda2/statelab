import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { APPENDIX_B_METRICS, makeTrajectory } from '@/test/fixtures';
import { Coral, DEFAULT_EVEN_COLOR, DEFAULT_LINE_WIDTH, DEFAULT_ODD_COLOR } from './Coral';
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

// ---- Post-audit parameters (§5.5): line width, colour, centre offset, animation ----

/**
 * Records every 2D-context call so a render can be asserted against the exact
 * drawing operations it performed — the only way to prove "pixel-identical"
 * without a real canvas.
 */
function recordingContext(): { calls: string[]; ctx: CanvasRenderingContext2D } {
  const calls: string[] = [];
  const rec =
    (name: string) =>
    (...args: unknown[]): void => {
      calls.push(`${name}(${args.map((a) => (typeof a === 'number' ? a.toFixed(4) : a)).join(',')})`);
    };
  const state: Record<string, unknown> = {};
  const ctx = new Proxy(
    {},
    {
      get(_t, prop: string) {
        if (
          [
            'clearRect',
            'fillRect',
            'beginPath',
            'closePath',
            'moveTo',
            'lineTo',
            'stroke',
            'arc',
            'fill',
            'setTransform',
            'save',
            'restore',
          ].includes(prop)
        ) {
          return rec(prop);
        }
        return state[prop];
      },
      set(_t, prop: string, value: unknown) {
        state[prop] = value;
        calls.push(`${prop}=${String(value)}`);
        return true;
      },
    },
  ) as CanvasRenderingContext2D;
  return { calls, ctx };
}

function renderRecording(props: Partial<React.ComponentProps<typeof Coral>>): string[] {
  const { calls, ctx } = recordingContext();
  const spy = vi
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(ctx as unknown as never);
  render(
    <Coral
      trajectories={[trajectoryWithParity('3')]}
      params={PARAMS}
      rule="relative"
      opacity={0.85}
      scale={1}
      {...props}
    />,
  );
  spy.mockRestore();
  return calls;
}

describe('Coral — line width', () => {
  it('defaults to the width the analytical mode previously hardcoded (1.2)', () => {
    expect(DEFAULT_LINE_WIDTH).toBe(1.2);
    expect(renderRecording({})).toContain('lineWidth=1.2');
  });

  it('defaults the aesthetic mode to its previous hardcoded 0.7', () => {
    expect(renderRecording({ rule: 'aesthetic' })).toContain('lineWidth=0.7');
  });

  it('changing the slider changes BOTH draw modes', () => {
    expect(renderRecording({ lineWidth: 2.4 })).toContain('lineWidth=2.4');
    // Aesthetic keeps the original 0.7/1.2 ratio: 2.4 * (0.7/1.2) = 1.4.
    expect(renderRecording({ rule: 'aesthetic', lineWidth: 2.4 })).toContain('lineWidth=1.4');
  });
});

describe('Coral — colour', () => {
  it('defaults to the previously hardcoded parity colours', () => {
    expect(DEFAULT_ODD_COLOR).toBe('#f0883e');
    expect(DEFAULT_EVEN_COLOR).toBe('#3fb950');
    const calls = renderRecording({});
    expect(calls).toContain('strokeStyle=#f0883e');
    expect(calls).toContain('strokeStyle=#3fb950');
  });

  it('uses the supplied colours instead', () => {
    const calls = renderRecording({ oddColor: '#ff0000', evenColor: '#0000ff' });
    expect(calls).toContain('strokeStyle=#ff0000');
    expect(calls).toContain('strokeStyle=#0000ff');
    expect(calls).not.toContain('strokeStyle=#f0883e');
  });
});

describe('Coral — centre offset', () => {
  it('(0,0) is pixel-identical to omitting the parameter entirely', () => {
    // The strict backward-compatibility requirement: an explicit zero offset must
    // produce exactly the same draw calls as the pre-existing code path.
    const withoutOffset = renderRecording({});
    const withZeroOffset = renderRecording({ offsetX: 0, offsetY: 0 });
    expect(withZeroOffset).toEqual(withoutOffset);
  });

  it('a non-zero offset translates every drawn point by exactly that amount', () => {
    const base = renderRecording({});
    const shifted = renderRecording({ offsetX: 25, offsetY: -10 });
    const coords = (calls: string[]): number[][] =>
      calls
        .filter((c) => c.startsWith('moveTo(') || c.startsWith('lineTo('))
        .map((c) => c.replace(/^[a-zA-Z]+\(|\)$/g, '').split(',').map(Number));

    const a = coords(base);
    const b = coords(shifted);
    expect(b.length).toBe(a.length);
    expect(a.length).toBeGreaterThan(0);
    a.forEach((p, i) => {
      expect(b[i]?.[0] ?? NaN).toBeCloseTo((p[0] ?? 0) + 25, 3);
      expect(b[i]?.[1] ?? NaN).toBeCloseTo((p[1] ?? 0) - 10, 3);
    });
  });
});

/**
 * Fires the callback exactly once at `timestamp`, then goes quiet. The animation
 * loop re-requests a frame until the path is complete, so an unbounded mock that
 * always invokes its callback synchronously recurses until the stack blows.
 */
// Return type inferred: `ReturnType<typeof vi.spyOn>` erases the callback
// signature and does not accept the concrete rAF spy.
function mockRafOnce(timestamp: number) {
  let fired = false;
  return vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb) => {
    if (!fired) {
      fired = true;
      cb(timestamp);
    }
    return 1;
  });
}

describe('Coral — animation speed', () => {
  it('defaults to instant: the whole path is drawn in one pass, no rAF', () => {
    const raf = vi.spyOn(window, 'requestAnimationFrame');
    const calls = renderRecording({});
    // n=3 has 7 parity bits -> 7 segments -> 8 points -> 7 lineTo per path.
    const segments = calls.filter((c) => c.startsWith('lineTo(')).length;
    expect(segments).toBe(7);
    expect(raf).not.toHaveBeenCalled();
    raf.mockRestore();
  });

  it('with a speed set, the first frame draws only a prefix and rAF drives the rest', () => {
    // Freeze time so the very first frame is at t=0 and nothing is revealed yet.
    const perf = vi.spyOn(performance, 'now').mockReturnValue(1000);
    const raf = mockRafOnce(1000); // same timestamp as the start -> zero elapsed

    const calls = renderRecording({ animationSpeed: 10 });
    const segments = calls.filter((c) => c.startsWith('lineTo(')).length;
    expect(segments).toBe(0); // prefix of length 0 at t=0
    expect(raf).toHaveBeenCalled();

    raf.mockRestore();
    perf.mockRestore();
  });

  it('reaches the full path once enough time has elapsed', () => {
    const perf = vi.spyOn(performance, 'now').mockReturnValue(0);
    // 10 seconds at 10 segments/s reveals 100 segments — well past the 7 here.
    const raf = mockRafOnce(10_000);

    const calls = renderRecording({ animationSpeed: 10 });
    expect(calls.filter((c) => c.startsWith('lineTo(')).length).toBe(7);

    raf.mockRestore();
    perf.mockRestore();
  });
});

describe('Coral — legacy behaviour', () => {
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
