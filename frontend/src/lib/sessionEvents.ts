/* SPDX-License-Identifier: Apache-2.0 */

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
      partitionId: string;
    }
  | {
      type: "phase";
      sessionId: string;
      targetNodeId: string;
      partitionId: string;
      name: PhaseName;
      state: PhaseState;
      payload?: unknown;
    }
  | {
      type: "transcriptDelta";
      sessionId: string;
      targetNodeId: string;
      partitionId: string;
      runId: string;
      text: string;
    }
  | {
      type: "finished";
      sessionId: string;
      targetNodeId: string;
      partitionId: string;
    }
  | {
      type: "shavingReady";
      sessionId: string;
      targetNodeId: string;
    }
  | {
      type: "sessionPartitionComplete";
      sessionId: string;
      completedAt: number;
    }
  | {
      type: "cancelled";
      sessionId: string;
      targetNodeId: string;
      partitionId: string;
    }
  | {
      type: "error";
      sessionId: string;
      targetNodeId: string;
      partitionId: string;
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
