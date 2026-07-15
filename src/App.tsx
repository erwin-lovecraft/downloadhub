import { useState } from "react";
import { AuthPanel } from "@/components/AuthPanel";
import { SearchPanel } from "@/components/SearchPanel";
import { QueuePanel } from "@/components/QueuePanel";
import { AgentActionsPanel } from "@/components/AgentActionsPanel";
import { SettingsDialog } from "@/components/SettingsDialog";
import { Button } from "@/components/ui/button";
import { useDownloadProgressListener } from "@/hooks/useDownloadProgressListener";

function App() {
  useDownloadProgressListener();
  const [settingsOpen, setSettingsOpen] = useState(false);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex shrink-0 items-center justify-between border-b px-6 py-3">
        <h1 className="text-lg font-semibold">downloadhub</h1>
        <div className="flex items-center gap-3">
          <Button variant="outline" size="sm" onClick={() => setSettingsOpen(true)}>
            Settings
          </Button>
          <AuthPanel />
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
