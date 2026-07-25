/**
 * Export metadata block (§6.4).
 *
 * **Every export must embed this exact FROZEN list** — Application Version,
 * Engine Version, Trajectory Schema Version, Visualization Version, Iteration
 * Limit, Cycle Detection, Dataset Definition, Rendering Parameters, Timestamp,
 * Platform Information. All values are read from the Trajectory Object or from
 * app constants; nothing is recomputed.
 */

import type { Trajectory } from '@/types/trajectory';

/** Application version (mirrors package.json). */
export const APPLICATION_VERSION = '0.1.0';
/** Version of the visualization layer, bumped when rendering semantics change. */
export const VISUALIZATION_VERSION = '1.0.0';
/** The cycle-detection algorithm the engine uses (§4.6 IMPLEMENTATION DECISION). */
export const CYCLE_DETECTION_ALGORITHM = 'hash-indexed-visited-set';
/**
 * Schema version stamped on dataset exports, where no single Trajectory Object is
 * retained to read it from. Mirrors {@link TRAJECTORY_SCHEMA_VERSION}.
 */
export const TRAJECTORY_SCHEMA_VERSION_FOR_EXPORT = '1.0.0';

/** Describes what data an export covers (a single run, or a generated dataset). */
export type DatasetDefinition =
  | { type: 'single'; initial_state: string }
  | { type: 'comparison'; initial_states: string[] }
  | { type: string; [key: string]: unknown };

/** Free-form record of how a visual export was drawn. */
export type RenderingParameters = Record<string, unknown>;

/** The FROZEN metadata block embedded in every export (§6.4). */
export interface ExportMetadata {
  application_version: string;
  engine_version: string;
  trajectory_schema_version: string;
  visualization_version: string;
  iteration_limit: number;
  cycle_detection: { algorithm: string; bound: number };
  dataset_definition: DatasetDefinition;
  rendering_parameters: RenderingParameters;
  timestamp: string;
  platform_information: { platform: string };
}

/** The exact FROZEN field order/name list — used by exporters and tests. */
export const REQUIRED_METADATA_FIELDS = [
  'application_version',
  'engine_version',
  'trajectory_schema_version',
  'visualization_version',
  'iteration_limit',
  'cycle_detection',
  'dataset_definition',
  'rendering_parameters',
  'timestamp',
  'platform_information',
] as const;

/**
 * Builds the metadata block for an export. Engine version, schema version,
 * iteration limit, and platform come from the Trajectory Object itself, so an
 * export always records the conditions that actually produced its data.
 */
export function buildExportMetadata(
  trajectory: Trajectory,
  datasetDefinition: DatasetDefinition,
  renderingParameters: RenderingParameters = {},
  now: Date = new Date(),
): ExportMetadata {
  const limit = trajectory.execution_metadata.iteration_limit_used;
  return {
    application_version: APPLICATION_VERSION,
    engine_version: trajectory.execution_metadata.engine_version,
    trajectory_schema_version: trajectory.trajectory_schema_version,
    visualization_version: VISUALIZATION_VERSION,
    iteration_limit: limit,
    cycle_detection: { algorithm: CYCLE_DETECTION_ALGORITHM, bound: limit },
    dataset_definition: datasetDefinition,
    rendering_parameters: renderingParameters,
    timestamp: now.toISOString(),
    platform_information: { platform: trajectory.execution_metadata.platform },
  };
}

/**
 * Builds the metadata block for a **dataset** export, where there is no single
 * Trajectory Object to source from (trajectories are streamed and released). The
 * engine/schema versions and platform are supplied by the caller from the stream's
 * own context; every FROZEN field is still present.
 */
export function buildDatasetExportMetadata(
  datasetDefinition: DatasetDefinition,
  iterationLimit: number,
  context: { engineVersion: string; schemaVersion: string; platform: string },
  renderingParameters: RenderingParameters = {},
  now: Date = new Date(),
): ExportMetadata {
  return {
    application_version: APPLICATION_VERSION,
    engine_version: context.engineVersion,
    trajectory_schema_version: context.schemaVersion,
    visualization_version: VISUALIZATION_VERSION,
    iteration_limit: iterationLimit,
    cycle_detection: { algorithm: CYCLE_DETECTION_ALGORITHM, bound: iterationLimit },
    dataset_definition: datasetDefinition,
    rendering_parameters: renderingParameters,
    timestamp: now.toISOString(),
    platform_information: { platform: context.platform },
  };
}

/** Renders the metadata block as `# key: value` comment lines (for CSV). */
export function metadataAsComments(metadata: ExportMetadata, prefix = '# '): string {
  return REQUIRED_METADATA_FIELDS.map((key) => {
    const value = metadata[key];
    const rendered = typeof value === 'object' ? JSON.stringify(value) : String(value);
    return `${prefix}${key}: ${rendered}`;
  }).join('\n');
}
