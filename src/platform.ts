/**
 * Coarse platform detection for UI decisions that differ between the desktop
 * Tauri window and the full-screen mobile webview (Android / iOS).
 *
 * The Android system WebView and iOS WKWebView both expose their OS in the
 * user agent, which is enough here — no plugin round-trip needed.
 */
export const IS_MOBILE: boolean =
  typeof navigator !== "undefined" &&
  /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
