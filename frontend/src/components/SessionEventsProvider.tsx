import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { toast } from "sonner";

import type { PartitionStrategy } from "@/lib/api";
import {
  subscribeSessionEvents,
  type ConnectionStatus,
  type PhaseState,
  type SessionEvent,
} from "@/lib/sessionEvents";

export type Lifecycle = {
  strategy: PartitionStrategy;
  userConcern: string | null;
  survey: PhaseState;
  plan: PhaseState;
  construct: PhaseState;
  surveyPayload?: unknown;
  planPayload?: unknown;
  constructProgress: { itemId: string; status: string }[];
  recentMessages: unknown[];
  lastError?: { code: string; message: string };
  finishedAt?: number;
  cancelledAt?: number;
};

const MAX_MESSAGES = 50;

type Store = {
  lifecycles: Map<string, Lifecycle>;
  connection: ConnectionStatus;
};

const CONNECTION_LOST_TOAST_ID = "session-events-connection";

type Listeners = Set<() => void>;

class SessionStore {
  private state: Store = {
    lifecycles: new Map(),
    connection: "connecting",
  };
  private listeners: Listeners = new Set();

  subscribe = (cb: () => void): (() => void) => {
    this.listeners.add(cb);
    return () => {
      this.listeners.delete(cb);
    };
  };

  getSnapshot = (): Store => this.state;

  applyEvent(event: SessionEvent) {
    const { lifecycles } = this.state;
    const next = new Map(lifecycles);
    const cur = next.get(event.targetNodeId);

    switch (event.type) {
      case "started":
        next.set(event.targetNodeId, {
          strategy: event.strategy,
          userConcern: event.userConcern,
          survey: "pending",
          plan: "pending",
          construct: "pending",
          constructProgress: [],
          recentMessages: [],
        });
        break;
      case "phase": {
        const base = cur ?? blankLifecycle();
        const updated: Lifecycle = {
          ...base,
          [event.name]: event.state,
        };
        if (event.name === "survey" && event.payload !== undefined)
          updated.surveyPayload = event.payload;
        if (event.name === "plan" && event.payload !== undefined)
          updated.planPayload = event.payload;
        next.set(event.targetNodeId, updated);
        break;
      }
      case "sdkMessage": {
        if (!cur) break;
        const recentMessages = [...cur.recentMessages, event.message].slice(
          -MAX_MESSAGES,
        );
        next.set(event.targetNodeId, { ...cur, recentMessages });
        break;
      }
      case "loopProgress": {
        if (!cur) break;
        next.set(event.targetNodeId, {
          ...cur,
          constructProgress: [
            ...cur.constructProgress,
            { itemId: event.itemId, status: event.status },
          ],
        });
        break;
      }
      case "finished": {
        if (!cur) break;
        next.set(event.targetNodeId, { ...cur, finishedAt: Date.now() });
        break;
      }
      case "cancelled": {
        if (!cur) break;
        next.set(event.targetNodeId, { ...cur, cancelledAt: Date.now() });
        break;
      }
      case "error": {
        if (!cur) break;
        next.set(event.targetNodeId, {
          ...cur,
          lastError: { code: event.code, message: event.message },
        });
        break;
      }
    }
    this.state = { ...this.state, lifecycles: next };
    this.emit();
  }

  setConnection(connection: ConnectionStatus) {
    if (this.state.connection === connection) return;
    this.state = { ...this.state, connection };
    this.emit();
  }

  resetLifecycle(targetNodeId: string) {
    if (!this.state.lifecycles.has(targetNodeId)) return;
    const next = new Map(this.state.lifecycles);
    next.delete(targetNodeId);
    this.state = { ...this.state, lifecycles: next };
    this.emit();
  }

  private emit() {
    for (const l of this.listeners) l();
  }
}

function blankLifecycle(): Lifecycle {
  return {
    strategy: "semantic",
    userConcern: null,
    survey: "pending",
    plan: "pending",
    construct: "pending",
    constructProgress: [],
    recentMessages: [],
  };
}

type ContextValue = {
  store: SessionStore;
};

const SessionEventsContext = createContext<ContextValue | null>(null);

export function SessionEventsProvider({
  sessionId,
  children,
}: {
  sessionId: string;
  children: ReactNode;
}) {
  const storeRef = useRef<SessionStore | null>(null);
  if (storeRef.current === null) storeRef.current = new SessionStore();
  const store = storeRef.current;

  useEffect(() => {
    const onConnection = (status: ConnectionStatus) => {
      store.setConnection(status);
      if (status === "closed") {
        toast.error("Lost connection to backend; reconnecting…", {
          id: CONNECTION_LOST_TOAST_ID,
          duration: Infinity,
        });
      } else if (status === "open") {
        toast.dismiss(CONNECTION_LOST_TOAST_ID);
      }
    };
    const unsub = subscribeSessionEvents(
      sessionId,
      (e) => store.applyEvent(e),
      onConnection,
    );
    return () => {
      unsub();
      toast.dismiss(CONNECTION_LOST_TOAST_ID);
    };
  }, [sessionId, store]);

  const value = useMemo(() => ({ store }), [store]);

  return (
    <SessionEventsContext.Provider value={value}>
      {children}
    </SessionEventsContext.Provider>
  );
}

function useStore(): SessionStore {
  const ctx = useContext(SessionEventsContext);
  if (!ctx)
    throw new Error("useSessionEvents must be used inside SessionEventsProvider");
  return ctx.store;
}

export function usePartitionLifecycle(targetNodeId: string): Lifecycle | undefined {
  const store = useStore();
  const subscribe = store.subscribe;
  const getSnapshot = store.getSnapshot;
  return useSyncExternalStore(
    subscribe,
    useCallback(
      () => getSnapshot().lifecycles.get(targetNodeId),
      [getSnapshot, targetNodeId],
    ),
  );
}

export function useSessionConnectionStatus(): ConnectionStatus {
  const store = useStore();
  return useSyncExternalStore(store.subscribe, () => store.getSnapshot().connection);
}

export function useResetLifecycle(): (targetNodeId: string) => void {
  const store = useStore();
  return useCallback((id: string) => store.resetLifecycle(id), [store]);
}
