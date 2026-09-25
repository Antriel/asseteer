/**
 * One labelled grid PNG per run, stitched from that run's `shot()` calls.
 *
 * Reading six screenshots one at a time is expensive, and — worse — it puts the two
 * frames of a before/after in different messages, where comparing them is guesswork.
 * The sheet is the thing to read *first*: it says what happened across the run and
 * which cell is worth a full-resolution look. The individual PNGs stay on disk for
 * exactly that look, so this trades detail for context on purpose.
 *
 * Cells are downscaled, so judge layout, state and gross colour here — not a hairline
 * border or 11px label. Open the individual PNG for those.
 *
 * Ported verbatim from scry-app's harness. Captions are SVG text composited last, which renders through
 * libvips' text stack on Windows as well as CI.
 */

import path from "node:path";
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import sharp from "sharp";
import { ROOT, SHOT_DIR } from "./paths.mjs";

/** Where a spec run accumulates its shots. Worker processes and the run's teardown are
 *  different processes, so the list has to survive on disk rather than in a module. */
const SHOT_LOG = path.join(SHOT_DIR, "shots.ndjson");

const SHEET_WIDTH = 1600;
const PAD = 14;
const GAP = 12;
const HEADER_H = 30;
const CAPTION_H = 34;

const BG = "#0d1117";
const CELL_BG = "#161b22";
const BORDER = "#30363d";
const TEXT = "#e6edf3";
const DIM = "#8b98a5";

const escapeXml = (s) =>
  String(s).replace(/[<>&'"]/g, (c) => `&${{ "<": "lt", ">": "gt", "&": "amp", "'": "apos", '"': "quot" }[c]};`);

/** Clip a caption to what fits a cell, so a long name cannot run into its neighbour. */
const clip = (s, width, fontSize) => {
  const max = Math.max(6, Math.floor(width / (fontSize * 0.55)));
  return s.length <= max ? s : `${s.slice(0, max - 1)}…`;
};

/**
 * Columns that keep a before/after pair side by side, and stay readable past that.
 *
 * Chosen to leave no orphan cell in the common counts (2, 3, 4, 6, 9): a half-empty
 * last row costs sheet height, and height is what the reader pays for.
 */
function columnsFor(n) {
  if (n <= 3) return n;
  if (n <= 4) return 2;
  if (n <= 9) return 3;
  return 4;
}

/** Append one shot to the run log. Called by whichever consumer took the shot. */
export function recordShot(entry) {
  mkdirSync(SHOT_DIR, { recursive: true });
  appendFileSync(SHOT_LOG, `${JSON.stringify(entry)}\n`);
}

/** Drop the previous run's log — called once when a run starts, not per worker. */
export function resetShotLog() {
  rmSync(SHOT_LOG, { force: true });
}

/** The current run's shots, in the order they were taken. */
function readShotLog() {
  if (!existsSync(SHOT_LOG)) return [];
  return readFileSync(SHOT_LOG, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

/**
 * Stitch `entries` into one labelled grid PNG.
 *
 * @param {{ file: string, label?: string, sub?: string }[]} entries
 * @param {object} [opts]
 * @param {string} [opts.out]    Where to write. Defaults to `tests/harness/out/contact.png`.
 * @param {string} [opts.title]  Drawn in the header strip.
 * @returns {Promise<string|null>} The sheet's path, or null if there was nothing to stitch.
 */
export async function buildContactSheet(entries, { out = path.join(SHOT_DIR, "contact.png"), title = "" } = {}) {
  // The same name shot twice overwrites its PNG, so the earlier entry now points at the
  // later capture. Keep the last position for each file — the sheet then shows what is
  // actually on disk, once, in the place it was last taken.
  const seen = new Map();
  for (const entry of entries) {
    if (existsSync(entry.file)) seen.set(entry.file, entry);
  }
  const shots = [...seen.values()];
  if (!shots.length) return null;

  const cols = columnsFor(shots.length);
  const rows = Math.ceil(shots.length / cols);
  const cellW = Math.floor((SHEET_WIDTH - PAD * 2 - GAP * (cols - 1)) / cols);

  const sized = await Promise.all(
    shots.map(async (entry) => {
      const { width = 1, height = 1 } = await sharp(entry.file).metadata();
      return { ...entry, width, height };
    })
  );

  // One image height for every cell, from the tallest aspect present: a pop-out window
  // and a main window in the same run then line up rather than staggering the grid.
  const maxRatio = Math.max(...sized.map((s) => s.height / s.width));
  const imgH = Math.min(Math.round(cellW * maxRatio), 900);
  const cellH = imgH + CAPTION_H;
  const sheetH = PAD * 2 + HEADER_H + rows * cellH + (rows - 1) * GAP;

  const composites = [];
  const chrome = [];

  if (title) {
    chrome.push(
      `<text x="${PAD}" y="${PAD + 19}" font-family="Segoe UI, DejaVu Sans, sans-serif" font-size="16" fill="${DIM}">${escapeXml(title)}</text>`
    );
  }

  for (const [i, entry] of sized.entries()) {
    const col = i % cols;
    const row = Math.floor(i / cols);
    const x = PAD + col * (cellW + GAP);
    const y = PAD + HEADER_H + row * (cellH + GAP);

    const resized = await sharp(entry.file)
      .resize({ width: cellW, height: imgH, fit: "inside", withoutEnlargement: true })
      .png()
      .toBuffer({ resolveWithObject: true });

    composites.push({
      input: resized.data,
      left: x + Math.floor((cellW - resized.info.width) / 2),
      top: y + Math.floor((imgH - resized.info.height) / 2),
    });

    const label = clip(`${i + 1} · ${entry.label ?? path.basename(entry.file, ".png")}`, cellW - 16, 17);
    chrome.push(
      `<rect x="${x}" y="${y}" width="${cellW}" height="${cellH}" fill="none" stroke="${BORDER}" stroke-width="1"/>`,
      `<text x="${x + 8}" y="${y + imgH + 20}" font-family="Segoe UI, DejaVu Sans, sans-serif" font-size="17" fill="${TEXT}">${escapeXml(label)}</text>`
    );
    if (entry.sub) {
      chrome.push(
        `<text x="${x + 8}" y="${y + imgH + 31}" font-family="Segoe UI, DejaVu Sans, sans-serif" font-size="11" fill="${DIM}">${escapeXml(clip(entry.sub, cellW - 16, 11))}</text>`
      );
    }
  }

  const cells = sized.map((_, i) => {
    const x = PAD + (i % cols) * (cellW + GAP);
    const y = PAD + HEADER_H + Math.floor(i / cols) * (cellH + GAP);
    return `<rect x="${x}" y="${y}" width="${cellW}" height="${cellH}" fill="${CELL_BG}"/>`;
  });

  mkdirSync(path.dirname(out), { recursive: true });
  await sharp({ create: { width: SHEET_WIDTH, height: sheetH, channels: 4, background: BG } })
    .composite([
      { input: Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${SHEET_WIDTH}" height="${sheetH}">${cells.join("")}</svg>`) },
      ...composites,
      { input: Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${SHEET_WIDTH}" height="${sheetH}">${chrome.join("")}</svg>`) },
    ])
    .png()
    .toFile(out);

  return out;
}

/** Build the sheet for the shots in the run log. Used by the spec run's teardown. */
export async function buildContactSheetFromLog(opts = {}) {
  const entries = readShotLog();
  if (entries.length < 2) return null;
  return buildContactSheet(entries, { title: `${entries.length} shots · ${path.relative(ROOT, SHOT_DIR)}`, ...opts });
}
