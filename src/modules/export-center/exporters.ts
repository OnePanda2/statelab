/**
 * Export Center formats (§6.4): **PNG, SVG, CSV, JSON** — each embedding the full
 * FROZEN metadata block from [`metadata`](./metadata).
 *
 * Pure functions over an already-finalized Trajectory Object; no trajectory value
 * is recomputed. **SVG is generated only here, at export time** — it is never used
 * for interactive rendering (§5.6).
 */

import type { Trajectory } from '@/types/trajectory';
import { metadataAsComments, type ExportMetadata } from './metadata';

// ---- JSON ----

/** Serializes the trajectory plus its metadata block as pretty JSON. */
export function toJson(trajectory: Trajectory, metadata: ExportMetadata): string {
  return JSON.stringify({ metadata, trajectory }, null, 2);
}

// ---- CSV ----

/**
 * Serializes the state sequence as CSV. The metadata block is written first as
 * `# key: value` comment lines, so the file is self-describing while remaining
 * readable by any CSV parser that skips comments.
 */
export function toCsv(trajectory: Trajectory, metadata: ExportMetadata): string {
  const header = 'index,state,bit_length,parity';
  const bits = trajectory.system_specific_metrics['bit_length_evolution'];
  const parity = trajectory.system_specific_metrics['parity_sequence'];
  const bitArray = Array.isArray(bits) ? bits : [];
  const parityArray = Array.isArray(parity) ? parity : [];

  const rows = trajectory.state_sequence.map((state, i) => {
    const bit = bitArray[i];
    const par = parityArray[i];
    return [
      String(i),
      state,
      bit === undefined ? '' : String(bit),
      par === undefined ? '' : String(par),
    ].join(',');
  });

  return `${metadataAsComments(metadata)}\n${header}\n${rows.join('\n')}\n`;
}

// ---- SVG (export-time only, §5.6) ----

export interface SvgOptions {
  width?: number;
  height?: number;
  logScale?: boolean;
  stroke?: string;
  background?: string;
}

/**
 * Renders the trajectory as a standalone SVG value chart. The metadata block is
 * embedded in a `<metadata>` element as JSON, so the vector file carries its own
 * provenance. Generated **only at export time** — never rendered interactively.
 */
