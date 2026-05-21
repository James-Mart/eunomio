import { useEffect, useState } from "react";
import { toast } from "sonner";
import { QRCodeSVG } from "qrcode.react";
import { Copy, Loader2, QrCode, Square } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { api, type TunnelStatus } from "@/lib/api";
import { formatError } from "@/lib/errors";
import { subscribeEventSource } from "@/lib/subscribeEventSource";
import { useAbortableEffect } from "@/lib/useAbortableEffect";

export default function MobileShareCard() {
  const [status, setStatus] = useState<TunnelStatus | null>(null);
  const [pending, setPending] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);

  useAbortableEffect(async (signal) => {
    try {
      const s = await api.getTunnel();
      if (!signal.aborted) setStatus(s);
    } catch {
      if (!signal.aborted) setStatus({ state: "idle", tokenRequired: true });
    }
  }, []);

  useEffect(() => {
    let stopped = false;
    const unsub = subscribeEventSource({
      url: "/api/tunnel/events",
      onMessage: (ev) => {
        let payload: TunnelStatus;
        try {
          payload = JSON.parse(ev.data) as TunnelStatus;
        } catch (err) {
          console.error("malformed tunnel SSE payload", err, ev.data);
          return;
        }
        // SSE payloads are token-redacted; preserve any token already
        // loaded via the initial REST fetch (or a prior fetch in this
        // tab). If we're "running" with no token cached anywhere, do
        // a one-off refetch from the local listener so a freshly
        // opened tab can recover the token. Skipped when the backend
        // is in dev-tunnel mode (tokenRequired = false), where the
        // refetch would never return a token.
        setStatus((prev) => {
          const merged: TunnelStatus = { ...(prev ?? {}), ...payload };
          if (
            merged.state === "running" &&
            merged.tokenRequired &&
            !merged.token &&
            !prev?.token
          ) {
            api.getTunnel().then((full) => {
              if (!stopped) setStatus(full);
            }).catch(() => {});
          }
          return merged;
        });
      },
    });
    return () => {
      stopped = true;
      unsub();
    };
  }, []);

  useEffect(() => {
    if (!modalOpen) return;
    if (!status) return;
    if (status.state !== "running" && !pending) {
      setModalOpen(false);
    }
  }, [status, modalOpen, pending]);

  const onStart = async () => {
    setPending(true);
    try {
      const next = await api.startTunnel();
      setStatus(next);
    } catch (e) {
      toast.error(formatError(e, "Failed to start tunnel"));
    } finally {
      setPending(false);
    }
  };

  const onStop = async () => {
    setPending(true);
    try {
      await api.stopTunnel();
      setStatus((prev) => ({
        state: "idle",
        tokenRequired: prev?.tokenRequired ?? true,
      }));
    } catch (e) {
      toast.error(formatError(e, "Failed to stop tunnel"));
    } finally {
      setPending(false);
    }
  };

  const onGetQr = () => {
    setModalOpen(true);
    if (status?.state !== "running") void onStart();
  };

  return (
    <>
      <Card>
        <CardHeader className="p-4 pb-2">
          <div className="flex items-center gap-2">
            <QrCode className="h-4 w-4 text-muted-foreground" aria-hidden="true" />
            <CardTitle className="text-base font-semibold">Continue on mobile</CardTitle>
          </div>
          <CardDescription className="text-xs">
            Scan a QR code on your phone to access this Eunomia instance remotely. Warning:
            the link grants full control, not just view access.
            {status && !status.tokenRequired && (
              <>
                {" "}
                <span className="text-destructive">
                  Dev tunnel: no share token, URL secrecy is the only gate.
                </span>
              </>
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="p-4 pt-2">
          <Controls
            status={status}
            pending={pending}
            onGetQr={onGetQr}
            onStop={onStop}
          />
        </CardContent>
      </Card>

      <Dialog open={modalOpen} onOpenChange={setModalOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Continue on mobile</DialogTitle>
            <DialogDescription>
              Scan this with your phone&rsquo;s camera. Anyone with this link has full control
              of this repo&rsquo;s Eunomia instance &mdash; they can view diffs, accept or
              abandon partitions, change settings, and trigger API-billing runs.
            </DialogDescription>
          </DialogHeader>
          <ModalBody status={status} />
        </DialogContent>
      </Dialog>
    </>
  );
}

function Controls({
  status,
  pending,
  onGetQr,
  onStop,
}: {
  status: TunnelStatus | null;
  pending: boolean;
  onGetQr: () => void;
  onStop: () => void;
}) {
  if (status === null) {
    return <p className="text-xs text-muted-foreground">Loading&hellip;</p>;
  }
  if (status.state === "error") {
    return (
      <div className="space-y-2">
        <p className="text-sm text-destructive">{status.errorMessage ?? "Tunnel error"}</p>
        <Button size="sm" onClick={onGetQr} disabled={pending}>
          {pending ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : "Try again"}
        </Button>
      </div>
    );
  }
  return (
    <div className="flex items-center gap-2">
      <Button size="sm" onClick={onGetQr} disabled={pending}>
        {pending && status.state !== "running" ? (
          <>
            <Loader2 className="mr-2 h-3.5 w-3.5 animate-spin" />
            Allocating tunnel&hellip;
          </>
        ) : (
          <>
            <QrCode className="mr-2 h-3.5 w-3.5" />
            Get QR Code
          </>
        )}
      </Button>
      {status.state === "running" && (
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
      )}
    </div>
  );
}

function ModalBody({ status }: { status: TunnelStatus | null }) {
  const [view, setView] = useState<"qr" | "link">("qr");

  if (status?.state !== "running" || !status.url) {
    return (
      <div className="flex flex-col items-center gap-3 py-6 text-sm text-muted-foreground">
        <Loader2 className="h-5 w-5 animate-spin" />
        Allocating tunnel&hellip;
      </div>
    );
  }
  if (status.tokenRequired && !status.token) {
    return (
      <div className="flex flex-col items-center gap-3 py-6 text-sm text-muted-foreground">
        <Loader2 className="h-5 w-5 animate-spin" />
        Allocating tunnel&hellip;
      </div>
    );
  }
  const shareUrl = buildShareUrl(status.url, status.token ?? null);
  return (
    <div className="flex flex-col gap-4">
      {!status.tokenRequired && (
        <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          Dev tunnel: no share token &mdash; anyone who learns this URL has
          full control. Stop the tunnel when you&rsquo;re done.
        </p>
      )}
      {view === "qr" ? (
        <div className="self-center rounded-lg bg-white p-4">
          <QRCodeSVG value={shareUrl} size={240} marginSize={0} />
        </div>
      ) : (
        <div className="flex items-start gap-2">
          <code className="min-w-0 flex-1 break-all rounded bg-muted px-3 py-2 text-sm">
            {shareUrl}
          </code>
          <Button
            size="icon"
            variant="ghost"
            className="h-8 w-8 shrink-0"
            aria-label="Copy share URL"
            onClick={() => copy(shareUrl)}
          >
            <Copy className="h-3.5 w-3.5" />
          </Button>
        </div>
      )}
      <Button
        variant="link"
        size="sm"
        className="h-auto self-center p-0 text-xs text-muted-foreground"
        onClick={() => setView(view === "qr" ? "link" : "qr")}
      >
        {view === "qr" ? "Show link instead" : "Show QR code instead"}
      </Button>
    </div>
  );
}

function buildShareUrl(url: string, token: string | null): string {
  const base = url.endsWith("/") ? url.slice(0, -1) : url;
  if (token === null) return `${base}/`;
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
