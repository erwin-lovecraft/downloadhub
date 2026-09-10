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
import {
  checkYtdlpCookies,
  JS_RUNTIME_LABELS,
  type CookieCheck,
  type JsRuntime,
} from "@/lib/settings";

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
  const [ytdlpPath, setYtdlpPath] = useState("");
  const [ytdlpCookiesPath, setYtdlpCookiesPath] = useState("");
  const [jsRuntime, setJsRuntime] = useState<JsRuntime>("auto");
  const [enqueueItag, setEnqueueItag] = useState("");
  const [cookieCheck, setCookieCheck] = useState<CookieCheck | null>(null);
  const [checkingCookies, setCheckingCookies] = useState(false);
  const [serverPath, setServerPath] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (settings.data) {
      setOutputPath(settings.data.default_output_path ?? "");
      setQuality(settings.data.default_quality);
      setMcpEnabled(settings.data.mcp_enabled);
      setFfmpegPath(settings.data.ffmpeg_path ?? "");
      setYtdlpPath(settings.data.ytdlp_path ?? "");
      setYtdlpCookiesPath(settings.data.ytdlp_cookies_path ?? "");
      setJsRuntime(settings.data.ytdlp_js_runtime);
      setEnqueueItag(settings.data.enqueue_itag?.toString() ?? "");
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

  /**
   * Runs the real check rather than trusting the path: a cookies file can
   * be present, well-formed, and still rejected by YouTube.
   */
  async function testCookies() {
    setCheckingCookies(true);
    setCookieCheck(null);
    try {
      setCookieCheck(await checkYtdlpCookies(ytdlpCookiesPath));
    } catch (e) {
      setCookieCheck({
        ok: false,
        summary: e instanceof Error ? e.message : String(e),
        problems: [],
      });
    } finally {
      setCheckingCookies(false);
    }
  }

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
      <DialogContent className="flex max-h-[85vh] flex-col gap-4 sm:max-w-md">
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
            {settings.error instanceof Error
              ? settings.error.message
              : String(settings.error)}
          </p>
        )}

        {settings.data && (
          <>
            {/* The fields scroll; Save stays pinned below it, so a long
                settings list can never push it off-screen. */}
            <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pr-1">
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">
                  Default output folder
                </label>
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
                  {(
                    Object.keys(FORMAT_PREFERENCE_LABELS) as FormatPreference[]
                  ).map((option) => (
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
                <label className="text-sm font-medium">
                  Queue itag (optional)
                </label>
                <Input
                  value={enqueueItag}
                  onChange={(e) =>
                    setEnqueueItag(e.target.value.replace(/[^0-9]/g, ""))
                  }
                  placeholder="e.g. 140 — leave blank to match the quality above"
                  inputMode="numeric"
                />
                <p className="text-xs text-muted-foreground">
                  Adding to the queue never inspects a video's formats, so it
                  is instant. Entries record this itag (or one matching the
                  quality above) and the real format is checked when you
                  download. If a download fails on the format, open the
                  entry's format list and pick another.
                </p>
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">
                  ffmpeg path (MP3 conversion)
                </label>
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
                  Leave empty to use the bundled ffmpeg (Windows) or one found
                  on PATH. Applies to the next download — no restart needed.
                </p>
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">yt-dlp path</label>
                <div className="flex gap-2">
                  <Input
                    value={ytdlpPath}
                    onChange={(e) => setYtdlpPath(e.target.value)}
                    placeholder="e.g. /opt/homebrew/bin/yt-dlp"
                  />
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="shrink-0"
                    onClick={async () => {
                      const file = await pickFile();
                      if (file) setYtdlpPath(file);
                    }}
                  >
                    Browse...
                  </Button>
                </div>
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">
                  yt-dlp cookies file
                </label>
                <div className="flex gap-2">
                  <Input
                    value={ytdlpCookiesPath}
                    onChange={(e) => {
                      setYtdlpCookiesPath(e.target.value);
                      setCookieCheck(null);
                    }}
                    placeholder="e.g. /Users/me/Downloads/cookies.txt"
                  />
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="shrink-0"
                    onClick={async () => {
                      const file = await pickFile();
                      if (file) {
                        setYtdlpCookiesPath(file);
                        setCookieCheck(null);
                      }
                    }}
                  >
                    Browse...
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="shrink-0"
                    disabled={!ytdlpCookiesPath.trim() || checkingCookies}
                    onClick={testCookies}
                  >
                    {checkingCookies ? "Testing..." : "Test cookies"}
                  </Button>
                </div>
                {cookieCheck && (
                  <div
                    className={`rounded-md border p-2 text-xs ${
                      cookieCheck.ok
                        ? "border-muted bg-muted/40"
                        : "border-destructive/40 bg-destructive/10"
                    }`}
                  >
                    <p className={cookieCheck.ok ? "" : "text-destructive"}>
                      {cookieCheck.summary}
                    </p>
                    {cookieCheck.problems.length > 0 && (
                      <ul className="mt-1 list-disc pl-4 text-muted-foreground">
                        {cookieCheck.problems.map((problem) => (
                          <li key={problem}>{problem}</li>
                        ))}
                      </ul>
                    )}
                  </div>
                )}
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">
                  JavaScript runtime (yt-dlp)
                </label>
                <div className="flex gap-2">
                  {(Object.keys(JS_RUNTIME_LABELS) as JsRuntime[]).map(
                    (option) => (
                      <Button
                        key={option}
                        type="button"
                        size="sm"
                        variant={jsRuntime === option ? "default" : "outline"}
                        onClick={() => setJsRuntime(option)}
                      >
                        {JS_RUNTIME_LABELS[option]}
                      </Button>
                    ),
                  )}
                </div>
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="flex items-center gap-2 text-sm font-medium">
                  <Checkbox
                    checked={mcpEnabled}
                    onCheckedChange={(checked) =>
                      setMcpEnabled(checked === true)
                    }
                  />
                  Allow AI agent access (MCP server)
                </label>
              </div>

              {mcpEnabled && serverPath && (
                <div className="flex flex-col gap-1.5">
                  <label className="text-sm font-medium">
                    Connect an AI agent
                  </label>
                  <p className="text-xs text-muted-foreground">
                    Add this to your agent's MCP config (Claude Desktop, Claude
                    Code, Gemini CLI, Codex). Fill in your YouTube API key if
                    you want keyword search. See docs/MCP_SETUP.md for per-agent
                    steps.
                  </p>
                  <pre className="max-h-40 overflow-auto rounded-md border bg-muted p-2 text-xs">
                    {buildAgentConfig(serverPath)}
                  </pre>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={copyAgentConfig}
                  >
                    {copied ? "Copied!" : "Copy config"}
                  </Button>
                </div>
              )}
            </div>

            <div className="flex shrink-0 flex-col gap-2 border-t pt-3">
              <Button
                type="button"
                disabled={save.isPending}
                onClick={() =>
                  save.mutate({
                    default_output_path: outputPath.trim() || null,
                    default_quality: quality,
                    mcp_enabled: mcpEnabled,
                    ffmpeg_path: ffmpegPath.trim() || null,
                    ytdlp_path: ytdlpPath.trim() || null,
                    ytdlp_cookies_path: ytdlpCookiesPath.trim() || null,
                    ytdlp_js_runtime: jsRuntime,
                    enqueue_itag: enqueueItag.trim()
                      ? Number(enqueueItag.trim())
                      : null,
                  })
                }
              >
                {save.isPending ? "Saving..." : "Save"}
              </Button>

              {save.error && (
                <p className="text-sm text-destructive">
                  {save.error instanceof Error
                    ? save.error.message
                    : String(save.error)}
                </p>
              )}
              {save.isSuccess && (
                <p className="text-sm text-muted-foreground">Saved.</p>
              )}
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
