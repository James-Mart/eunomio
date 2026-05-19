import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Copy, Link2, Loader2, Square } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { api, ApiError, type TunnelStatus } from "@/lib/api";

export default function PublicUrlCard() {
  const [status, setStatus] = useState<TunnelStatus | null>(null);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .getTunnel()
      .then((s) => {
        if (!cancelled) setStatus(s);
      })
      .catch(() => {
        if (!cancelled) setStatus({ state: "idle" });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let stopped = false;
    let source: EventSource | null = null;
    let reconnectTimer: number | null = null;
    let attempt = 0;

    const connect = () => {
      if (stopped) return;
      source = new EventSource("/api/tunnel/events");
      source.onopen = () => {
        attempt = 0;
      };
      source.onmessage = (ev) => {
        try {
          setStatus(JSON.parse(ev.data) as TunnelStatus);
        } catch (err) {
          console.error("malformed tunnel SSE payload", err, ev.data);
        }
      };
      source.onerror = () => {
        if (stopped) return;
        source?.close();
        source = null;
        const delay = Math.min(1000 * 2 ** Math.min(attempt++, 5), 30_000);
        reconnectTimer = window.setTimeout(connect, delay);
      };
    };
    connect();
    return () => {
      stopped = true;
      if (reconnectTimer != null) window.clearTimeout(reconnectTimer);
      source?.close();
    };
  }, []);

  const onStart = async () => {
    setPending(true);
    try {
      const next = await api.startTunnel();
      setStatus(next);
    } catch (e) {
      const msg = formatError(e, "Failed to start tunnel");
      toast.error(msg);
    } finally {
      setPending(false);
    }
  };

  const onStop = async () => {
    setPending(true);
    try {
      await api.stopTunnel();
      setStatus({ state: "idle" });
    } catch (e) {
      toast.error(formatError(e, "Failed to stop tunnel"));
    } finally {
      setPending(false);
    }
  };

  return (
    <Card>
      <CardHeader className="p-4 pb-2">
        <div className="flex items-center gap-2">
          <Link2 className="h-4 w-4 text-muted-foreground" aria-hidden="true" />
          <CardTitle className="text-base font-semibold">Public URL</CardTitle>
        </div>
        <CardDescription className="text-xs">
          Open a Cloudflare quick tunnel so you can view this session from another device.
        </CardDescription>
      </CardHeader>
      <CardContent className="p-4 pt-2">
        <Body
          status={status}
          pending={pending}
          onStart={onStart}
          onStop={onStop}
        />
      </CardContent>
    </Card>
  );
}

function Body({
  status,
  pending,
  onStart,
  onStop,
}: {
  status: TunnelStatus | null;
  pending: boolean;
  onStart: () => void;
  onStop: () => void;
}) {
  if (status === null) {
    return <p className="text-xs text-muted-foreground">Loading…</p>;
  }
  if (status.state === "running" && status.url && status.token) {
    return <RunningView status={status} pending={pending} onStop={onStop} />;
  }
  if (status.state === "error") {
    return (
      <div className="space-y-2">
        <p className="text-sm text-destructive">{status.errorMessage ?? "Tunnel error"}</p>
        <Button size="sm" onClick={onStart} disabled={pending}>
          {pending ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : "Try again"}
        </Button>
      </div>
    );
  }
  return (
    <Button size="sm" onClick={onStart} disabled={pending}>
      {pending ? (
        <>
          <Loader2 className="mr-2 h-3.5 w-3.5 animate-spin" />
          Allocating tunnel…
        </>
      ) : (
        "Get Link"
      )}
    </Button>
  );
}

function RunningView({
  status,
  pending,
  onStop,
}: {
  status: TunnelStatus;
  pending: boolean;
  onStop: () => void;
}) {
  const shareUrl = buildShareUrl(status.url!, status.token!);
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <code className="flex-1 truncate rounded bg-muted px-2 py-1 text-xs">{shareUrl}</code>
        <Button
          size="icon"
          variant="ghost"
          className="h-7 w-7"
          aria-label="Copy public URL"
          onClick={() => copy(shareUrl)}
        >
          <Copy className="h-3.5 w-3.5" />
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Anyone with this link can view every session for this repo. Stop the tunnel to invalidate it.
      </p>
      <Button size="sm" variant="outline" onClick={onStop} disabled={pending}>
        {pending ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
        ) : (
          <>
            <Square className="mr-2 h-3.5 w-3.5" />
            Stop
          </>
        )}
      </Button>
    </div>
  );
}

function buildShareUrl(url: string, token: string): string {
  const base = url.endsWith("/") ? url.slice(0, -1) : url;
  return `${base}/?eunomia_token=${token}`;
}

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success("Copied");
  } catch {
    toast.error("Copy failed");
  }
}

function formatError(e: unknown, fallback: string): string {
  if (e instanceof ApiError) return e.message;
  if (e instanceof Error) return e.message;
  return fallback;
}
