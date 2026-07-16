import { open } from "@tauri-apps/plugin-dialog";

/** Opens a native folder picker. Resolves to null if the user cancels. */
export async function pickOutputFolder(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}

/** Opens a native file picker (any file). Resolves to null if the user cancels. */
export async function pickFile(): Promise<string | null> {
  const selected = await open({ directory: false, multiple: false });
  return typeof selected === "string" ? selected : null;
}
