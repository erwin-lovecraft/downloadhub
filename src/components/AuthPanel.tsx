import { useAuth } from "@/hooks/useAuth";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";

export function AuthPanel() {
  const { status, login, logout } = useAuth();

  if (status.isLoading) {
    return <p className="text-sm text-muted-foreground">Checking sign-in status...</p>;
  }

  const user = status.data ?? null;
  const error = login.error ?? status.error;

  if (!user) {
    return (
      <div className="flex flex-col items-center gap-3">
        <Button onClick={() => login.mutate()} disabled={login.isPending}>
          {login.isPending ? "Waiting for Google..." : "Sign in with Google"}
        </Button>
        {error && (
          <p className="max-w-sm text-center text-sm text-destructive">
            {error instanceof Error ? error.message : String(error)}
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="flex items-center gap-3">
      <Avatar>
        <AvatarImage src={user.picture ?? undefined} alt={user.name ?? user.email} />
        <AvatarFallback>{(user.name ?? user.email).slice(0, 1).toUpperCase()}</AvatarFallback>
      </Avatar>
      <div className="flex flex-col">
        <span className="text-sm font-medium">{user.name ?? user.email}</span>
        <span className="text-xs text-muted-foreground">{user.email}</span>
      </div>
      <Button variant="outline" size="sm" onClick={() => logout.mutate()} disabled={logout.isPending}>
        Sign out
      </Button>
    </div>
  );
}
