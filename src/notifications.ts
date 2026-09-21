import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";

/**
 * Android 13+ requires a runtime opt-in before the app may post notifications,
 * and pings are delivered as notifications on mobile. Returns whether pings
 * can currently reach the user.
 */
export async function ensureNotificationPermission(): Promise<boolean> {
  try {
    if (await isPermissionGranted()) return true;
    return (await requestPermission()) === "granted";
  } catch {
    // Outside the Tauri webview (plain `npm run dev`) there is no bridge.
    return true;
  }
}
