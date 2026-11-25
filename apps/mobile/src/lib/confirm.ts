// Tauri's dialog plugin replaces window.confirm with an async native dialog,
// so the result has to be awaited before it is tested.
export async function confirmAction(message: string): Promise<boolean> {
  return Boolean(await window.confirm(message));
}
