import { Sheet, SheetContent, SheetTitle } from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useSessionStore } from "@/store/session";
import { useQueueStore } from "@/store/queue";
import { FORMAT_TABLE } from "@/data/mock";

export function VideoDetailDrawer() {
  const detailOpen = useSessionStore((s) => s.detailOpen);
  const closeDetail = useSessionStore((s) => s.closeDetail);
  const selectedVideo = useSessionStore((s) => s.selectedVideo);
  const selectedFormat = useSessionStore((s) => s.selectedFormat);
  const selectFormat = useSessionStore((s) => s.selectFormat);
  const addFromVideo = useQueueStore((s) => s.addFromVideo);

  if (!selectedVideo) return null;

  const format = FORMAT_TABLE.find((f) => f.key === selectedFormat) ?? FORMAT_TABLE[1];

  return (
    <Sheet open={detailOpen} onOpenChange={(open) => !open && closeDetail()}>
      <SheetContent
        side="right"
        className="top-8 h-[calc(100%-2rem)] w-[420px] gap-0 overflow-y-auto rounded-none p-0 sm:max-w-none"
      >
        <SheetTitle className="sr-only">{selectedVideo.title}</SheetTitle>
        <div
          className="aspect-video flex-none"
          style={{ background: selectedVideo.thumbGradient }}
        />
        <div className="flex flex-1 flex-col gap-1.5 p-5">
          <div className="text-base font-semibold leading-tight">{selectedVideo.title}</div>
          <div className="text-xs text-muted-foreground">
            {selectedVideo.channel} · {selectedVideo.subs}
          </div>
          <div className="text-[11px] text-muted-foreground/80">{selectedVideo.uploaded}</div>
          <div className="mt-2 text-xs leading-relaxed text-muted-foreground">
            {selectedVideo.desc} <span className="cursor-pointer text-primary">Show more</span>
          </div>
          <div className="my-3.5 h-px bg-black/6" />
          <div className="mb-2 text-[13px] font-semibold">Format &amp; quality</div>
          {FORMAT_TABLE.map((f) => (
            <div
              key={f.key}
              onClick={() => selectFormat(f.key)}
              className={cn(
                "mb-1.5 flex cursor-pointer items-center justify-between rounded-[6px] border px-3 py-2.5",
                f.key === selectedFormat ? "border-primary bg-accent" : "border-black/6 bg-card",
              )}
            >
              <div>
                <div className="text-[13px] font-semibold">{f.res}</div>
                <div className="text-[11px] text-muted-foreground">
                  {f.type} · itag {f.itag}
                </div>
              </div>
              <div className="text-xs text-muted-foreground">{f.size}</div>
            </div>
          ))}
        </div>
        <div className="flex-none border-t border-black/6 p-4">
          <Button
            className="h-10 w-full text-sm font-semibold"
            onClick={() => addFromVideo(selectedVideo, format)}
          >
            Add to Queue
          </Button>
        </div>
      </SheetContent>
    </Sheet>
  );
}
