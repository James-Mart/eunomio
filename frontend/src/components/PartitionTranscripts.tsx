import { useCallback, useEffect, useMemo, useState } from "react";

import {
  api,
  type Run,
  type RunKind,
  type Transcript,
  type TranscriptMessage,
} from "@/lib/api";
import {
  useRunMessageSubscription,
  type RunMessageEvent,
} from "@/components/SessionEventsProvider";

type Props = {
  partitionId: number;
  runs: Run[];
};

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; transcript: Transcript }
  | { kind: "error"; message: string };

const KIND_LABEL: Record<RunKind, string> = {
  survey: "Surveyor",
  plan: "Planner",
  construct: "Constructor",
};

export default function PartitionTranscripts({ partitionId, runs }: Props) {
  const [enabled, setEnabled] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    void api
      .getPartitionSettings()
      .then((s) => {
        if (!cancelled) setEnabled(s.general.transcriptsEnabled);
      })
      .catch(() => {
        if (!cancelled) setEnabled(false);
      });
    return () => {
      cancelled = true;
    };
  }, [partitionId]);

  if (!enabled) return null;

  const ordered = [...runs].sort((a, b) => a.startedAt - b.startedAt);

  return (
    <section className="space-y-2">
      <h4 className="text-sm font-medium">Transcripts</h4>
      <div className="space-y-2">
        {ordered.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            No runs yet on this partition.
          </p>
        ) : (
          ordered.map((r) => (
            <RunTranscriptItem
              key={r.id}
              partitionId={partitionId}
              run={r}
            />
          ))
        )}
      </div>
    </section>
  );
}

function RunTranscriptItem({
  partitionId,
  run,
}: {
  partitionId: number;
  run: Run;
}) {
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [liveMessages, setLiveMessages] = useState<TranscriptMessage[]>([]);
  const [showStatus, setShowStatus] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setState({ kind: "loading" });
    setLiveMessages([]);
    void api
      .getRunTranscript(partitionId, run.id)
      .then((transcript) => {
        if (!cancelled) setState({ kind: "ready", transcript });
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setState({
          kind: "error",
          message: e instanceof Error ? e.message : "Failed to load transcript",
        });
      });
    return () => {
      cancelled = true;
    };
  }, [partitionId, run.id]);

  const isRunning = run.status === "running";

  const onMessage = useCallback(
    (event: RunMessageEvent) => {
      if (!isRunning) return;
      if (event.partitionId !== partitionId || event.runId !== run.id) return;
      setLiveMessages((prev) => [
        ...prev,
        { seq: prev.length, ts: Math.floor(Date.now() / 1000), message: event.message },
      ]);
    },
    [isRunning, partitionId, run.id],
  );

  useRunMessageSubscription(onMessage);

  const merged = useMemo(() => {
    if (state.kind !== "ready") return liveMessages;
    return [...state.transcript.messages, ...liveMessages];
  }, [state, liveMessages]);

  const entries = useMemo(() => groupMessages(merged), [merged]);

  const kindLabel = KIND_LABEL[run.kind];
  const summary = `${kindLabel} · run #${run.id} · ${run.status}`;

  return (
    <details className="rounded-md border bg-muted/40 text-sm">
      <summary className="cursor-pointer select-none px-3 py-2 font-medium">
        {summary}
      </summary>
      <div className="space-y-3 px-3 pb-3 pt-1">
        {state.kind === "loading" ? (
          <p className="text-xs text-muted-foreground">Loading transcript…</p>
        ) : state.kind === "error" ? (
          <p className="text-xs text-destructive">{state.message}</p>
        ) : (
          <>
            <PromptBlock prompt={state.transcript.prompt} />
            <MessagesBlock
              entries={entries}
              hideStatus={!showStatus}
              showStatus={showStatus}
              onToggleStatus={() => setShowStatus((v) => !v)}
            />
            <ResultBlock transcript={state.transcript} />
          </>
        )}
      </div>
    </details>
  );
}

function PromptBlock({ prompt }: { prompt: string | null }) {
  if (!prompt) {
    return (
      <p className="text-xs text-muted-foreground">
        Prompt not captured for this run.
      </p>
    );
  }
  return (
    <details className="rounded border bg-background">
      <summary className="cursor-pointer select-none px-2 py-1 text-xs font-medium">
        Prompt
      </summary>
      <pre className="overflow-x-auto whitespace-pre-wrap break-words p-2 text-xs">
        {prompt}
      </pre>
    </details>
  );
}

