import { type RefObject, useEffect, useRef, useState } from "react";

import { adjacentTab, type MainTab } from "./mainTabs";
import {
  SWIPE_TAB_THRESHOLD_PX,
  resolveSwipeTab,
  swipeOffsetPx,
} from "./swipeTabGesture";

const AXIS_SLOP_PX = 8;
const SETTLE_MS = 220;

export type TabSwipeMotion = {
  offsetPx: number;
  animate: boolean;
};

type Options = {
  containerRef: RefObject<HTMLElement | null>;
  activeTab: MainTab;
  onTabChange: (tab: MainTab) => void;
  enabled: boolean;
};

const IDLE_MOTION: TabSwipeMotion = { offsetPx: 0, animate: false };

function isTextEntryTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    target.closest("input, textarea, select, [contenteditable='true']") !== null
  );
}

export function useMobileTabSwipe({
  containerRef,
  activeTab,
  onTabChange,
  enabled,
}: Options): TabSwipeMotion {
  const [motion, setMotion] = useState<TabSwipeMotion>(IDLE_MOTION);
  const activeTabRef = useRef(activeTab);
  const onTabChangeRef = useRef(onTabChange);

  useEffect(() => {
    activeTabRef.current = activeTab;
  }, [activeTab]);

  useEffect(() => {
    onTabChangeRef.current = onTabChange;
  }, [onTabChange]);

  useEffect(() => {
    if (!enabled) return;
    const el = containerRef.current;
    if (!el) return;

    let startX = 0;
    let startY = 0;
    let tracking = false;
    let axis: "unset" | "h" | "v" = "unset";
    let lastDx = 0;
    let settling = false;
    let settleTimer = 0;

    const trackEl = () => el.querySelector<HTMLElement>(".app-tab-pager-track");

    const commitNeighbor = () => {
      if (!settling) return;
      settling = false;
      window.clearTimeout(settleTimer);
      trackEl()?.removeEventListener("transitionend", onSettleEnd);
      const direction = resolveSwipeTab(lastDx, 0, SWIPE_TAB_THRESHOLD_PX);
      const neighbor =
        direction === null ? null : adjacentTab(activeTabRef.current, direction);
      if (neighbor) onTabChangeRef.current(neighbor);
      setMotion(IDLE_MOTION);
    };

    const onSettleEnd = (event: TransitionEvent) => {
      if (event.propertyName !== "transform") return;
      commitNeighbor();
    };

    const onStart = (event: TouchEvent) => {
      if (event.touches.length !== 1) return;
      if (isTextEntryTarget(event.target)) {
        tracking = false;
        return;
      }
      window.clearTimeout(settleTimer);
      settling = false;
      trackEl()?.removeEventListener("transitionend", onSettleEnd);
      const touch = event.touches[0];
      if (!touch) return;
      startX = touch.clientX;
      startY = touch.clientY;
      tracking = true;
      axis = "unset";
      lastDx = 0;
      setMotion(IDLE_MOTION);
    };

    const onMove = (event: TouchEvent) => {
      if (!tracking || event.touches.length !== 1) return;
      const touch = event.touches[0];
      if (!touch) return;
      const dx = touch.clientX - startX;
      const dy = touch.clientY - startY;
      if (axis === "unset") {
        if (Math.abs(dx) < AXIS_SLOP_PX && Math.abs(dy) < AXIS_SLOP_PX) return;
        axis = Math.abs(dx) > Math.abs(dy) ? "h" : "v";
      }
      if (axis !== "h") return;
      const tab = activeTabRef.current;
      const offset = swipeOffsetPx(
        dx,
        adjacentTab(tab, "prev") !== null,
        adjacentTab(tab, "next") !== null,
      );
      lastDx = dx;
      if (offset !== 0) event.preventDefault();
      setMotion({ offsetPx: offset, animate: false });
    };

    const onEnd = () => {
      if (!tracking) return;
      tracking = false;
      if (axis !== "h") {
        setMotion(IDLE_MOTION);
        return;
      }
      const direction = resolveSwipeTab(lastDx, 0, SWIPE_TAB_THRESHOLD_PX);
      const neighbor =
        direction === null ? null : adjacentTab(activeTabRef.current, direction);
      if (!direction || !neighbor) {
        setMotion({ offsetPx: 0, animate: true });
        return;
      }
      const width = el.clientWidth || 1;
      settling = true;
      setMotion({
        offsetPx: direction === "next" ? -width : width,
        animate: true,
      });
      trackEl()?.addEventListener("transitionend", onSettleEnd);
      settleTimer = window.setTimeout(commitNeighbor, SETTLE_MS + 40);
    };

    el.addEventListener("touchstart", onStart, { passive: true });
    el.addEventListener("touchmove", onMove, { passive: false });
    el.addEventListener("touchend", onEnd);
    el.addEventListener("touchcancel", onEnd);

    return () => {
      window.clearTimeout(settleTimer);
      trackEl()?.removeEventListener("transitionend", onSettleEnd);
      el.removeEventListener("touchstart", onStart);
      el.removeEventListener("touchmove", onMove);
      el.removeEventListener("touchend", onEnd);
      el.removeEventListener("touchcancel", onEnd);
    };
  }, [containerRef, enabled]);

  useEffect(() => {
    setMotion(IDLE_MOTION);
  }, [activeTab]);

  if (!enabled) return IDLE_MOTION;
  return motion;
}
