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
import { Checkbox } from "@/components/ui/checkbox";
import { pickFile, pickOutputFolder } from "@/lib/dialog";
import { buildAgentConfig, mcpServerPath } from "@/lib/mcp";
import { FORMAT_PREFERENCE_LABELS, type FormatPreference } from "@/lib/enqueue";

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
  const [mcpEnabled, setMcpEnabled] = useState(true);
  const [ffmpegPath, setFfmpegPath] = useState("");
  const [serverPath, setServerPath] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (settings.data) {
      setOutputPath(settings.data.default_output_path ?? "");
      setQuality(settings.data.default_quality);
      setMcpEnabled(settings.data.mcp_enabled);
      setFfmpegPath(settings.data.ffmpeg_path ?? "");
    }
  }, [settings.data]);

  useEffect(() => {
    if (open) {
      mcpServerPath()
        .then(setServerPath)
        .catch(() => setServerPath(null));
      setCopied(false);
    }
  }, [open]);

  async function copyAgentConfig() {
    if (!serverPath) return;
    try {
      await navigator.clipboard.writeText(buildAgentConfig(serverPath));
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

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
                {(Object.keys(FORMAT_PREFERENCE_LABELS) as FormatPreference[]).map((option) => (
                  <Button
                    key={option}
                    type="button"
                    size="sm"
                    variant={quality === option ? "default" : "outline"}
                    onClick={() => setQuality(option)}
                  >
                    {FORMAT_PREFERENCE_LABELS[option]}
                  </Button>
                ))}
              </div>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-sm font-medium">ffmpeg path (MP3 conversion)</label>
              <div className="flex gap-2">
                <Input
                  value={ffmpegPath}
                  onChange={(e) => setFfmpegPath(e.target.value)}
                  placeholder="e.g. /opt/homebrew/bin/ffmpeg"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="shrink-0"
                  onClick={async () => {
                    const file = await pickFile();
                    if (file) setFfmpegPath(file);
                  }}
                >
                  Browse...
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Leave empty to use the bundled ffmpeg (Windows) or one found on
                PATH. Applies to the next download — no restart needed.
              </p>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="flex items-center gap-2 text-sm font-medium">
                <Checkbox
                  checked={mcpEnabled}
                  onCheckedChange={(checked) => setMcpEnabled(checked === true)}
                />
                Allow AI agent access (MCP server)
              </label>
              <p className="text-xs text-muted-foreground">
                Lets external AI agents search YouTube and add entries to your
                queue directly. They cannot start downloads — only you can,
                with "Download all". Review the queue before starting it.
              </p>
            </div>

            {mcpEnabled && serverPath && (
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">Connect an AI agent</label>
                <p className="text-xs text-muted-foreground">
                  Add this to your agent's MCP config (Claude Desktop, Claude
                  Code, Gemini CLI, Codex). Fill in your YouTube API key if you
                  want keyword search. See docs/MCP_SETUP.md for per-agent steps.
                </p>
                <pre className="max-h-40 overflow-auto rounded-md border bg-muted p-2 text-xs">
                  {buildAgentConfig(serverPath)}
                </pre>
                <Button type="button" variant="outline" size="sm" onClick={copyAgentConfig}>
                  {copied ? "Copied!" : "Copy config"}
                </Button>
              </div>
            )}

            <Button
              type="button"
              disabled={save.isPending}
              onClick={() =>
                save.mutate({
                  default_output_path: outputPath.trim() || null,
                  default_quality: quality,
                  mcp_enabled: mcpEnabled,
                  ffmpeg_path: ffmpegPath.trim() || null,
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