type TranscriptEntry =
  | {
      kind: "thinking";
      startSeq: number;
      endSeq: number;
      ts: number;
      text: string;
    }
  | {
      kind: "assistant";
      startSeq: number;
      endSeq: number;
      ts: number;
      text: string;
    }
  | {
      kind: "tool";
      startSeq: number;
      endSeq: number;
      ts: number;
      name: string;
      callStatus: "running" | "completed";
      exitCode: number | null;
    }
  | { kind: "status"; seq: number; ts: number; status: string }
  | { kind: "unknown"; seq: number; ts: number; type: string; message: unknown };

function MessagesBlock({
  entries,
  hideStatus,
  showStatus,
  onToggleStatus,
}: {
  entries: TranscriptEntry[];
  hideStatus: boolean;
  showStatus: boolean;
  onToggleStatus: () => void;
}) {
  const hasStatus = entries.some((e) => e.kind === "status");
  const visible = hideStatus
    ? entries.filter((e) => e.kind !== "status")
    : entries;
  if (entries.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">
        No stream messages captured.
      </p>
    );
  }
  return (
    <div className="space-y-2">
      {hasStatus ? (
        <button
          type="button"
          onClick={onToggleStatus}
          className="text-xs text-muted-foreground underline-offset-2 hover:underline"
        >
          {showStatus ? "Hide status messages" : "Show status messages"}
        </button>
      ) : null}
      <ol className="space-y-2">
        {visible.map((e) => (
          <EntryRow key={entryKey(e)} entry={e} />
        ))}
      </ol>
    </div>
  );
}

function EntryRow({ entry }: { entry: TranscriptEntry }) {
  switch (entry.kind) {
    case "thinking":
      return <ThinkingEntry entry={entry} />;
    case "assistant":
      return <AssistantEntry entry={entry} />;
    case "tool":
      return <ToolEntry entry={entry} />;
    case "status":
      return <StatusEntry entry={entry} />;
    case "unknown":
      return <UnknownEntry entry={entry} />;
  }
}

function ThinkingEntry({
  entry,
}: {
  entry: Extract<TranscriptEntry, { kind: "thinking" }>;
}) {
  return (
    <li className="rounded border bg-background px-3 py-2">
      <EntryHeader
        seqLabel={formatSeqRange(entry.startSeq, entry.endSeq)}
        ts={entry.ts}
        label="thinking"
      />
      <p className="mt-1 whitespace-pre-wrap break-words text-sm italic text-muted-foreground">
        {entry.text}
      </p>
    </li>
  );
}

function AssistantEntry({
  entry,
}: {
  entry: Extract<TranscriptEntry, { kind: "assistant" }>;
}) {
  return (
    <li className="rounded border bg-background px-3 py-2">
      <EntryHeader
        seqLabel={formatSeqRange(entry.startSeq, entry.endSeq)}
        ts={entry.ts}
        label="assistant"
      />
      <p className="mt-1 whitespace-pre-wrap break-words text-sm">
        {entry.text}
      </p>
    </li>
  );
}

function ToolEntry({
  entry,
}: {
  entry: Extract<TranscriptEntry, { kind: "tool" }>;
}) {
  return (
    <li className="rounded border bg-background px-3 py-2">
      <EntryHeader
        seqLabel={formatSeqRange(entry.startSeq, entry.endSeq)}
        ts={entry.ts}
        label={`tool: ${entry.name}`}
        trailing={toolStatusSuffix(entry.callStatus, entry.exitCode)}
      />
    </li>
  );
}

function StatusEntry({
  entry,
}: {
  entry: Extract<TranscriptEntry, { kind: "status" }>;
}) {
  return (
    <li className="rounded border bg-background px-3 py-2 opacity-60">
      <EntryHeader
        seqLabel={formatSeqRange(entry.seq, entry.seq)}
        ts={entry.ts}
        label={`status: ${entry.status}`}
      />
    </li>
  );
}

