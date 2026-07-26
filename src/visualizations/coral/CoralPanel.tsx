/**
 * Coral control panel (§5.5): exposes all six FROZEN parameters (odd angle, even
 * angle, line length, opacity, scale, rotation) and the direction rules, and
 * renders the live [`Coral`](./Coral) canvas over every accumulated trajectory.
 *
 * Trajectories accumulate until **Reset** is pressed, so successive runs overlay.
 * That is what the `aesthetic` rule needs to form a tree — one trajectory is only
 * ever one branch.
 *
 * Holds UI state only. It **cannot** trigger computation: the bulk-add control
 * calls `onAddRange`, and the owning layer runs it through the Research
 * Controller (§5.1). This panel never imports an engine path (§5.2) — an ESLint
 * boundary rule enforces that.
 */

import { useMemo, useState } from 'react';
import type { Trajectory } from '@/types/trajectory';
import { Coral, CORAL_EVEN_COLOR, CORAL_ODD_COLOR } from './Coral';
import { DIRECTION_RULES, type CoralParams, type DirectionRule } from './coralPath';

export interface CoralPanelProps {
  /** Every trajectory drawn so far, most recent last. */
  trajectories: Trajectory[];
  /** Clears the accumulated overlay. */
  onReset: () => void;
  /** Asks the owner to run `from..to` and append the results. */
  onAddRange: (from: number, to: number) => void;
  /** Percent complete of an in-flight bulk add, or `null` when idle. */
  progress: number | null;
  /** Error text from the owner's last bulk add, if any. */
  runError?: string | null;
}

/** Upper bound on a single bulk add, so a typo cannot launch a runaway job. */
const MAX_BULK = 5000;

