/**
 * Dev preview: renders the Coral overlay to a PNG outside the app, so a change to
 * the path engine can be eyeballed without launching the UI.
 *
 * It is faithful on both sides of the boundary:
 *  - geometry comes from the app's real `computeCoralPath` (compiled from
 *    src/visualizations/coral/coralPath.ts), and
 *  - parity sequences come from the real engine, fetched from a running
 *    StateLabServer — nothing is recomputed here.
 *
 * Usage:
 *   npx esbuild src/visualizations/coral/coralPath.ts --format=esm --outfile=<tmp>/coralPath.mjs
 *   node scripts/preview-coral.mjs <serverUrl> <from> <to> <rule> <out.png>
 *
 * Example:
 *   node scripts/preview-coral.mjs http://127.0.0.1:51287 1 400 aesthetic preview.png
 */

import { deflateSync } from 'node:zlib';
import { writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const [, , SERVER, FROM, TO, RULE, OUT, MODULE] = process.argv;
const from = Number(FROM ?? 1);
const to = Number(TO ?? 200);
const rule = RULE ?? 'aesthetic';
const out = OUT ?? 'coral-preview.png';
const modulePath = MODULE ?? '/tmp/coralPath.mjs';

const { computeCoralPath, pathBounds } = await import(pathToFileURL(modulePath).href);

const params = { oddAngle: 17, evenAngle: -10, lineLength: 6, rotation: -90 };

// --- Parity sequences straight from the engine ---
const parities = [];
for (let n = from; n <= to; n++) {
  const res = await fetch(`${SERVER}/api/run?initialState=${n}`);
  const t = await res.json();
  const p = t.system_specific_metrics?.parity_sequence;
  if (Array.isArray(p) && p.length > 0) parities.push(p);
}
console.log(`fetched ${parities.length} parity sequences from the engine`);

// --- Real path geometry ---
const paths = parities.map((p) => computeCoralPath(p, params, rule));

// --- Render ---
const W = 900;
const H = 620;
const px = new Uint8Array(W * H * 4);
for (let i = 0; i < W * H; i++) px[i * 4 + 3] = 255; // opaque black

const b = pathBounds(paths.flat());
const pad = 24;
const s = Math.min((W - 2 * pad) / (b.maxX - b.minX || 1), (H - 2 * pad) / (b.maxY - b.minY || 1));
const cx = (b.minX + b.maxX) / 2;
const cy = (b.minY + b.maxY) / 2;
const PX = (x) => W / 2 + (x - cx) * s;
const PY = (y) => H / 2 - (y - cy) * s;

const alpha = Math.max(0.05, Math.min(1, 0.85 / Math.sqrt(paths.length || 1)));
function plot(x, y, a) {
  const xi = Math.round(x);
  const yi = Math.round(y);
  if (xi < 0 || yi < 0 || xi >= W || yi >= H) return;
  const i = (yi * W + xi) * 4;
  const v = Math.min(255, px[i] + Math.round(255 * a));
  px[i] = v;
  px[i + 1] = v;
  px[i + 2] = v;
}
function line(x0, y0, x1, y1) {
  const steps = Math.ceil(Math.hypot(x1 - x0, y1 - y0)) + 1;
  for (let k = 0; k <= steps; k++) {
    const t = k / steps;
    plot(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, alpha);
  }
}
for (const path of paths) {
  for (let i = 0; i < path.length - 1; i++) {
    line(PX(path[i].x), PY(path[i].y), PX(path[i + 1].x), PY(path[i + 1].y));
  }
}

// --- PNG ---
const T = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const byte of buf) c = T[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
};
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr[8] = 8;
ihdr[9] = 6;
const raw = Buffer.alloc((W * 4 + 1) * H);
for (let y = 0; y < H; y++) {
  const off = y * (W * 4 + 1);
  raw[off] = 0;
  Buffer.from(px.buffer, y * W * 4, W * 4).copy(raw, off + 1);
}
writeFileSync(
  out,
  Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]),
);
console.log(`wrote ${out} (${W}x${H}, rule=${rule}, ${paths.length} trajectories)`);
