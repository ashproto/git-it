import { reportError } from "./errors.svelte";

/**
 * Copy `text` to the clipboard. False means it wasn't copied and that was reported under
 * `title`, so the caller must not report again. WKWebView allows the write only during the
 * user's gesture, so call this from the click before any await; `writeText` runs before
 * this function's first await for the same reason.
 */
export async function copyText(text: string, title: string): Promise<boolean> {
  if (!navigator.clipboard) {
    reportError(title, "Clipboard is unavailable.");
    return false;
  }
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (e) {
    reportError(title, e);
    return false;
  }
}
