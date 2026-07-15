import { AuthPanel } from "@/components/AuthPanel";
import { SearchPanel } from "@/components/SearchPanel";
import { QueuePanel } from "@/components/QueuePanel";
import { useDownloadProgressListener } from "@/hooks/useDownloadProgressListener";

function App() {
  useDownloadProgressListener();

  return (
    <main className="flex min-h-screen flex-col items-center gap-6 bg-background p-10 text-foreground">
      <h1 className="text-xl font-semibold">downloadhub</h1>
      <AuthPanel />
      <SearchPanel />
      <QueuePanel />
    </main>
  );
}

export default App;
