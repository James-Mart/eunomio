import type { PartitionStrategy } from "@/lib/api";

export type PhaseName = "survey" | "plan" | "construct";
export type PhaseState =
  | "pending"
  | "running"
  | "awaiting_review"
  | "done"
  | "error";

export type SessionEvent =
  | {
      type: "started";
      sessionId: string;
      targetNodeId: string;
      strategy: PartitionStrategy;
      userConcern: string | null;
    }
  | {
      type: "phase";
      sessionId: string;
      targetNodeId: string;
      name: PhaseName;
      state: PhaseState;
      payload?: unknown;
    }
  | {
      type: "sdkMessage";
      sessionId: string;
      targetNodeId: string;
      message: unknown;
    }
  | {
      type: "loopProgress";
      sessionId: string;
      targetNodeId: string;
      itemId: string;
      status: string;
    }
  | { type: "finished"; sessionId: string; targetNodeId: string }
  | { type: "cancelled"; sessionId: string; targetNodeId: string }
  | {
      type: "error";
      sessionId: string;
      targetNodeId: string;
      code: string;
      message: string;
    };

export type ConnectionStatus = "connecting" | "open" | "closed";

export function subscribeSessionEvents(
  sessionId: string,
  onEvent: (e: SessionEvent) => void,
  onConnectionChange?: (status: ConnectionStatus) => void,
): () => void {
  let source: EventSource | null = null;
  let reconnectTimer: number | null = null;
  let attempt = 0;
  let stopped = false;

  const connect = () => {
    if (stopped) return;
    onConnectionChange?.("connecting");
    source = new EventSource(`/api/sessions/${sessionId}/events`);
    source.onopen = () => {
      attempt = 0;
      onConnectionChange?.("open");
    };
    source.onmessage = (ev) => {
      try {
        onEvent(JSON.parse(ev.data) as SessionEvent);
      } catch (err) {
        console.error("malformed SSE payload", err, ev.data);
      }
    };
    source.onerror = () => {
      if (stopped) return;
      onConnectionChange?.("closed");
      source?.close();
      source = null;
      const delay = backoffMs(attempt++);
      reconnectTimer = window.setTimeout(connect, delay);
    };
  };

  connect();

  return () => {
    stopped = true;
    if (reconnectTimer != null) window.clearTimeout(reconnectTimer);
    source?.close();
  };
}

function backoffMs(attempt: number): number {
  const capped = Math.min(attempt, 5);
  return Math.min(1000 * 2 ** capped, 30_000);
}
