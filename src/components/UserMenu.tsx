import { useAuth } from "@/hooks/useAuth";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { ChevronDownIcon, LogInIcon, LogOutIcon, SettingsIcon, UserIcon } from "lucide-react";

export function UserMenu({ onOpenSettings }: { onOpenSettings: () => void }) {
  const { status, login, logout } = useAuth();

  const user = status.data ?? null;
  const error = login.error ?? status.error ?? logout.error;
  const displayName = user ? (user.name ?? user.email) : "Anonymous";

  return (
    <div className="flex items-center gap-2">
      {error && (
        <p className="max-w-56 truncate text-sm text-destructive" title={error instanceof Error ? error.message : String(error)}>
          {error instanceof Error ? error.message : String(error)}
        </p>
      )}

      <DropdownMenu>
        <DropdownMenuTrigger
          disabled={status.isLoading}
          className={cn(
            buttonVariants({ variant: "ghost" }),
            "h-9 gap-2 pr-2 pl-1.5 data-[popup-open]:bg-muted data-[popup-open]:text-foreground"
          )}
        >
          <Avatar size="sm">
            {user?.picture && <AvatarImage src={user.picture} alt={displayName} />}
            <AvatarFallback>
              {user ? displayName.slice(0, 1).toUpperCase() : <UserIcon className="size-3.5" />}
            </AvatarFallback>
          </Avatar>
          <span className="max-w-36 truncate">{displayName}</span>
          <ChevronDownIcon className="size-3.5 text-muted-foreground" />
        </DropdownMenuTrigger>

        <DropdownMenuContent align="end" className="w-56">
          <div className="flex items-center gap-2 px-1.5 py-1.5">
            <Avatar size="sm">
              {user?.picture && <AvatarImage src={user.picture} alt={displayName} />}
              <AvatarFallback>
                {user ? displayName.slice(0, 1).toUpperCase() : <UserIcon className="size-3.5" />}
              </AvatarFallback>
            </Avatar>
            <div className="flex min-w-0 flex-col">
              <span className="truncate text-sm font-medium">{displayName}</span>
              {user && <span className="truncate text-xs text-muted-foreground">{user.email}</span>}
            </div>
          </div>

          <DropdownMenuSeparator />

          <DropdownMenuItem onClick={onOpenSettings}>
            <SettingsIcon />
            Settings
          </DropdownMenuItem>

          <DropdownMenuSeparator />

          {user ? (
            <DropdownMenuItem
              variant="destructive"
              disabled={logout.isPending}
              onClick={() => logout.mutate()}
            >
              <LogOutIcon />
              {logout.isPending ? "Signing out..." : "Sign out"}
            </DropdownMenuItem>
          ) : (
            <DropdownMenuItem disabled={login.isPending} onClick={() => login.mutate()}>
              <LogInIcon />
              {login.isPending ? "Waiting for Google..." : "Sign in with Google"}
            </DropdownMenuItem>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
