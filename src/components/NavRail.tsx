import { Bot, Download, History, LayoutGrid, Search, Settings } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { useSessionStore } from "@/store/session";
import { useQueueStore } from "@/store/queue";
import { useAgentsStore } from "@/store/agents";
import type { NavKey } from "@/types";

interface NavItemProps {
  icon: ReactNode;
  label: string;
  active: boolean;
  badge?: number;
  accent?: "primary" | "agent";
  onClick: () => void;
}

function NavItem({ icon, label, active, badge, accent = "primary", onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "relative flex h-9 w-full items-center gap-2.5 rounded-[4px] px-2.5 text-left text-[13px] transition-colors hover:bg-black/4",
        active ? "font-semibold" : "font-normal text-foreground",
        active && accent === "primary" && "bg-accent text-accent-foreground",
        active && accent === "agent" && "bg-agent-subtle text-agent",
      )}
    >
      {active && (
        <div
          className={cn(
            "absolute left-0 top-1.5 bottom-1.5 w-[3px] rounded-sm",
            accent === "agent" ? "bg-agent" : "bg-primary",
          )}
        />
      )}
      <span className="flex size-4 flex-none items-center justify-center [&>svg]:size-4">
        {icon}
      </span>
      <span className="flex-1">{label}</span>
      {!!badge && badge > 0 && (
        <div
          className={cn(
            "flex h-[18px] min-w-[18px] items-center justify-center rounded-full px-1.5 text-[10px] font-bold text-white",
            accent === "agent" ? "bg-agent" : "bg-primary",
          )}
        >
          {badge}
        </div>
      )}
    </button>
  );
}

export function NavRail() {
  const activeNav = useSessionStore((s) => s.activeNav);
  const navigate = useSessionStore((s) => s.navigate);
  const items = useQueueStore((s) => s.items);
  const pending = useAgentsStore((s) => s.pending);

  const queueBadgeCount = items.filter((it) =>
    ["downloading", "queued", "paused"].includes(it.status),
  ).length;

  const go = (key: NavKey) => () => navigate(key);

  return (
    <div className="flex w-60 flex-none flex-col border-r border-black/6 bg-rail p-2">
      <div className="flex flex-col gap-0.5">
        <NavItem
          icon={<Search />}
          label="Search"
          active={activeNav === "search"}
          onClick={go("search")}
        />
        <NavItem
          icon={<Download />}
          label="Queue"
          active={activeNav === "queue"}
          badge={queueBadgeCount}
          onClick={go("queue")}
        />
        <NavItem
          icon={<History />}
          label="History"
          active={activeNav === "history"}
          onClick={go("history")}
        />
        <NavItem
          icon={<Bot />}
          label="Agent Activity"
          active={activeNav === "agent"}
          badge={pending.length}
          accent="agent"
          onClick={go("agent")}
        />
        <NavItem
          icon={<Settings />}
          label="Settings"
          active={activeNav === "settings"}
          onClick={go("settings")}
        />
      </div>

      <div className="flex-1" />

      <div className="mx-1 my-2 h-px bg-black/6" />
      <NavItem
        icon={<LayoutGrid />}
        label="Component sheet"
        active={activeNav === "components"}
        onClick={go("components")}
      />

      <div className="mt-2 flex items-center gap-2.5 border-t border-black/6 px-2 pt-2.5">
        <div className="flex size-7 flex-none items-center justify-center rounded-full bg-primary text-xs font-bold text-primary-foreground">
          G
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-xs font-semibold text-foreground">Guest User</div>
          <div className="text-[11px] text-muted-foreground">Signed in with Google</div>
        </div>
      </div>
    </div>
  );
}
