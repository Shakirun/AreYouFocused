import type { TabStep } from "./mainTabs";

/** Horizontal travel, in px, required before a swipe changes tabs. */
export const SWIPE_TAB_THRESHOLD_PX = 48;

/**
 * Finger delta from touchstart (`clientX` / `clientY`).
 * A leftward swipe (negative dx) advances to the next tab.
 */
export function resolveSwipeTab(
  dx: number,
  dy: number,
  threshold = SWIPE_TAB_THRESHOLD_PX,
): TabStep | null {
  if (Math.abs(dx) <= Math.abs(dy)) return null;
  if (Math.abs(dx) <= threshold) return null;
  return dx < 0 ? "next" : "prev";
}

/** Pixel offset while dragging. Zero when that direction has no neighbor. */
export function swipeOffsetPx(
  dx: number,
  canGoPrev: boolean,
  canGoNext: boolean,
): number {
  if (dx > 0 && !canGoPrev) return 0;
  if (dx < 0 && !canGoNext) return 0;
  return dx;
}
