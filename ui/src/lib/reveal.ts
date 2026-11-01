/**
 * Scrolls `panel` so its element marked `.current` (the value the selected key holds) sits in the middle of the
 * visible height, or to the top when nothing is marked. Only `panel` scrolls: `scrollIntoView` would also move
 * the page and the other scroll areas.
 */
export function centerCurrent(panel: HTMLElement | undefined | null): void {
  if (!panel) return;
  const el = panel.querySelector<HTMLElement>(".current");
  if (!el) {
    panel.scrollTop = 0;
    return;
  }
  const p = panel.getBoundingClientRect();
  const r = el.getBoundingClientRect();
  panel.scrollTop += r.top - p.top - (panel.clientHeight - r.height) / 2;
}
