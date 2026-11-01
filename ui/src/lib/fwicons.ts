// Icons of the firmware's own codes (mouse, volume) and Menu, drawn as in the Fn layer diagram of the M10SP
// manual. Each icon is a list of shapes in a 20 x 16 box centred on 0,0, stroked in currentColor; `solid` shapes
// are filled.

export type Shape =
  { d: string; solid?: boolean } | { x: number; y: number; w: number; h: number; rx: number };

const mouse = (x: number): Shape[] => [
  { x: x - 3.5, y: -6, w: 7, h: 12, rx: 3.5 },
  { d: `M${x - 3.5} 0H${x + 3.5}M${x} -6V0` },
];
const wheel: Shape = { d: "M-2.8 -4.8H-1.2V-1.4H-2.8Z", solid: true };
const speaker: Shape = { d: "M-7 -2.2H-4.6L-1 -5.6V5.6L-4.6 2.2H-7Z", solid: true };

export const ICONS: Record<string, Shape[]> = {
  lclick: [...mouse(0), { d: "M0 -6A3.5 3.5 0 0 0 -3.5 -2.5V0H0Z", solid: true }],
  rclick: [...mouse(0), { d: "M0 -6A3.5 3.5 0 0 1 3.5 -2.5V0H0Z", solid: true }],
  mup: [...mouse(-2.5), { d: "M5 3.5V-4M3 -2L5 -4L7 -2" }],
  mdown: [...mouse(-2.5), { d: "M5 -3.5V4M3 2L5 4L7 2" }],
  mleft: [...mouse(3), { d: "M-2.5 0H-9M-7 -2L-9 0L-7 2" }],
  mright: [...mouse(-3), { d: "M2.5 0H9M7 -2L9 0L7 2" }],
  wheelup: [...mouse(-2), wheel, { d: "M4 -1C5.2 -2.4 5.2 -4.6 4 -6M3 -5.5L4 -6.5L5 -5.4" }],
  wheeldown: [...mouse(-2), wheel, { d: "M4 -5C5.2 -3.6 5.2 -1.4 4 0M3 -0.6L4 0.5L5 -0.5" }],
  mute: [speaker, { d: "M2.5 -2.5L7 2M7 -2.5L2.5 2" }],
  voldown: [speaker, { d: "M1.5 -2.5A3.5 3.5 0 0 1 1.5 2.5" }],
  volup: [speaker, { d: "M1.5 -2.5A3.5 3.5 0 0 1 1.5 2.5M3.8 -5A7 7 0 0 1 3.8 5" }],
  menu: [{ x: -5.5, y: -6, w: 11, h: 12, rx: 1 }, { d: "M-3 -2.8H3M-3 0H3M-3 2.8H3" }],
};
