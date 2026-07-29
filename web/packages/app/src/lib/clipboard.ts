import { flash } from "./flash"

export function writeClipboard(text: string): Promise<void> {
  // `navigator.clipboard` exists only in a secure context, so a page served over plain http from a
  // LAN address has none; rejecting keeps that on the promise path instead of throwing out of the
  // key handler.
  const clipboard: Clipboard | undefined = navigator.clipboard
  return clipboard === undefined ? Promise.reject(new Error("clipboard unavailable")) : clipboard.writeText(text)
}

export function copyTaskId(id: string): void {
  void writeClipboard(id).then(
    () => flash.show(`Copied ${id}`, "ok"),
    () => flash.show("Copy failed", "error"),
  )
}
