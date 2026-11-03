import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

const PREFERENCE_KEY = "volum-completion-notifications";

export function completionNotificationsEnabled() {
  return localStorage.getItem(PREFERENCE_KEY) === "true";
}

export async function setCompletionNotifications(enabled: boolean) {
  if (!enabled) {
    localStorage.setItem(PREFERENCE_KEY, "false");
    return false;
  }
  const granted = await isPermissionGranted() || await requestPermission() === "granted";
  localStorage.setItem(PREFERENCE_KEY, String(granted));
  return granted;
}

export async function notifyCompletion(title: string, body: string) {
  try {
    if (!completionNotificationsEnabled() || !await isPermissionGranted()) return;
    sendNotification({ title, body });
  } catch {
    // Completion alerts are optional and must never make the completed operation look failed.
  }
}