export function CoralPanel({
  trajectories,
  onReset,
  onAddRange,
  progress,
  runError,
}: CoralPanelProps): JSX.Element {
  const [oddAngle, setOddAngle] = useState(10);
  const [evenAngle, setEvenAngle] = useState(-8);
  const [lineLength, setLineLength] = useState(6);
  const [opacity, setOpacity] = useState(0.85);
  const [scale, setScale] = useState(1);
  const [rotation, setRotation] = useState(-90);
  const [rule, setRule] = useState<DirectionRule>('relative');
  const [bulkFrom, setBulkFrom] = useState('1');
  const [bulkTo, setBulkTo] = useState('500');
  const [error, setError] = useState<string | null>(null);

  const params: CoralParams = useMemo(
    () => ({ oddAngle, evenAngle, lineLength, rotation }),
    [oddAngle, evenAngle, lineLength, rotation],
  );

  const aesthetic = rule === 'aesthetic';

  /**
   * Switching rules also seeds angles that suit the chosen rule. For the tree,
   * `even ≈ −odd/2` balances the roughly 2:1 excess of even steps so the trunk
   * grows straight instead of curling into a spiral. Applied only on the switch,
   * so any later hand-tuning is preserved.
   */
  function changeRule(next: DirectionRule): void {
    if (next === 'aesthetic' && rule !== 'aesthetic') {
      // Collatz runs roughly 37% odd / 63% even, so 17 and -10 make the mean
      // turn per step ~0 and the trunk grows straight.
      setOddAngle(17);
      setEvenAngle(-10);
      setLineLength(6);
    }
    setRule(next);
  }

  /** Validates the range, then hands it to the owner to execute. */
  function requestRange(): void {
    const from = Math.max(1, Number(bulkFrom) || 1);
    const to = Number(bulkTo) || from;
    if (to < from) {
      setError('"To" must be greater than or equal to "From".');
      return;
    }
    const count = to - from + 1;
    if (count > MAX_BULK) {
      setError(
        `That is ${count.toLocaleString()} trajectories; the limit is ${MAX_BULK.toLocaleString()}.`,
      );
      return;
    }
    setError(null);
    onAddRange(from, to);
  }

  return (
    <section>
      <div className="mb-4 flex flex-wrap items-end gap-x-6 gap-y-4">
        {/* Half-degree steps: the tree's straightness is sensitive to the odd/even
            balance, and whole degrees are too coarse to tune it. */}
        <Slider label="Odd angle" value={oddAngle} min={-180} max={180} step={0.5} suffix="°" onChange={setOddAngle} />
        <Slider label="Even angle" value={evenAngle} min={-180} max={180} step={0.5} suffix="°" onChange={setEvenAngle} />
        <Slider label="Line length" value={lineLength} min={1} max={30} step={1} onChange={setLineLength} />
        <Slider label="Opacity" value={opacity} min={0.05} max={1} step={0.05} onChange={setOpacity} />
        <Slider label="Scale" value={scale} min={0.2} max={4} step={0.1} suffix="×" onChange={setScale} />
        <Slider label="Rotation" value={rotation} min={-180} max={180} step={1} suffix="°" onChange={setRotation} />
        <label className="sl-field">
          <span className="sl-label">Direction rule</span>
          <select
            className="sl-select"
            value={rule}
            onChange={(e) => changeRule(e.target.value as DirectionRule)}
          >
            {DIRECTION_RULES.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="mb-4 flex flex-wrap items-end gap-4 sl-card">
        <span className="sl-label">Overlay</span>
        <span className="sl-pill sl-pill--neutral">
          {trajectories.length} trajector{trajectories.length === 1 ? 'y' : 'ies'}
        </span>
        <NumberField label="From" value={bulkFrom} onChange={setBulkFrom} disabled={progress !== null} />
        <NumberField label="To" value={bulkTo} onChange={setBulkTo} disabled={progress !== null} />
        <button
          className="sl-btn sl-btn--primary"
          onClick={requestRange}
          disabled={progress !== null}
          data-action="coral-add-range"
        >
          {progress !== null ? `Adding… ${progress}%` : 'Add range'}
        </button>
        <button
          className="sl-btn"
          onClick={onReset}
          disabled={progress !== null || trajectories.length === 0}
          data-action="coral-reset"
        >
          Reset
        </button>
        {(error ?? runError) && (
          <span className="sl-error">{error ?? runError}</span>
        )}
      </div>

      <Coral
        trajectories={trajectories}
        params={params}
        rule={rule}
        opacity={opacity}
        scale={scale}
        height={aesthetic ? 520 : 360}
      />

      <p className="mt-4 sl-hint">
        {aesthetic ? (
          <>
            Each trajectory is traced from its end (the fixed point 1) back to its start, so the shared
            tail <span className="font-mono">…4→2→1</span> becomes a common trunk and the paths fan out.
            The tree only emerges with many overlaid — try <span className="font-mono">Add range 1–500</span>,
            then adjust the angles.
          </>
        ) : (
          <>
            {/* These two swatches must match the canvas stroke colours exactly,
                so they use the renderer's own constants rather than a theme
                token — a legend that drifts from the plot is worse than none. */}
            <span style={{ color: CORAL_ODD_COLOR }}>■</span> odd-parity segment ·{' '}
            <span style={{ color: CORAL_EVEN_COLOR }}>■</span> even-parity segment · driven by each
            trajectory&apos;s pre-computed parity sequence. Every run adds to the overlay until you Reset.
          </>
        )}
      </p>
    </section>
  );
}

interface SliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  suffix?: string;
  onChange: (value: number) => void;
}

function Slider({ label, value, min, max, step, suffix, onChange }: SliderProps): JSX.Element {
  return (
    <label className="sl-field">
      <span className="sl-label">
        {label} <span className="text-[color:var(--sl-text)]">{value}{suffix ?? ''}</span>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="sl-slider"
      />
    </label>
  );
}

function NumberField({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
}): JSX.Element {
  return (
    <label className="sl-field">
      <span className="sl-label">{label}</span>
      <input
        className="w-24 sl-input sl-input--mono"
        value={value}
        inputMode="numeric"
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
      />
    </label>
  );
}
