/**
 * Generate the fixture library the harness scans: `tests/fixtures/library/`.
 *
 * Generated rather than committed — binaries in git for a test fixture are noise, and
 * this is deterministic (seeded noise, fixed geometry), so every run sees the same
 * bytes. Regenerated when `VERSION` changes; bump it whenever the contents change.
 *
 * What it covers, on purpose:
 * - audio of different lengths (duration filter, list rows) with search-relevant names:
 *   multi-word, hyphenated (`sci-fi` is an FTS syntax trap), shared prefixes
 * - images of different sizes/aspects (grid, thumbnails)
 * - a zip, and a zip nested inside it — the paths that differ most in the backend
 *
 * Standalone: `node tests/fixtures/makeLibrary.mjs [--force]`. The harness calls
 * `ensureLibrary()` itself.
 */

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';
import sharp from 'sharp';

const VERSION = '1';

export const LIBRARY = path.join(path.dirname(fileURLToPath(import.meta.url)), 'library');
const STAMP = path.join(LIBRARY, '.fixture-version');

// ── audio ────────────────────────────────────────────────────────────────────────

const RATE = 22050;

/** Mulberry32 — seeded, so "noise" is the same bytes every run. */
function rng(seed) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * A mono 16-bit WAV. `kind` picks a timbre so the files sound (and look, in a
 * waveform) different: a decaying noise burst, a pitch sweep, or a steady tone.
 */
function wav({ seconds, kind, freq = 440, seed = 1 }) {
  const n = Math.round(seconds * RATE);
  const data = Buffer.alloc(n * 2);
  const rand = rng(seed);
  let phase = 0;
  for (let i = 0; i < n; i++) {
    const t = i / n;
    let s;
    if (kind === 'burst') {
      s = (rand() * 2 - 1) * Math.exp(-t * 8);
    } else if (kind === 'sweep') {
      phase += (2 * Math.PI * (freq + freq * 3 * t)) / RATE;
      s = Math.sin(phase) * Math.min(1, (1 - t) * 4);
    } else {
      phase += (2 * Math.PI * freq) / RATE;
      s = Math.sin(phase) * 0.6 + (rand() * 2 - 1) * 0.05;
    }
    data.writeInt16LE(Math.round(Math.max(-1, Math.min(1, s)) * 0.8 * 32767), i * 2);
  }

  const header = Buffer.alloc(44);
  header.write('RIFF', 0);
  header.writeUInt32LE(36 + data.length, 4);
  header.write('WAVE', 8);
  header.write('fmt ', 12);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(1, 22); // mono
  header.writeUInt32LE(RATE, 24);
  header.writeUInt32LE(RATE * 2, 28);
  header.writeUInt16LE(2, 32);
  header.writeUInt16LE(16, 34);
  header.write('data', 36);
  header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]);
}

// ── images ───────────────────────────────────────────────────────────────────────

