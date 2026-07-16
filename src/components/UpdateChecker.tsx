import { useState } from "react";
import { DownloadIcon, RefreshCwIcon } from "lucide-react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";

type Phase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up-to-date" }
  | { kind: "available"; update: Update }
  | { kind: "installing"; pct: number | null }
  | { kind: "error"; message: string };

export function UpdateChecker() {
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });

  async function checkForUpdate() {
    setPhase({ kind: "checking" });
    setOpen(true);
    try {
      const update = await check();
      setPhase(update ? { kind: "available", update } : { kind: "up-to-date" });
    } catch (err) {
      setPhase({ kind: "error", message: String(err) });
    }
  }

  async function installUpdate(update: Update) {
    setPhase({ kind: "installing", pct: null });
    try {
      let downloaded = 0;
      let total = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setPhase({
            kind: "installing",
            pct: total > 0 ? Math.round((downloaded / total) * 100) : null,
          });
        }
      });
      // Installer has run; restart into the new version.
      await relaunch();
    } catch (err) {
      setPhase({ kind: "error", message: String(err) });
    }
  }

  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        className="gap-2"
        onClick={checkForUpdate}
        disabled={phase.kind === "checking" || phase.kind === "installing"}
      >
        <RefreshCwIcon className="size-4" />
        Check for updates
      </Button>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Software update</DialogTitle>
            <DialogDescription>
              {phase.kind === "checking" && "Checking for updates…"}
              {phase.kind === "up-to-date" && "You're on the latest version."}
              {phase.kind === "available" &&
                `Version ${phase.update.version} is available.`}
              {phase.kind === "installing" &&
                (phase.pct === null
                  ? "Downloading update…"
                  : `Downloading update… ${phase.pct}%`)}
              {phase.kind === "error" && "Update check failed."}
            </DialogDescription>
          </DialogHeader>

          {phase.kind === "available" && phase.update.body && (
            <p className="max-h-40 overflow-y-auto whitespace-pre-wrap text-sm text-muted-foreground">
              {phase.update.body}
            </p>
          )}

          {phase.kind === "error" && (
            <p className="text-sm text-destructive">{phase.message}</p>
          )}

          {phase.kind === "available" && (
            <div className="flex justify-end">
              <Button
                className="gap-2"
                onClick={() => installUpdate(phase.update)}
              >
                <DownloadIcon className="size-4" />
                Download & install
              </Button>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}