export function toSvg(
  trajectory: Trajectory,
  metadata: ExportMetadata,
  options: SvgOptions = {},
): string {
  const width = options.width ?? 900;
  const height = options.height ?? 360;
  const stroke = options.stroke ?? '#58a6ff';
  const background = options.background ?? '#0e1116';
  const pad = 24;

  // BigInt string -> f64 only here, at export/render time (§4.5).
  let values = trajectory.state_sequence.map((s) => Number(s));
  if (options.logScale) {
    values = values.map((v) => Math.log10(v + 1));
  }
  const finite = values.filter((v) => Number.isFinite(v));
  const min = finite.length > 0 ? Math.min(...finite, 0) : 0;
  const max = finite.length > 0 ? Math.max(...finite) : 1;
  const span = max - min || 1;
  const n = values.length;

  const points = values
    .map((v, i) => {
      if (!Number.isFinite(v)) {
        return null;
      }
      const x = pad + (n <= 1 ? 0 : (i / (n - 1)) * (width - 2 * pad));
      const y = height - pad - ((v - min) / span) * (height - 2 * pad);
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .filter((p): p is string => p !== null)
    .join(' ');

  const title = `StateLab ${trajectory.system_id} trajectory for n=${escapeXml(
    trajectory.initial_state,
  )}`;

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <title>${title}</title>
  <metadata>
    <statelab:export xmlns:statelab="https://statelab.local/schema/export/1.0.0">
${escapeXml(JSON.stringify(metadata, null, 2))}
    </statelab:export>
  </metadata>
  <rect width="${width}" height="${height}" fill="${background}"/>
  <polyline fill="none" stroke="${stroke}" stroke-width="1.5" points="${points}"/>
</svg>
`;
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ---- PNG (metadata embedded as a tEXt chunk) ----

const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

const CRC_TABLE: number[] = (() => {
  const table = new Array<number>(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc = (CRC_TABLE[(crc ^ byte) & 0xff] ?? 0) ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/** Builds a PNG `tEXt` chunk: `[len][type][keyword\0text][crc]`. */
function textChunk(keyword: string, text: string): Uint8Array {
  const typeAndData: number[] = [];
  for (const ch of 'tEXt') {
    typeAndData.push(ch.charCodeAt(0));
  }
  for (const ch of keyword) {
    typeAndData.push(ch.charCodeAt(0) & 0xff);
  }
  typeAndData.push(0);
  // PNG tEXt is Latin-1; JSON metadata is ASCII-safe, but clamp defensively.
  for (const ch of text) {
    typeAndData.push(ch.charCodeAt(0) & 0xff);
  }
  const body = Uint8Array.from(typeAndData);
  const dataLength = body.length - 4; // exclude the 4-byte type
  const crc = crc32(body);

  const chunk = new Uint8Array(body.length + 8);
  const view = new DataView(chunk.buffer);
  view.setUint32(0, dataLength);
  chunk.set(body, 4);
  view.setUint32(4 + body.length, crc);
  return chunk;
}

/**
 * Inserts the metadata block into PNG bytes as a `tEXt` chunk (keyword
 * `StateLab`), placed immediately before `IEND` so the image stays valid.
 * Throws if the input is not a PNG.
 */
export function embedPngMetadata(png: Uint8Array, metadata: ExportMetadata): Uint8Array {
  for (let i = 0; i < PNG_SIGNATURE.length; i++) {
    if (png[i] !== PNG_SIGNATURE[i]) {
      throw new Error('not a PNG: bad signature');
    }
  }

  // Walk chunks to find IEND's offset.
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength);
  let offset = 8;
  let iendOffset = -1;
  while (offset + 8 <= png.length) {
    const length = view.getUint32(offset);
    const type = String.fromCharCode(
      png[offset + 4] ?? 0,
      png[offset + 5] ?? 0,
      png[offset + 6] ?? 0,
      png[offset + 7] ?? 0,
    );
    if (type === 'IEND') {
      iendOffset = offset;
      break;
    }
    offset += 12 + length; // length + type + data + crc
  }
  if (iendOffset < 0) {
    throw new Error('not a PNG: no IEND chunk');
  }

  const chunk = textChunk('StateLab', JSON.stringify(metadata));
  const out = new Uint8Array(png.length + chunk.length);
  out.set(png.subarray(0, iendOffset), 0);
  out.set(chunk, iendOffset);
  out.set(png.subarray(iendOffset), iendOffset + chunk.length);
  return out;
}

/** Reads back a `tEXt` chunk's payload for the given keyword (used by tests). */
export function readPngTextChunk(png: Uint8Array, keyword: string): string | null {
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength);
  let offset = 8;
  while (offset + 8 <= png.length) {
    const length = view.getUint32(offset);
    const type = String.fromCharCode(
      png[offset + 4] ?? 0,
      png[offset + 5] ?? 0,
      png[offset + 6] ?? 0,
      png[offset + 7] ?? 0,
    );
    if (type === 'tEXt') {
      const data = png.subarray(offset + 8, offset + 8 + length);
      const nul = data.indexOf(0);
      if (nul > 0) {
        const key = String.fromCharCode(...data.subarray(0, nul));
        if (key === keyword) {
          return String.fromCharCode(...data.subarray(nul + 1));
        }
      }
    }
    if (type === 'IEND') {
      break;
    }
    offset += 12 + length;
  }
  return null;
}

/** Decodes a `data:image/png;base64,...` URL into raw bytes. */
export function dataUrlToBytes(dataUrl: string): Uint8Array {
  const comma = dataUrl.indexOf(',');
  const base64 = comma >= 0 ? dataUrl.slice(comma + 1) : dataUrl;
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
