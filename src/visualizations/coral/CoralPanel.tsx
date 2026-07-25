/**
 * Coral control panel (§5.5): exposes all six FROZEN parameters (odd angle, even
 * angle, line length, opacity, scale, rotation) and the five direction rules, and
 * renders the live [`Coral`](./Coral) canvas. Holds only UI state; computes no
 * trajectory mathematics.
 */

import { useMemo, useState } from 'react';
import type { Trajectory } from '@/types/trajectory';
import { Coral } from './Coral';
import { DIRECTION_RULES, type CoralParams, type DirectionRule } from './coralPath';

export interface CoralPanelProps {
  trajectory: Trajectory;
}

export function CoralPanel({ trajectory }: CoralPanelProps): JSX.Element {
  const [oddAngle, setOddAngle] = useState(18);
  const [evenAngle, setEvenAngle] = useState(-16);
  const [lineLength, setLineLength] = useState(6);
  const [opacity, setOpacity] = useState(0.85);
  const [scale, setScale] = useState(1);
  const [rotation, setRotation] = useState(-90);
  const [rule, setRule] = useState<DirectionRule>('relative');

  const params: CoralParams = useMemo(
    () => ({ oddAngle, evenAngle, lineLength, rotation }),
    [oddAngle, evenAngle, lineLength, rotation],
  );

  return (
    <section>
      <div className="mb-3 flex flex-wrap items-end gap-x-5 gap-y-3">
        <Slider label="Odd angle" value={oddAngle} min={-180} max={180} step={1} suffix="°" onChange={setOddAngle} />
        <Slider label="Even angle" value={evenAngle} min={-180} max={180} step={1} suffix="°" onChange={setEvenAngle} />
        <Slider label="Line length" value={lineLength} min={1} max={30} step={1} onChange={setLineLength} />
        <Slider label="Opacity" value={opacity} min={0.05} max={1} step={0.05} onChange={setOpacity} />
        <Slider label="Scale" value={scale} min={0.2} max={4} step={0.1} suffix="×" onChange={setScale} />
        <Slider label="Rotation" value={rotation} min={-180} max={180} step={1} suffix="°" onChange={setRotation} />
        <label className="flex flex-col gap-1">
          <span className="text-xs uppercase tracking-wide text-slate-400">Direction rule</span>
          <select
            className="rounded-lg border border-slate-700 bg-slate-800 px-2 py-1.5 text-sm outline-none focus:border-sky-500"
            value={rule}
            onChange={(e) => setRule(e.target.value as DirectionRule)}
          >
            {DIRECTION_RULES.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      <Coral trajectory={trajectory} params={params} rule={rule} opacity={opacity} scale={scale} />

      <p className="mt-2 text-xs text-slate-500">
        <span className="text-orange-400">■</span> odd-parity segment ·{' '}
        <span className="text-emerald-400">■</span> even-parity segment · driven by the trajectory&apos;s
        pre-computed parity sequence.
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
    <label className="flex flex-col gap-1">
      <span className="text-xs uppercase tracking-wide text-slate-400">
        {label} <span className="text-slate-300">{value}{suffix ?? ''}</span>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-32 accent-sky-500"
      />
    </label>
  );
}