function UnknownEntry({
  entry,
}: {
  entry: Extract<TranscriptEntry, { kind: "unknown" }>;
}) {
  return (
    <li className="rounded border bg-background">
      <details>
        <summary className="cursor-pointer select-none px-3 py-2 text-xs">
          <span className="font-mono">
            {formatSeqRange(entry.seq, entry.seq)}
          </span>
          <span className="ml-2 text-muted-foreground">
            {formatTs(entry.ts)}
          </span>
          <span className="ml-2 font-medium">{entry.type}</span>
        </summary>
        <pre className="overflow-x-auto whitespace-pre-wrap break-words p-2 text-xs">
          {JSON.stringify(entry.message, null, 2)}
        </pre>
      </details>
    </li>
  );
}

function EntryHeader({
  seqLabel,
  ts,
  label,
  trailing,
}: {
  seqLabel: string;
  ts: number;
  label: string;
  trailing?: string;
}) {
  return (
    <div className="flex items-baseline gap-2 text-xs">
      <span className="font-mono text-muted-foreground">{seqLabel}</span>
      <span className="text-muted-foreground">{formatTs(ts)}</span>
      <span className="font-medium">{label}</span>
      {trailing ? (
        <span className="text-muted-foreground">— {trailing}</span>
      ) : null}
    </div>
  );
}

function ResultBlock({ transcript }: { transcript: Transcript }) {
  const hasRaw = transcript.rawResult !== null && transcript.rawResult !== "";
  const hasParsed = transcript.parsedResult !== null;
  const hasError = transcript.errorMessage !== null;
  if (!hasRaw && !hasParsed && !hasError) {
    return (
      <p className="text-xs text-muted-foreground">No terminal result yet.</p>
    );
  }
  return (
    <div className="space-y-1">
      {hasError ? (
        <details className="rounded border border-destructive/40 bg-background">
          <summary className="cursor-pointer select-none px-2 py-1 text-xs font-medium text-destructive">
            Error
          </summary>
          <pre className="overflow-x-auto whitespace-pre-wrap break-words p-2 text-xs">
            {transcript.errorMessage}
          </pre>
        </details>
      ) : null}
      {hasParsed ? (
        <details className="rounded border bg-background" open>
          <summary className="cursor-pointer select-none px-2 py-1 text-xs font-medium">
            Parsed result
          </summary>
          <pre className="overflow-x-auto whitespace-pre-wrap break-words p-2 text-xs">
            {JSON.stringify(transcript.parsedResult, null, 2)}
          </pre>
        </details>
      ) : null}
      {hasRaw ? (
        <details className="rounded border bg-background">
          <summary className="cursor-pointer select-none px-2 py-1 text-xs font-medium">
            Raw output
          </summary>
          <pre className="overflow-x-auto whitespace-pre-wrap break-words p-2 text-xs">
            {transcript.rawResult}
          </pre>
        </details>
      ) : null}
    </div>
  );
}

function groupMessages(messages: TranscriptMessage[]): TranscriptEntry[] {
  const entries: TranscriptEntry[] = [];
  const toolIndexById = new Map<string, number>();

  for (const m of messages) {
    let entry: TranscriptEntry | "fold" | null = null;
    try {
      const type = readMessageType(m.message);
      switch (type) {
        case "thinking": {
          const text = extractThinkingText(m.message);
          if (text === null) {
            entry = makeUnknown(m, type);
            break;
          }
          const last = entries[entries.length - 1];
          if (last && last.kind === "thinking") {
            last.text += text;
            last.endSeq = m.seq;
            entry = "fold";
          } else {
            entry = {
              kind: "thinking",
              startSeq: m.seq,
              endSeq: m.seq,
              ts: m.ts,
              text,
            };
          }
          break;
        }
        case "assistant": {
          const text = extractAssistantText(m.message);
          if (text === null) {
            entry = makeUnknown(m, type);
            break;
          }
          const last = entries[entries.length - 1];
          if (last && last.kind === "assistant") {
            last.text += text;
            last.endSeq = m.seq;
            entry = "fold";
          } else {
            entry = {
              kind: "assistant",
              startSeq: m.seq,
              endSeq: m.seq,
              ts: m.ts,
              text,
            };
          }
          break;
        }
        case "tool_call": {
          const parsed = extractToolCall(m.message);
          if (!parsed) {
            entry = makeUnknown(m, type);
            break;
          }
          const existingIdx =
            parsed.callId !== null ? toolIndexById.get(parsed.callId) : undefined;
          if (existingIdx !== undefined) {
            const existing = entries[existingIdx];
            if (existing.kind === "tool") {
              existing.callStatus = parsed.callStatus;
              if (parsed.exitCode !== null) existing.exitCode = parsed.exitCode;
              existing.endSeq = m.seq;
              entry = "fold";
              break;
            }
          }
          entry = {
            kind: "tool",
            startSeq: m.seq,
            endSeq: m.seq,
            ts: m.ts,
            name: parsed.name,
            callStatus: parsed.callStatus,
            exitCode: parsed.exitCode,
          };
          break;
        }
        case "status": {
          entry = {
            kind: "status",
            seq: m.seq,
            ts: m.ts,
            status: extractStatus(m.message) ?? "unknown",
          };
          break;
        }
        default:
          entry = makeUnknown(m, type);
      }
    } catch {
      entry = makeUnknown(m, "unknown");
    }

    if (entry === "fold" || entry === null) continue;

    if (entry.kind === "tool") {
      const callId = readToolCallId(m.message);
      if (callId) toolIndexById.set(callId, entries.length);
    }
    entries.push(entry);
  }

  return entries;
}

