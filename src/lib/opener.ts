import { openPath } from "@tauri-apps/plugin-opener";

/** Opens a folder in the platform file manager (Finder/Explorer/etc.). */
export const openFolder = (path: string) => openPath(path);
