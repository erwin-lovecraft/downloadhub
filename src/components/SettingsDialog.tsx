import { useEffect, useState } from "react";
import { useSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { pickOutputFolder } from "@/lib/dialog";
import type { FormatPreference } from "@/lib/playlist";

export function SettingsDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { settings, save } = useSettings();
  const [outputPath, setOutputPath] = useState("");
  const [quality, setQuality] = useState<FormatPreference>("best_progressive");

  useEffect(() => {
    if (settings.data) {
      setOutputPath(settings.data.default_output_path ?? "");
      setQuality(settings.data.default_quality);
    }
  }, [settings.data]);

  function handleOpenChange(next: boolean) {
    if (!next) save.reset();
    onOpenChange(next);
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="flex flex-col gap-4 sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Defaults pre-filled when adding videos or a playlist to the queue.
          </DialogDescription>
        </DialogHeader>

        {settings.isLoading && (
          <p className="text-sm text-muted-foreground">Loading settings...</p>
        )}
        {settings.error && (
          <p className="text-sm text-destructive">
            {settings.error instanceof Error ? settings.error.message : String(settings.error)}
          </p>
        )}

        {settings.data && (
          <>
            <div className="flex flex-col gap-1.5">
              <label className="text-sm font-medium">Default output folder</label>
              <div className="flex gap-2">
                <Input
                  value={outputPath}
                  onChange={(e) => setOutputPath(e.target.value)}
                  placeholder="e.g. C:\Downloads"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="shrink-0"
                  onClick={async () => {
                    const folder = await pickOutputFolder();
                    if (folder) setOutputPath(folder);
                  }}
                >
                  Browse...
                </Button>
              </div>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-sm font-medium">Default quality</label>
              <div className="flex gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant={quality === "best_progressive" ? "default" : "outline"}
                  onClick={() => setQuality("best_progressive")}
                >
                  Best quality (video + audio)
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant={quality === "best_audio_only" ? "default" : "outline"}
                  onClick={() => setQuality("best_audio_only")}
                >
                  Audio only
                </Button>
              </div>
            </div>

            <Button
              type="button"
              disabled={save.isPending}
              onClick={() =>
                save.mutate({
                  default_output_path: outputPath.trim() || null,
                  default_quality: quality,
                })
              }
            >
              {save.isPending ? "Saving..." : "Save"}
            </Button>

            {save.error && (
              <p className="text-sm text-destructive">
                {save.error instanceof Error ? save.error.message : String(save.error)}
              </p>
            )}
            {save.isSuccess && <p className="text-sm text-muted-foreground">Saved.</p>}
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
