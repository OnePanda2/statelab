/**
 * Feature Analysis module (§6.1).
 *
 * A display/inspection surface over the System-Specific Metrics **already computed
 * and embedded** in the Trajectory Object. It performs no computation of its own —
 * it reads `trajectory.system_specific_metrics`, formats each value, and renders
 * the FROZEN "Metric Not Supported" fallback for any absent key (§5.2). Sourced
 * only from the Trajectory Object (Phase 6 DoD).
 */

import type { Trajectory } from '@/types/trajectory';
import {
  COLLATZ_METRICS,
  METRIC_GROUPS,
  formatMetricValue,
  isNotApplicable,
  isSupported,
} from './metrics';

export interface FeatureAnalysisProps {
  trajectory: Trajectory;
}

export function FeatureAnalysis({ trajectory }: FeatureAnalysisProps): JSX.Element {
  const metrics = trajectory.system_specific_metrics;

  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
      {METRIC_GROUPS.map((group) => {
        const items = COLLATZ_METRICS.filter((m) => m.group === group);
        const wide = group === 'Sequences';
        return (
          <div
            key={group}
            className={`rounded-xl border border-slate-700/60 bg-slate-900/40 p-4 ${
              wide ? 'md:col-span-2' : ''
            }`}
          >
            <h3 className="mb-2 text-xs uppercase tracking-wide text-slate-400">{group}</h3>
            <table className="w-full border-collapse">
              <tbody>
                {items.map((m) => {
                  const value = metrics[m.key];
                  const supported = isSupported(value);
                  const na = isNotApplicable(value);
                  const valueClass = !supported
                    ? 'italic text-slate-500'
                    : na
                      ? 'text-slate-500'
                      : 'text-slate-200';
                  return (
                    <tr key={m.key} className="border-b border-slate-700/40 last:border-0">
                      <td className="whitespace-nowrap py-1.5 pr-4 align-top text-slate-400">
                        {m.label}
                      </td>
                      <td
                        className={`py-1.5 text-right align-top font-mono text-xs ${valueClass} ${
                          wide ? 'break-all text-left' : ''
                        }`}
                        data-metric={m.key}
                      >
                        {formatMetricValue(value)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        );
      })}
    </div>
  );
}
