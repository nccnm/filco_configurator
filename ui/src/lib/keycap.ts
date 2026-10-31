// Keyboard geometry, the same numbers as the "M10SP layer setup" page (gen_layers.py of the analysis repository),
// so both drawings look alike: 64 px per key unit, keys drawn as an outer and an inner rounded rectangle.
import type { KeyView } from "../bindings/ipc";

export const U = 64;
export const H = 64;
export const PAD = 6;

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface KeyGeometry {
  outer: Rect;
  inner: Rect;
  cx: number;
  cy: number;
}

export function geometry(k: KeyView): KeyGeometry {
  const x = PAD + k.x * U;
  const y = PAD + k.y * H;
  const w = k.w * U;
  const h = k.h * H;
  const inner = { x: x + 8, y: y + 6, w: w - 16, h: h - 18 };
  return {
    outer: { x: x + 2, y: y + 2, w: w - 4, h: h - 4 },
    inner,
    cx: inner.x + inner.w / 2,
    cy: inner.y + inner.h / 2,
  };
}

export function viewBox(width: number, height: number): string {
  return `0 0 ${width * U + 2 * PAD} ${height * H + 2 * PAD}`;
}

/** Font size so the longest line fits the keycap (condensed font, about 0.56 em per character). */
export function fit(label: string, width: number, base: number): number {
  const longest = Math.max(1, ...label.split("\n").map((l) => [...l].length));
  return Math.round(Math.min(base, (width - 6) / (longest * 0.56)) * 10) / 10;
}

export const ARROWS = new Set(["←", "↑", "→", "↓"]);

/** y of each line so a block of lines is centred on `cy`. */
export function lineYs(label: string, cy: number, size: number): number[] {
  const lines = label.split("\n");
  const lh = size * 1.12;
  const y0 = cy - (lh * (lines.length - 1)) / 2;
  return lines.map((_, i) => y0 + i * lh);
}

export type Direction = "left" | "right" | "up" | "down";

/** The key an arrow press moves to: nearest key centre in that direction, preferring the same row or column. */
export function neighbour(keys: readonly KeyView[], from: KeyView, dir: Direction): KeyView {
  const c = (k: KeyView) => ({ x: k.x + k.w / 2, y: k.y + k.h / 2 });
  const o = c(from);
  let best: KeyView = from;
  let bestScore = Infinity;
  for (const k of keys) {
    if (k === from) continue;
    const p = c(k);
    const dx = p.x - o.x;
    const dy = p.y - o.y;
    const along = dir === "left" ? -dx : dir === "right" ? dx : dir === "up" ? -dy : dy;
    const across = dir === "left" || dir === "right" ? Math.abs(dy) : Math.abs(dx);
    if (along <= 0.01) continue;
    const score = along + across * 4;
    if (score < bestScore) {
      bestScore = score;
      best = k;
    }
  }
  return best;
}
