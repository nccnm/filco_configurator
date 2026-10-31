import { describe, expect, it } from "vitest";
import type { KeyView } from "../bindings/ipc";
import { fit, geometry, lineYs, neighbour, viewBox } from "./keycap";

const key = (id: string, x: number, y: number, w = 1): KeyView => ({
  id,
  index: 0,
  x,
  y,
  w,
  h: 1,
  legend: id,
  m_key: false,
  unverified: false,
  note: null,
});

describe("geometry", () => {
  it("matches the layer page drawing (outer inset 2, inner inset 8/6)", () => {
    const g = geometry(key("Tab", 0, 1, 1.5));
    expect(g.outer).toEqual({ x: 8, y: 72, w: 92, h: 60 });
    expect(g.inner).toEqual({ x: 14, y: 76, w: 80, h: 46 });
    expect(g.cx).toBe(54);
    expect(g.cy).toBe(99);
  });

  it("view box covers the layout plus padding", () => {
    expect(viewBox(17.9, 5)).toBe(`0 0 ${17.9 * 64 + 12} ${5 * 64 + 12}`);
  });
});

describe("fit", () => {
  it("keeps the base size for short labels and shrinks long ones", () => {
    expect(fit("F5", 48, 14.5)).toBe(14.5);
    expect(fit("Backspace", 48, 14.5)).toBeLessThan(14.5);
    expect(fit("click\ntrái", 48, 14.5)).toBe(fit("click", 48, 14.5));
  });
});

describe("lineYs", () => {
  it("centres multi-line labels", () => {
    expect(lineYs("a", 100, 10)).toEqual([100]);
    const [a = 0, b = 0] = lineYs("a\nb", 100, 10);
    expect(a + b).toBeCloseTo(200);
  });
});

describe("neighbour", () => {
  const esc = key("Esc", 0, 0);
  const one = key("1", 1, 0);
  const two = key("2", 2, 0);
  const tab = key("Tab", 0, 1, 1.5);
  const q = key("Q", 1.5, 1);
  const w = key("W", 2.5, 1);
  const keys = [esc, one, two, tab, q, w];

  it("moves along a row and stops at the edge", () => {
    expect(neighbour(keys, one, "right").id).toBe("2");
    expect(neighbour(keys, one, "left").id).toBe("Esc");
    expect(neighbour(keys, esc, "left").id).toBe("Esc");
  });

  it("moves between rows to the nearest key", () => {
    expect(neighbour(keys, one, "down").id).toBe("Q");
    expect(neighbour(keys, w, "up").id).toBe("2");
  });
});
