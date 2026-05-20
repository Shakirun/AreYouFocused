import { type RefObject, useEffect, useRef } from "react";

const WINDOW_INNER_WIDTH = 460;
const MIN_INNER_HEIGHT = 320;
export const MAX_WINDOW_INNER_HEIGHT = 770;
const HEIGHT_SLOP_PX = 12;

function measureContentHeight(contentEl: HTMLElement | null): number {
  if (contentEl) {
    const rect = contentEl.getBoundingClientRect();
    const top = rect.top + window.scrollY;
    return Math.ceil(top + rect.height);
  }
  return document.documentElement.scrollHeight;
}

export function useFitWindowHeight(
  contentRef: RefObject<HTMLElement | null>,
  resizeDeps: readonly unknown[] = [],
) {
  const lastApplied = useRef(0);
  const debounceTimer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    let cancelled = false;

    async function apply() {
      if (cancelled) return;
      try {
        const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
        const scrollH = measureContentHeight(contentRef.current);
        const screenCap = Math.max(
          MIN_INNER_HEIGHT,
          Math.floor(window.screen.availHeight * 0.94),
        );
        const maxH = Math.min(screenCap, MAX_WINDOW_INNER_HEIGHT);
        const target = Math.min(
          maxH,
          Math.max(MIN_INNER_HEIGHT, scrollH + HEIGHT_SLOP_PX),
        );
        if (Math.abs(target - lastApplied.current) < 4) return;
        lastApplied.current = target;
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(WINDOW_INNER_WIDTH, target));
      } catch {
        /* Not inside Tauri webview */
      }
    }

    function schedule() {
      clearTimeout(debounceTimer.current);
      debounceTimer.current = setTimeout(() => void apply(), 48);
    }

    const observed = contentRef.current ?? document.documentElement;
    const ro = new ResizeObserver(schedule);
    ro.observe(observed);

    const onToggle = (e: Event) => {
      if (e.target instanceof HTMLDetailsElement) schedule();
    };
    document.addEventListener("toggle", onToggle, true);

    void document.fonts?.ready?.then(() => schedule());

    requestAnimationFrame(() => {
      requestAnimationFrame(() => void apply());
    });

    schedule();

    return () => {
      cancelled = true;
      clearTimeout(debounceTimer.current);
      ro.disconnect();
      document.removeEventListener("toggle", onToggle, true);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- contentRef is stable; resizeDeps drive re-measure
  }, [contentRef, ...resizeDeps]);
}
