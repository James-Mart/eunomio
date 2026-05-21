import type { PhaseName, PhaseState } from "@/lib/api";
import {
  subscribeEventSource,
  type ConnectionStatus,
} from "@/lib/subscribeEventSource";

export type { PhaseName, PhaseState, ConnectionStatus };

export type SessionEvent =
  | {
      type: "started";
      sessionId: string;
      targetNodeId: string;
      partitionId: number;
    }
  | {
      type: "phase";
      sessionId: string;
      targetNodeId: string;
      partitionId: number;
      name: PhaseName;
      state: PhaseState;
      payload?: unknown;
    }
  | {
      type: "sdkMessage";
      sessionId: string;
      targetNodeId: string;
      partitionId: number;
      runId: number;
      message: unknown;
    }
  | {
      type: "finished";
      sessionId: string;
      targetNodeId: string;
      partitionId: number;
    }
  | {
      type: "cancelled";
      sessionId: string;
      targetNodeId: string;
      partitionId: number;
    }
  | {
      type: "error";
      sessionId: string;
      targetNodeId: string;
      partitionId: number;
      code: string;
      message: string;
    };

export function subscribeSessionEvents(
  sessionId: string,
  onEvent: (e: SessionEvent) => void,
  onConnectionChange?: (status: ConnectionStatus) => void,
): () => void {
  return subscribeEventSource({
    url: `/api/sessions/${sessionId}/events`,
    onMessage: (ev) => {
      try {
        onEvent(JSON.parse(ev.data) as SessionEvent);
      } catch (err) {
        console.error("malformed SSE payload", err, ev.data);
      }
    },
    onConnectionChange,
  });
}
