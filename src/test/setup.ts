/**
 * Vitest setup: jsdom implements neither the canvas 2D context nor
 * `ResizeObserver`. Chart components guard against both, but stubbing them lets the
 * draw path actually execute under test (exercising `drawTrajectory`) without
 * jsdom "not implemented" noise.
 */

import { vi } from 'vitest';

function make2dContextStub(): CanvasRenderingContext2D {
  const noop = (): void => {};
  return {
    clearRect: noop,
    beginPath: noop,
    moveTo: noop,
    lineTo: noop,
    stroke: noop,
    arc: noop,
    fill: noop,
    setTransform: noop,
    strokeStyle: '',
    fillStyle: '',
    lineWidth: 1,
  } as unknown as CanvasRenderingContext2D;
}

HTMLCanvasElement.prototype.getContext = vi.fn(() => make2dContextStub()) as never;

globalThis.ResizeObserver = class {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
} as never;
