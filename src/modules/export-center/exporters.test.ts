import { describe, expect, it } from 'vitest';
import { APPENDIX_B_METRICS, makeTrajectory } from '@/test/fixtures';
import {
  buildExportMetadata,
  metadataAsComments,
  REQUIRED_METADATA_FIELDS,
  type ExportMetadata,
} from './metadata';
import {
  embedPngMetadata,
  readPngTextChunk,
  toCsv,
  toJson,
  toSvg,
} from './exporters';

const TRAJECTORY = makeTrajectory(['3', '10', '5', '16', '8', '4', '2', '1'], {
  system_specific_metrics: APPENDIX_B_METRICS,
});

function metadata(): ExportMetadata {
  return buildExportMetadata(
    TRAJECTORY,
    { type: 'single', initial_state: '3' },
    { chart: 'value', scale: 1.0 },
    new Date('2026-07-25T00:00:00Z'),
  );
}

describe('buildExportMetadata', () => {
  it('contains every FROZEN field (§6.4)', () => {
    const m = metadata() as unknown as Record<string, unknown>;
    for (const field of REQUIRED_METADATA_FIELDS) {
      expect(m[field], `missing metadata field: ${field}`).toBeDefined();
    }
  });

  it('sources engine/schema/limit/platform from the Trajectory Object', () => {
    const m = metadata();
    expect(m.engine_version).toBe(TRAJECTORY.execution_metadata.engine_version);
    expect(m.trajectory_schema_version).toBe(TRAJECTORY.trajectory_schema_version);
    expect(m.iteration_limit).toBe(TRAJECTORY.execution_metadata.iteration_limit_used);
    expect(m.platform_information.platform).toBe(TRAJECTORY.execution_metadata.platform);
  });

  it('records the cycle-detection algorithm and its bound', () => {
    const m = metadata();
    expect(m.cycle_detection.algorithm).toBe('hash-indexed-visited-set');
    expect(m.cycle_detection.bound).toBe(m.iteration_limit);
  });
});

describe('JSON export', () => {
  it('embeds the full metadata block alongside the trajectory', () => {
    const parsed = JSON.parse(toJson(TRAJECTORY, metadata())) as Record<string, never>;
    expect(parsed['trajectory']).toBeDefined();
    const m = parsed['metadata'] as unknown as Record<string, unknown>;
    for (const field of REQUIRED_METADATA_FIELDS) {
      expect(m[field], `JSON missing ${field}`).toBeDefined();
    }
  });
});

describe('CSV export', () => {
  const csv = toCsv(TRAJECTORY, metadata());

  it('embeds every metadata field as a comment line', () => {
    for (const field of REQUIRED_METADATA_FIELDS) {
      expect(csv, `CSV missing ${field}`).toContain(`# ${field}:`);
    }
  });

  it('writes a header and one row per state', () => {
    const lines = csv.trim().split('\n');
    const dataLines = lines.filter((l) => !l.startsWith('#'));
    expect(dataLines[0]).toBe('index,state,bit_length,parity');
    expect(dataLines.length).toBe(1 + TRAJECTORY.state_sequence.length);
    expect(dataLines[1]).toBe('0,3,2,1'); // index 0, state 3, bits 2, parity 1
  });

  it('leaves trailing parity blank (one fewer parity bit than states)', () => {
    const last = csv.trim().split('\n').at(-1);
    expect(last).toBe('7,1,1,'); // final state has no outgoing transition
  });
});

describe('SVG export', () => {
  const svg = toSvg(TRAJECTORY, metadata());

  it('is a standalone SVG document', () => {
    expect(svg).toContain('<svg');
    expect(svg).toContain('xmlns="http://www.w3.org/2000/svg"');
    expect(svg).toContain('<polyline');
  });

  it('embeds every metadata field inside <metadata>', () => {
    expect(svg).toContain('<metadata>');
    for (const field of REQUIRED_METADATA_FIELDS) {
      expect(svg, `SVG missing ${field}`).toContain(field);
    }
  });
});

describe('PNG metadata embedding', () => {
  /** A minimal but structurally valid PNG: signature + IHDR-ish + IEND. */
  function fakePng(): Uint8Array {
    const sig = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    const iend = [0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82];
    return Uint8Array.from([...sig, ...iend]);
  }

  it('inserts a readable tEXt chunk containing the full metadata', () => {
    const out = embedPngMetadata(fakePng(), metadata());
    const text = readPngTextChunk(out, 'StateLab');
    expect(text).not.toBeNull();
    const m = JSON.parse(text ?? '{}') as Record<string, unknown>;
    for (const field of REQUIRED_METADATA_FIELDS) {
      expect(m[field], `PNG missing ${field}`).toBeDefined();
    }
  });

  it('keeps the PNG valid: signature intact and IEND still last', () => {
    const out = embedPngMetadata(fakePng(), metadata());
    expect([...out.subarray(0, 8)]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    const tail = String.fromCharCode(...out.subarray(out.length - 8, out.length - 4));
    expect(tail).toBe('IEND');
  });

  it('rejects non-PNG input rather than corrupting it', () => {
    expect(() => embedPngMetadata(Uint8Array.from([1, 2, 3]), metadata())).toThrow(/not a PNG/);
  });
});

describe('metadataAsComments', () => {
  it('renders one prefixed line per FROZEN field', () => {
    const lines = metadataAsComments(metadata()).split('\n');
    expect(lines.length).toBe(REQUIRED_METADATA_FIELDS.length);
    expect(lines[0]).toContain('application_version');
  });
});
