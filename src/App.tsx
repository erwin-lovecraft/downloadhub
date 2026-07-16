import { useState } from "react";
import { SettingsIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SearchPanel } from "@/components/SearchPanel";
import { QueuePanel } from "@/components/QueuePanel";
import { AgentActionsPanel } from "@/components/AgentActionsPanel";
import { SettingsDialog } from "@/components/SettingsDialog";
import { UpdateChecker } from "@/components/UpdateChecker";
import { useDownloadProgressListener } from "@/hooks/useDownloadProgressListener";

function App() {
  useDownloadProgressListener();
  const [settingsOpen, setSettingsOpen] = useState(false);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex shrink-0 items-center justify-between border-b bg-card px-6 py-3">
        <h1 className="text-lg font-semibold">DownloadHub</h1>
        <div className="flex items-center gap-1">
          <UpdateChecker />
          <Button variant="ghost" size="sm" className="gap-2" onClick={() => setSettingsOpen(true)}>
            <SettingsIcon className="size-4" />
            Settings
          </Button>
        </div>
      </header>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />

      <div className="flex min-h-0 flex-1 gap-6 overflow-hidden p-6">
        <section className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <SearchPanel />
        </section>

        <aside className="flex w-80 shrink-0 flex-col gap-3 overflow-hidden border-l pl-6">
          <AgentActionsPanel />
          <QueuePanel />
        </aside>
      </div>
    </div>
  );
}

export default App;
