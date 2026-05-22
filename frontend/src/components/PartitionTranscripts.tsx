import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronRightIcon, CopyIcon } from "@primer/octicons-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { useTranscriptDeltaSubscription } from "@/components/SessionEventsProvider";
import { api, type Run, type Transcript } from "@/lib/api";
import { cn } from "@/lib/utils";

type Props = {
  partitionId: number;
  runs: Run[];
};

const STICKY_TAIL_PX = 40;

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success("Copied");
  } catch {
    toast.error("Copy failed");
  }
}

function CopyTextButton({
  text,
  ariaLabel,
}: {
  text: string;
  ariaLabel: string;
}) {
  return (
    <Button
      size="icon"
      variant="ghost"
      className="h-8 w-8 shrink-0"
      aria-label={ariaLabel}
      onClick={() => void copyText(text)}
    >
      <CopyIcon className="h-3.5 w-3.5" />
    </Button>
  );
}

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
            transcriptsEnabled
          />
        ))}
      </div>
    </div>
  );
}

function RunTranscriptSection({
  partitionId,
  run,
  transcriptsEnabled,
}: {
  partitionId: number;
  run: Run;
  transcriptsEnabled: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [liveText, setLiveText] = useState("");
  const [loading, setLoading] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  const liveFetchDone = useRef(false);

  const isRunning = run.status === "running";
  const useLiveDeltas = isRunning && transcriptsEnabled;

  useTranscriptDeltaSubscription(
    useCallback(
      (ev) => {
        if (ev.partitionId !== partitionId || ev.runId !== run.id) return;
        setLiveText((prev) => prev + ev.text);
      },
      [partitionId, run.id],
    ),
  );

  useEffect(() => {
    if (!open) {
      liveFetchDone.current = false;
      return;
    }
    if (useLiveDeltas && liveFetchDone.current) return;
    if (useLiveDeltas) liveFetchDone.current = true;
    setLoading(true);
    void api
      .getRunTranscript(partitionId, run.id)
      .then((t) => {
        setTranscript(t);
        if (useLiveDeltas) {
          setLiveText(t.transcriptText ?? "");
        }
      })
      .finally(() => setLoading(false));
  }, [open, partitionId, run.id, useLiveDeltas]);

  useEffect(() => {
    if (!open || useLiveDeltas) return;
    void api.getRunTranscript(partitionId, run.id).then((t) => {
      setTranscript(t);
      setLiveText("");
    });
  }, [open, useLiveDeltas, partitionId, run.id]);

  const outputText = isRunning
    ? liveText
    : (transcript?.transcriptText ?? "");

  useEffect(() => {
    if (!isRunning || !stickToBottom.current) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [outputText, isRunning]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    stickToBottom.current =
      el.scrollHeight - el.scrollTop - el.clientHeight < STICKY_TAIL_PX;
  };

  const label = `${run.kind} run`;

  return (
    <div className="rounded-md border bg-muted/30">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left"
      >
        <ChevronRightIcon
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
                  <p className="mb-1 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                    Prompt
                    <CopyTextButton
                      text={transcript.prompt}
                      ariaLabel="Copy prompt"
                    />
                  </p>
                  <pre className="max-h-40 overflow-auto rounded bg-background p-2 text-xs whitespace-pre-wrap">
                    {transcript.prompt}
                  </pre>
                </div>
              )}
              <div>
                <p className="mb-1 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                  Output
                  {outputText ? (
                    <CopyTextButton
                      text={outputText}
                      ariaLabel="Copy output"
                    />
                  ) : null}
                  {isRunning && (
                    <span
                      className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-primary"
                      aria-hidden="true"
                    />
                  )}
                </p>
                {!outputText ? (
                  <p className="text-xs text-muted-foreground">
                    No output yet.
                  </p>
                ) : (
                  <div
                    ref={scrollRef}
                    onScroll={onScroll}
                    className="max-h-60 overflow-auto rounded bg-background p-2"
                  >
                    <pre className="text-xs whitespace-pre-wrap">
                      {outputText}
                    </pre>
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
