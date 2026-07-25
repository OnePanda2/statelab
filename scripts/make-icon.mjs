/**
 * Generates the StateLab source icon: a 1024x1024 PNG showing the real Collatz
 * trajectory for n = 27 (111 iterations, peak 9232) on a log scale.
 *
 * Dependency-free — writes the PNG by hand using Node's built-in zlib, so the
 * repo needs no image tooling. Feed the result to `npx tauri icon` to produce the
 * full platform icon set.
 *
 * Usage:  node scripts/make-icon.mjs [out.png]
 */

import { deflateSync } from 'node:zlib';
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

const SIZE = 1024;
const OUT = process.argv[2] ?? 'src-tauri/icons/source.png';

// ---- The actual trajectory (same rule as the engine: odd -> 3n+1, even -> n/2) ----
function collatz(n) {
  const seq = [n];
  let v = BigInt(n);
  while (v !== 1n) {
    v = v % 2n === 1n ? 3n * v + 1n : v / 2n;
    seq.push(Number(v));
  }
  return seq;
}

const seq = collatz(27);
const ys = seq.map((v) => Math.log10(v + 1));
const maxY = Math.max(...ys);

// ---- Canvas ----
const px = new Uint8Array(SIZE * SIZE * 4);

function set(x, y, r, g, b, a = 255) {
  if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) return;
  const i = (y * SIZE + x) * 4;
  // Simple source-over blend against what's already there.
  const inv = 1 - a / 255;
  px[i] = Math.round(r * (a / 255) + px[i] * inv);
  px[i + 1] = Math.round(g * (a / 255) + px[i + 1] * inv);
  px[i + 2] = Math.round(b * (a / 255) + px[i + 2] * inv);
  px[i + 3] = 255;
}

// Background: rounded-square dark panel with a subtle vertical gradient.
const RADIUS = 180;
for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const dx = Math.max(RADIUS - x, x - (SIZE - 1 - RADIUS), 0);
    const dy = Math.max(RADIUS - y, y - (SIZE - 1 - RADIUS), 0);
    if (dx * dx + dy * dy > RADIUS * RADIUS) continue; // outside the rounded corner
    const t = y / SIZE;
    set(x, y, Math.round(14 + 8 * t), Math.round(17 + 10 * t), Math.round(22 + 14 * t));
  }
}

// Trajectory polyline, thick, with a soft glow.
const pad = 190;
const plotW = SIZE - 2 * pad;
const plotH = SIZE - 2 * pad;
const xAt = (i) => pad + (i / (ys.length - 1)) * plotW;
const yAt = (v) => SIZE - pad - (v / maxY) * plotH;

function line(x0, y0, x1, y1, width, r, g, b, a) {
  const steps = Math.ceil(Math.hypot(x1 - x0, y1 - y0) * 2) + 1;
  for (let s = 0; s <= steps; s++) {
    const t = s / steps;
    const cx = x0 + (x1 - x0) * t;
    const cy = y0 + (y1 - y0) * t;
    const rad = width / 2;
    for (let oy = -Math.ceil(rad); oy <= Math.ceil(rad); oy++) {
      for (let ox = -Math.ceil(rad); ox <= Math.ceil(rad); ox++) {
        const d = Math.hypot(ox, oy);
        if (d > rad) continue;
        const edge = Math.min(1, (rad - d) / 1.5); // cheap antialiasing
        set(Math.round(cx + ox), Math.round(cy + oy), r, g, b, a * edge);
      }
    }
  }
}

// Glow pass, then the crisp line on top. A single colour keeps the silhouette
// legible when the icon is downscaled to 32x32 — per-segment parity colouring
// turned to visual noise at small sizes.
for (let i = 0; i < ys.length - 1; i++) {
  line(xAt(i), yAt(ys[i]), xAt(i + 1), yAt(ys[i + 1]), 40, 88, 166, 255, 22);
}
for (let i = 0; i < ys.length - 1; i++) {
  // Warm the line slightly toward the peak so the climb reads as a gradient.
  const t = ys[i] / maxY;
  const r = Math.round(88 + 152 * t * t);
  const g = Math.round(166 - 30 * t * t);
  const b = Math.round(255 - 193 * t * t);
  line(xAt(i), yAt(ys[i]), xAt(i + 1), yAt(ys[i + 1]), 16, r, g, b, 255);
}

// Baseline rule.
line(pad, SIZE - pad, SIZE - pad, SIZE - pad, 5, 139, 148, 158, 90);

// ---- PNG encoding ----
const CRC_TABLE = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const byte of buf) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // colour type: RGBA
// 10..12 = compression/filter/interlace = 0

// Raw scanlines, each prefixed with filter byte 0.
const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE);
for (let y = 0; y < SIZE; y++) {
  const off = y * (SIZE * 4 + 1);
  raw[off] = 0;
  Buffer.from(px.buffer, y * SIZE * 4, SIZE * 4).copy(raw, off + 1);
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, png);
console.log(`wrote ${OUT} (${SIZE}x${SIZE}, ${(png.length / 1024).toFixed(0)} KB)`);
console.log(`trajectory: n=27, ${seq.length - 1} iterations, peak ${Math.max(...seq)}`);
