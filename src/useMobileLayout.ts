import { useEffect, useState } from "react";

/** Narrow viewport or touch-first device — bottom nav + full viewport shell. */
const MOBILE_LAYOUT_QUERY =
  "(max-width: 767px), (pointer: coarse) and (max-width: 1024px)";

function readMobileLayout(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia(MOBILE_LAYOUT_QUERY).matches;
}

export function useMobileLayout(): boolean {
  const [mobile, setMobile] = useState(readMobileLayout);

  useEffect(() => {
    const mq = window.matchMedia(MOBILE_LAYOUT_QUERY);
    const onChange = () => setMobile(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  return mobile;
}