function png({ w, h, bg, shape = '' }) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">
    <rect width="100%" height="100%" fill="${bg}"/>${shape}</svg>`;
  return sharp(Buffer.from(svg)).png().toBuffer();
}

const circle = (w, h, fill) =>
  `<circle cx="${w / 2}" cy="${h / 2}" r="${Math.min(w, h) / 3}" fill="${fill}"/>`;
const stripes = (w, h, fill) =>
  Array.from({ length: 4 }, (_, i) => `<rect x="0" y="${(i * h) / 4}" width="${w}" height="${h / 8}" fill="${fill}"/>`).join('');

// ── zip ──────────────────────────────────────────────────────────────────────────

/** Minimal deflate zip writer: enough for the backend's zip crate, no dependency. */
function zip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const { name, data } of entries) {
    const nameBuf = Buffer.from(name, 'utf8');
    const deflated = zlib.deflateRawSync(data);
    const crc = zlib.crc32(data);

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4); // version needed
    local.writeUInt16LE(0x0800, 6); // UTF-8 names
    local.writeUInt16LE(8, 8); // deflate
    local.writeUInt32LE(0, 10); // time/date: 1980-00-00, deterministic
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(deflated.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    local.writeUInt16LE(0, 28);
    locals.push(local, nameBuf, deflated);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(8, 10);
    central.writeUInt32LE(0, 12);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(deflated.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, nameBuf);

    offset += local.length + nameBuf.length + deflated.length;
  }

  const centralBuf = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralBuf.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, centralBuf, end]);
}

// ── the library ──────────────────────────────────────────────────────────────────

async function contents() {
  return {
    'Weapons/gun_shot_01.wav': wav({ seconds: 0.6, kind: 'burst', seed: 1 }),
    'Weapons/gun_shot_02.wav': wav({ seconds: 0.8, kind: 'burst', seed: 2 }),
    'Weapons/reload_pistol.wav': wav({ seconds: 1.2, kind: 'sweep', freq: 300, seed: 3 }),
    'Explosions/explosion_big.wav': wav({ seconds: 3.0, kind: 'burst', seed: 4 }),
    'Explosions/explosion_small.wav': wav({ seconds: 1.0, kind: 'burst', seed: 5 }),
    'Sci-Fi/laser_blast.wav': wav({ seconds: 0.5, kind: 'sweep', freq: 800, seed: 6 }),
    'Sci-Fi/sci-fi_door_open.wav': wav({ seconds: 1.5, kind: 'sweep', freq: 120, seed: 7 }),
    'Ambience/forest_birds_loop.wav': wav({ seconds: 6.0, kind: 'tone', freq: 1800, seed: 8 }),
    'Footsteps/footstep_grass_01.wav': wav({ seconds: 0.3, kind: 'burst', seed: 9 }),
    'Footsteps/footstep_gravel_01.wav': wav({ seconds: 0.35, kind: 'burst', seed: 10 }),
    'UI/button_red.png': await png({ w: 256, h: 96, bg: '#c0392b', shape: circle(256, 96, '#ffffff55') }),
    'UI/button_green.png': await png({ w: 256, h: 96, bg: '#27ae60', shape: circle(256, 96, '#ffffff55') }),
    'UI/icon_heart.png': await png({ w: 64, h: 64, bg: '#1d1d1d', shape: circle(64, 64, '#e74c3c') }),
    'Tiles/grass_tile.png': await png({ w: 128, h: 128, bg: '#4caf50', shape: stripes(128, 128, '#388e3c') }),
    'Tiles/stone_tile.png': await png({ w: 128, h: 128, bg: '#9e9e9e', shape: stripes(128, 128, '#757575') }),
    'Tiles/water_wide.png': await png({ w: 512, h: 128, bg: '#2196f3', shape: stripes(512, 128, '#1976d2') }),
    'Packs/Retro Pack.zip': zip([
      { name: 'Sounds/retro_coin.wav', data: wav({ seconds: 0.4, kind: 'sweep', freq: 900, seed: 11 }) },
      { name: 'Sounds/retro_jump.wav', data: wav({ seconds: 0.5, kind: 'sweep', freq: 400, seed: 12 }) },
      { name: 'Sprites/coin.png', data: await png({ w: 32, h: 32, bg: '#00000000', shape: circle(32, 32, '#f1c40f') }) },
      {
        name: 'Extras/bonus.zip',
        data: zip([
          { name: 'retro_powerup.wav', data: wav({ seconds: 0.9, kind: 'sweep', freq: 600, seed: 13 }) },
          { name: 'bonus_star.png', data: await png({ w: 48, h: 48, bg: '#000000', shape: circle(48, 48, '#ffeb3b') }) },
        ]),
      },
    ]),
  };
}

/** Make sure the library exists at the current VERSION. Returns its absolute path. */
export async function ensureLibrary({ force = false } = {}) {
  if (!force && existsSync(STAMP) && readFileSync(STAMP, 'utf8') === VERSION) return LIBRARY;

  rmSync(LIBRARY, { recursive: true, force: true });
  for (const [rel, data] of Object.entries(await contents())) {
    const file = path.join(LIBRARY, rel);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, data);
  }
  writeFileSync(STAMP, VERSION);
  return LIBRARY;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(await ensureLibrary({ force: process.argv.includes('--force') }));
}
