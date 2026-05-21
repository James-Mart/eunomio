import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronRight } from "lucide-react";

import {
  api,
  type Run,
  type Transcript,
  type TranscriptMessage,
} from "@/lib/api";
import { useRunMessageSubscription } from "@/components/SessionEventsProvider";
import { cn } from "@/lib/utils";

type Props = {
  partitionId: number;
  runs: Run[];
};

export default function PartitionTranscripts({ partitionId, runs }: Props) {
  const [enabled, setEnabled] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    void api.getPartitionSettings().then((s) => {
      if (!cancelled) setEnabled(s.general.transcriptsEnabled);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const sorted = useMemo(
    () => [...runs].sort((a, b) => a.startedAt - b.startedAt),
    [runs],
  );

  if (enabled !== true || sorted.length === 0) return null;

  return (
    <div className="space-y-2 border-t pt-4">
      <h4 className="text-sm font-medium">Transcripts</h4>
      <div className="space-y-2">
        {sorted.map((run) => (
          <RunTranscriptSection
            key={run.id}
            partitionId={partitionId}
            run={run}
          />
        ))}
      </div>
    </div>
  );
}

function RunTranscriptSection({
  partitionId,
  run,
}: {
  partitionId: number;
  run: Run;
}) {
  const [open, setOpen] = useState(false);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [liveMessages, setLiveMessages] = useState<TranscriptMessage[]>([]);
  const [loading, setLoading] = useState(false);

  const isRunning = run.status === "running";

  useRunMessageSubscription(
    useCallback(
      (ev) => {
        if (ev.partitionId !== partitionId || ev.runId !== run.id) return;
        setLiveMessages((prev) => [
          ...prev,
          {
            seq: prev.length,
            ts: Math.floor(Date.now() / 1000),
            message: ev.message,
          },
        ]);
      },
      [partitionId, run.id],
    ),
  );

  useEffect(() => {
    if (!open || transcript || loading) return;
    setLoading(true);
    void api
      .getRunTranscript(partitionId, run.id)
      .then((t) => setTranscript(t))
      .finally(() => setLoading(false));
  }, [open, transcript, loading, partitionId, run.id]);

  const messages =
    isRunning && liveMessages.length > 0
      ? liveMessages
      : (transcript?.messages ?? []);

  const label = `${run.kind} run #${run.id}`;

  return (
    <div className="rounded-md border bg-muted/30">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left"
      >
        <ChevronRight
          className={cn(
            "h-3.5 w-3.5 shrink-0 transition-transform",
            open && "rotate-90",
          )}
          aria-hidden="true"
        />
        <span className="text-sm font-medium capitalize">{label}</span>
        {isRunning && (
          <span className="ml-auto text-xs text-muted-foreground">running</span>
        )}
      </button>
      {open && (
        <div className="space-y-3 px-3 pb-3 pl-8">
          {loading && !transcript ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <>
              {transcript?.prompt && (
                <div>
                  <p className="mb-1 text-xs font-medium text-muted-foreground">
                    Prompt
                  </p>
                  <pre className="max-h-40 overflow-auto rounded bg-background p-2 text-xs whitespace-pre-wrap">
                    {transcript.prompt}
                  </pre>
                </div>
              )}
              <div>
                <p className="mb-1 text-xs font-medium text-muted-foreground">
                  Messages ({messages.length})
                </p>
                {messages.length === 0 ? (
                  <p className="text-xs text-muted-foreground">No messages.</p>
                ) : (
                  <div className="max-h-60 space-y-2 overflow-auto">
                    {messages.map((m) => (
                      <pre
                        key={m.seq}
                        className="rounded bg-background p-2 text-xs whitespace-pre-wrap"
                      >
                        {JSON.stringify(m.message, null, 2)}
                      </pre>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
