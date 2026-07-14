import { getCurrentWindow } from "@tauri-apps/api/window";

const isTauri = "__TAURI_INTERNALS__" in window;

async function minimize() {
  if (!isTauri) return;
  await getCurrentWindow().minimize();
}

async function toggleMaximize() {
  if (!isTauri) return;
  await getCurrentWindow().toggleMaximize();
}

async function closeWindow() {
  if (!isTauri) return;
  await getCurrentWindow().close();
}

export function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      className="flex h-8 flex-none items-center justify-between border-b border-black/6 pl-3"
    >
      <div
        data-tauri-drag-region
        className="flex items-center gap-2 text-xs text-muted-foreground"
      >
        <div className="flex size-4 items-center justify-center rounded-[4px] bg-primary text-[10px] font-bold text-primary-foreground">
          D
        </div>
        DownloadHub
      </div>
      <div className="flex h-full">
        <button
          aria-label="Minimize"
          onClick={minimize}
          className="flex h-8 w-[46px] items-center justify-center border-none bg-transparent hover:bg-black/6"
        >
          <div className="h-px w-2.5 bg-foreground" />
        </button>
        <button
          aria-label="Maximize"
          onClick={toggleMaximize}
          className="flex h-8 w-[46px] items-center justify-center border-none bg-transparent hover:bg-black/6"
        >
          <div className="size-2.5 border border-foreground" />
        </button>
        <button
          aria-label="Close"
          onClick={closeWindow}
          className="group flex h-8 w-[46px] items-center justify-center border-none bg-transparent text-foreground hover:bg-[#C42B1C] hover:text-white"
        >
          <div className="relative size-2.5">
            <div className="absolute left-0 top-[4px] h-px w-2.5 rotate-45 bg-current" />
            <div className="absolute left-0 top-[4px] h-px w-2.5 -rotate-45 bg-current" />
          </div>
        </button>
      </div>
    </div>
  );
}
