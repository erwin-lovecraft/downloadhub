import { AuthPanel } from "@/components/AuthPanel";
import { SearchPanel } from "@/components/SearchPanel";
import { QueuePanel } from "@/components/QueuePanel";
import { useDownloadProgressListener } from "@/hooks/useDownloadProgressListener";

function App() {
  useDownloadProgressListener();

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex shrink-0 items-center justify-between border-b px-6 py-3">
        <h1 className="text-lg font-semibold">downloadhub</h1>
        <AuthPanel />
      </header>

      <div className="flex min-h-0 flex-1 gap-6 overflow-hidden p-6">
        <section className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <SearchPanel />
        </section>

        <aside className="flex w-80 shrink-0 flex-col overflow-hidden border-l pl-6">
          <QueuePanel />
        </aside>
      </div>
    </div>
  );
}

export default App;