function readMessageType(value: unknown): string {
  if (value && typeof value === "object" && "type" in value) {
    const t = (value as { type?: unknown }).type;
    if (typeof t === "string") return t;
  }
  return "unknown";
}

function extractThinkingText(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const t = (value as { text?: unknown }).text;
  return typeof t === "string" ? t : null;
}

function extractAssistantText(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const msg = (value as { message?: unknown }).message;
  if (!msg || typeof msg !== "object") return null;
  const content = (msg as { content?: unknown }).content;
  if (!Array.isArray(content)) return null;
  const parts: string[] = [];
  for (const block of content) {
    if (!block || typeof block !== "object") continue;
    const blockType = (block as { type?: unknown }).type;
    if (blockType === "text") {
      const text = (block as { text?: unknown }).text;
      if (typeof text === "string") parts.push(text);
    } else if (typeof blockType === "string") {
      parts.push(`[${blockType}]`);
    }
  }
  return parts.join("");
}

type ParsedToolCall = {
  name: string;
  callId: string | null;
  callStatus: "running" | "completed";
  exitCode: number | null;
};

function extractToolCall(value: unknown): ParsedToolCall | null {
  if (!value || typeof value !== "object") return null;
  const obj = value as {
    name?: unknown;
    call_id?: unknown;
    status?: unknown;
    result?: unknown;
  };
  const name = typeof obj.name === "string" ? obj.name : "?";
  const callId = typeof obj.call_id === "string" ? obj.call_id : null;
  const callStatus: "running" | "completed" =
    obj.status === "completed" ? "completed" : "running";
  let exitCode: number | null = null;
  if (obj.result && typeof obj.result === "object") {
    const result = obj.result as { value?: unknown };
    if (result.value && typeof result.value === "object") {
      const code = (result.value as { exitCode?: unknown }).exitCode;
      if (typeof code === "number") exitCode = code;
    }
  }
  return { name, callId, callStatus, exitCode };
}

function readToolCallId(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const id = (value as { call_id?: unknown }).call_id;
  return typeof id === "string" ? id : null;
}

function extractStatus(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const s = (value as { status?: unknown }).status;
  return typeof s === "string" ? s : null;
}

function makeUnknown(
  m: TranscriptMessage,
  type: string,
): Extract<TranscriptEntry, { kind: "unknown" }> {
  return { kind: "unknown", seq: m.seq, ts: m.ts, type, message: m.message };
}

function toolStatusSuffix(
  callStatus: "running" | "completed",
  exitCode: number | null,
): string {
  if (callStatus === "running") return "running";
  if (exitCode === null) return "completed";
  return `exit ${exitCode}`;
}

function formatSeqRange(start: number, end: number): string {
  return start === end ? `#${start}` : `#${start}..#${end}`;
}

function entryKey(entry: TranscriptEntry): string {
  switch (entry.kind) {
    case "thinking":
    case "assistant":
    case "tool":
      return `${entry.kind}-${entry.startSeq}`;
    case "status":
    case "unknown":
      return `${entry.kind}-${entry.seq}`;
  }
}

function formatTs(ts: number): string {
  try {
    return new Date(ts * 1000).toLocaleTimeString();
  } catch {
    return "";
  }
}
