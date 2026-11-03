import { focusManager, onlineManager, type QueryClient } from "@tanstack/react-query";

export function installQueryLifecycle(queryClient: QueryClient) {
  const updateOnline = () => onlineManager.setOnline(navigator.onLine);
  const updateFocus = () => {
    const focused = document.visibilityState === "visible";
    focusManager.setFocused(focused);
    if (focused && navigator.onLine) void queryClient.invalidateQueries({ stale: true });
  };

  window.addEventListener("online", updateOnline);
  window.addEventListener("offline", updateOnline);
  window.addEventListener("pageshow", updateFocus);
  window.addEventListener("pagehide", updateFocus);
  document.addEventListener("visibilitychange", updateFocus);
  updateOnline();
  updateFocus();

  return () => {
    window.removeEventListener("online", updateOnline);
    window.removeEventListener("offline", updateOnline);
    window.removeEventListener("pageshow", updateFocus);
    window.removeEventListener("pagehide", updateFocus);
    document.removeEventListener("visibilitychange", updateFocus);
  };
}
