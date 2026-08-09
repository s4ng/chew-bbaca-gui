// 앱 아이콘 생성기 — 외부 의존성 없이 PNG/ICO 를 직접 작성한다.
//
//   node scripts/generate-icons.mjs
//
// 산출물: src-tauri/icons/{32x32.png,128x128.png,128x128@2x.png,icon.png,icon.ico}
//
// 디자인 툴 산출물로 교체할 예정이라면 이 스크립트를 지워도 무방하다.
// 지금은 "빌드가 아이콘 없이 실패하지 않게" 하는 것이 목적이다.

import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const OUT_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "icons");

// ---------------------------------------------------------------- 그리기

const BG_TOP = [13, 74, 79]; // 짙은 청록
const BG_BOTTOM = [7, 44, 48];
const ACCENT = [77, 208, 195];
const LIGHT = [233, 250, 247];

/** 한 변이 `size` 인 RGBA 픽셀 버퍼를 만든다. */
function render(size) {
  const px = Buffer.alloc(size * size * 4);
  const radius = size * 0.22;

  const put = (x, y, [r, g, b], a = 255) => {
    const i = (y * size + x) * 4;
    if (a === 255) {
      px[i] = r;
      px[i + 1] = g;
      px[i + 2] = b;
      px[i + 3] = 255;
      return;
    }
    // 알파 합성 (기존 픽셀 위에 올린다)
    const sa = a / 255;
    px[i] = Math.round(r * sa + px[i] * (1 - sa));
    px[i + 1] = Math.round(g * sa + px[i + 1] * (1 - sa));
    px[i + 2] = Math.round(b * sa + px[i + 2] * (1 - sa));
    px[i + 3] = Math.max(px[i + 3], Math.round(255 * sa));
  };

  // 라운드 사각형 배경 (세로 그라데이션)
  for (let y = 0; y < size; y++) {
    const t = y / (size - 1);
    const bg = [
      Math.round(BG_TOP[0] + (BG_BOTTOM[0] - BG_TOP[0]) * t),
      Math.round(BG_TOP[1] + (BG_BOTTOM[1] - BG_TOP[1]) * t),
      Math.round(BG_TOP[2] + (BG_BOTTOM[2] - BG_TOP[2]) * t),
    ];
    for (let x = 0; x < size; x++) {
      const a = roundedRectCoverage(x, y, size, radius);
      if (a > 0) put(x, y, bg, Math.round(a * 255));
    }
  }

  // loci 트랙 3줄 — 길이가 다른 막대
  const bars = [
    { y: 0.32, w: 0.56, c: LIGHT },
    { y: 0.5, w: 0.4, c: ACCENT },
    { y: 0.68, w: 0.5, c: LIGHT },
  ];
  const h = Math.max(1, Math.round(size * 0.085));
  const x0 = Math.round(size * 0.22);
  for (const bar of bars) {
    const y0 = Math.round(size * bar.y - h / 2);
    const w = Math.round(size * bar.w);
    for (let y = y0; y < y0 + h; y++) {
      for (let x = x0; x < x0 + w; x++) {
        if (x >= 0 && y >= 0 && x < size && y < size) put(x, y, bar.c);
      }
    }
  }
  return px;
}

/** 라운드 사각형 내부 비율(0~1). 가장자리 4x4 슈퍼샘플링으로 부드럽게 만든다. */
function roundedRectCoverage(x, y, size, radius) {
  const S = 4;
  let hits = 0;
  for (let sy = 0; sy < S; sy++) {
    for (let sx = 0; sx < S; sx++) {
      const px = x + (sx + 0.5) / S;
      const py = y + (sy + 0.5) / S;
      if (insideRounded(px, py, size, radius)) hits++;
    }
  }
  return hits / (S * S);
}

function insideRounded(px, py, size, r) {
  const min = 0;
  const max = size;
  if (px < min || py < min || px > max || py > max) return false;
  const cx = Math.min(Math.max(px, min + r), max - r);
  const cy = Math.min(Math.max(py, min + r), max - r);
  const dx = px - cx;
  const dy = py - cy;
  return dx * dx + dy * dy <= r * r;
}

// ---------------------------------------------------------------- PNG

function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function toPng(size, px) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0; // filter: none
    px.copy(raw, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---------------------------------------------------------------- ICO

/** 32bpp BGRA BMP 엔트리 (bottom-up). Windows 전 버전에서 안전하다. */
function toBmpEntry(size, px) {
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8); // XOR + AND 마스크 높이
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  const xor = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    const src = (size - 1 - y) * size * 4;
    for (let x = 0; x < size; x++) {
      const s = src + x * 4;
      const d = (y * size + x) * 4;
      xor[d] = px[s + 2];
      xor[d + 1] = px[s + 1];
      xor[d + 2] = px[s];
      xor[d + 3] = px[s + 3];
    }
  }
  // AND 마스크: 알파를 쓰므로 전부 0. 행은 4바이트 정렬.
  const andStride = Math.ceil(size / 32) * 4;
  const and = Buffer.alloc(andStride * size);
  return Buffer.concat([header, xor, and]);
}

function toIco(entries) {
  const dir = Buffer.alloc(6 + entries.length * 16);
  dir.writeUInt16LE(0, 0);
  dir.writeUInt16LE(1, 2); // 1 = icon
  dir.writeUInt16LE(entries.length, 4);
  let offset = dir.length;
  const blobs = [];
  entries.forEach(({ size, data }, i) => {
    const p = 6 + i * 16;
    dir[p] = size >= 256 ? 0 : size;
    dir[p + 1] = size >= 256 ? 0 : size;
    dir[p + 2] = 0; // 팔레트 없음
    dir[p + 3] = 0;
    dir.writeUInt16LE(1, p + 4);
    dir.writeUInt16LE(32, p + 6);
    dir.writeUInt32LE(data.length, p + 8);
    dir.writeUInt32LE(offset, p + 12);
    offset += data.length;
    blobs.push(data);
  });
  return Buffer.concat([dir, ...blobs]);
}

// ---------------------------------------------------------------- 실행

mkdirSync(OUT_DIR, { recursive: true });

const cache = new Map();
const pixels = (n) => {
  if (!cache.has(n)) cache.set(n, render(n));
  return cache.get(n);
};

for (const [name, size] of [
  ["32x32.png", 32],
  ["128x128.png", 128],
  ["128x128@2x.png", 256],
  ["icon.png", 512],
]) {
  writeFileSync(join(OUT_DIR, name), toPng(size, pixels(size)));
  console.log(`  ${name}`);
}

const ico = toIco(
  [16, 32, 48, 64, 128, 256].map((size) => ({ size, data: toBmpEntry(size, pixels(size)) })),
);
writeFileSync(join(OUT_DIR, "icon.ico"), ico);
console.log("  icon.ico");
