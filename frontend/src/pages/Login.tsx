import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { BrandMark } from "@/components/BrandMark";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { api } from "@/lib/api";
import { formatError } from "@/lib/errors";

type Props = {
  onSuccess: () => void;
  pendingPullRequestUrl?: string | null;
};

export default function Login({ onSuccess, pendingPullRequestUrl }: Props) {
  const passwordRef = useRef<HTMLInputElement>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [useEnvKey, setUseEnvKey] = useState(false);
  const [hasEnvKey, setHasEnvKey] = useState(false);
  const [loadingSetup, setLoadingSetup] = useState(true);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const setup = await api.getAuthSetup();
        if (cancelled) return;
        setUsername(setup.suggestedUsername);
        setHasEnvKey(setup.hasEnvKey);
      } catch (e) {
        if (!cancelled) {
          toast.error(formatError(e, "Failed to load login setup"));
        }
      } finally {
        if (!cancelled) setLoadingSetup(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!loadingSetup) passwordRef.current?.focus();
  }, [loadingSetup]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    try {
      await api.login({
        username: username.trim(),
        cursorApiKey: useEnvKey ? undefined : password,
        useEnvKey: useEnvKey || undefined,
      });
      onSuccess();
    } catch (e) {
      toast.error(formatError(e, "Login failed"));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="flex h-dvh flex-col items-center justify-center bg-background px-4">
      <div className="w-full max-w-sm space-y-6">
        <div className="flex flex-col items-center gap-3 text-center">
          <BrandMark className="text-3xl" />
          <div>
            <h1 className="text-xl font-semibold tracking-tight">Sign in</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              Local account for this Eunomio instance.
            </p>
            {pendingPullRequestUrl ? (
              <p className="mt-3 text-sm text-muted-foreground">
                Sign in to open{" "}
                <span className="font-mono text-xs break-all">{pendingPullRequestUrl}</span>
              </p>
            ) : null}
          </div>
        </div>

        {loadingSetup ? (
          <div className="space-y-4">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : (
          <form onSubmit={submit} className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="login-username">Username</Label>
              <Input
                id="login-username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoComplete="username"
                required
              />
            </div>

            {hasEnvKey ? (
              <div className="flex items-start gap-3 rounded-md border border-border px-3 py-2.5">
                <Checkbox
                  id="login-use-env-key"
                  checked={useEnvKey}
                  onChange={(e) => setUseEnvKey(e.target.checked)}
                  className="mt-0.5"
                />
                <div className="space-y-0.5">
                  <Label htmlFor="login-use-env-key" className="font-normal">
                    Detected key from environment — use it?
                  </Label>
                  <p className="text-xs text-muted-foreground">
                    Uses the Cursor API key from the server environment instead
                    of entering one here.
                  </p>
                </div>
              </div>
            ) : null}

            {!useEnvKey ? (
              <div className="space-y-1.5">
                <Label htmlFor="login-password">Cursor API key</Label>
                <Input
                  ref={passwordRef}
                  id="login-password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="new-password"
                  required={!useEnvKey}
                />
              </div>
            ) : null}

            <Button
              type="submit"
              className="w-full"
              variant="outline"
              disabled={submitting}
            >
              {submitting ? "Signing in…" : "Sign in"}
            </Button>
          </form>
        )}
      </div>
    </div>
  );
}
