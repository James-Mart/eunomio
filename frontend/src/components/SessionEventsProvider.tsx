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

import { api, type Partition, type Run } from "@/lib/api";
import {
  subscribeSessionEvents,
  type ConnectionStatus,
  type PhaseState,
  type SessionEvent,
} from "@/lib/sessionEvents";

export type ConstructPayload =
  | { outcome: "ok"; candidateTreeSha: string; candidateCommitSha: string }
  | { outcome: "blocked"; reason: string };

export type PlanPayload =
  | {
      outcome: "split";
      strategy: "synthetic" | "vertical" | "horizontal";
      strategyRationale: string;
      edges: { id: string; title: string; description: string }[];
    }
  | { outcome: "indivisible"; rationale: string };

export type Lifecycle = {
  partitionId: number;
  targetNodeId: string;
  survey: PhaseState | "pending";
  plan: PhaseState | "pending";
  construct: PhaseState | "pending";
  surveyPayload?: unknown;
  planPayload?: PlanPayload;
  constructPayload?: ConstructPayload;
  recentMessages: unknown[];
  lastError?: { code: string; message: string };
  finishedAt?: number;
  cancelledAt?: number;
  acceptedAt?: number;
};

const MAX_MESSAGES = 50;

type ConstructListener = () => void;

type Store = {
  lifecycles: Map<number, Lifecycle>;
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
  private constructListeners: Set<ConstructListener> = new Set();

  subscribe = (cb: () => void): (() => void) => {
    this.listeners.add(cb);
    return () => {
      this.listeners.delete(cb);
    };
  };

  subscribeConstruct = (cb: ConstructListener): (() => void) => {
    this.constructListeners.add(cb);
    return () => {
      this.constructListeners.delete(cb);
    };
  };

  getSnapshot = (): Store => this.state;

  applyEvent(event: SessionEvent) {
    const { lifecycles } = this.state;
    const next = new Map(lifecycles);
    const cur = next.get(event.partitionId);

    let constructChanged = false;

    switch (event.type) {
      case "started":
        if (cur) {
          constructChanged = true;
          break;
        }
        next.set(event.partitionId, {
          partitionId: event.partitionId,
          targetNodeId: event.targetNodeId,
          survey: "pending",
          plan: "pending",
          construct: "pending",
          recentMessages: [],
        });
        constructChanged = true;
        break;
      case "phase": {
        const base = cur ?? blankLifecycle(event.partitionId, event.targetNodeId);
        const updated: Lifecycle = {
          ...base,
          partitionId: event.partitionId,
          targetNodeId: event.targetNodeId,
          [event.name]: event.state,
        };
        if (event.name === "survey" && event.payload !== undefined)
          updated.surveyPayload = event.payload;
        if (event.name === "plan" && event.payload !== undefined)
          updated.planPayload = event.payload as PlanPayload;
        if (event.name === "construct" && event.payload !== undefined) {
          updated.constructPayload = event.payload as ConstructPayload;
        }
        next.set(event.partitionId, updated);
        constructChanged = true;
        break;
      }
      case "sdkMessage": {
        if (!cur) break;
        const recentMessages = [...cur.recentMessages, event.message].slice(
          -MAX_MESSAGES,
        );
        next.set(event.partitionId, { ...cur, recentMessages });
        break;
      }
      case "finished": {
        if (!cur) break;
        next.set(event.partitionId, {
          ...cur,
          finishedAt: Date.now(),
          acceptedAt: Date.now(),
        });
        constructChanged = true;
        break;
      }
      case "cancelled": {
        if (!cur) break;
        next.set(event.partitionId, { ...cur, cancelledAt: Date.now() });
        constructChanged = true;
        break;
      }
      case "error": {
        if (!cur) break;
        next.set(event.partitionId, {
          ...cur,
          lastError: { code: event.code, message: event.message },
        });
        constructChanged = true;
        break;
      }
    }
    this.state = { ...this.state, lifecycles: next };
    this.emit();
    if (constructChanged) for (const l of this.constructListeners) l();
  }

  setConnection(connection: ConnectionStatus) {
    if (this.state.connection === connection) return;
    this.state = { ...this.state, connection };
    this.emit();
  }

  hydrate(lifecycles: Lifecycle[]) {
    const { lifecycles: cur } = this.state;
    let inserted = 0;
    const next = new Map(cur);
    for (const l of lifecycles) {
      if (next.has(l.partitionId)) continue;
      next.set(l.partitionId, l);
      inserted++;
    }
    if (inserted === 0) return;
    this.state = { ...this.state, lifecycles: next };
    this.emit();
    for (const l of this.constructListeners) l();
  }

  resetLifecycle(partitionId: number) {
    if (!this.state.lifecycles.has(partitionId)) return;
    const next = new Map(this.state.lifecycles);
    next.delete(partitionId);
    this.state = { ...this.state, lifecycles: next };
    this.emit();
    for (const l of this.constructListeners) l();
  }

  applyPartitionSnapshot(p: Partition) {
    const cur = this.state.lifecycles.get(p.id);
    const next = new Map(this.state.lifecycles);
    if (!cur) {
      next.set(p.id, buildLifecycleFromSnapshot(p, []));
    } else {
      next.set(p.id, { ...cur, [p.phase]: p.phaseState });
    }
    this.state = { ...this.state, lifecycles: next };
    this.emit();
    for (const l of this.constructListeners) l();
  }

  private emit() {
    for (const l of this.listeners) l();
  }
}

function blankLifecycle(partitionId: number, targetNodeId: string): Lifecycle {
  return {
    partitionId,
    targetNodeId,
    survey: "pending",
    plan: "pending",
    construct: "pending",
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
    let cancelled = false;

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

    void (async () => {
      try {
        const partitions = await api.listPartitions(sessionId);
        if (cancelled || partitions.length === 0) return;
        const runsByPartition = await Promise.all(
          partitions.map((p) =>
            api.listRuns(p.id).catch(() => [] as Run[]),
          ),
        );
        if (cancelled) return;
        const hydrated = partitions.map((p, i) =>
          buildLifecycleFromSnapshot(p, runsByPartition[i]),
        );
        store.hydrate(hydrated);
      } catch {
        // Hydration is best-effort; SSE still drives live updates.
      }
    })();

    return () => {
      cancelled = true;
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

function buildLifecycleFromSnapshot(p: Partition, runs: Run[]): Lifecycle {
  const lifecycle = blankLifecycle(p.id, p.targetNodeId);
  lifecycle[p.phase] = p.phaseState;

  if (p.phase === "construct" && p.phaseState === "awaiting_review") {
    const constructRun = runs.find(
      (r) => r.kind === "construct" && r.status === "finished",
    );
    const result = constructRun?.result as
      | { outcome?: string; reason?: string }
      | undefined;
    if (result?.outcome === "blocked" && typeof result.reason === "string") {
      lifecycle.constructPayload = {
        outcome: "blocked",
        reason: result.reason,
      };
    } else if (p.candidateSliceTreeSha && p.candidateSliceCommitSha) {
      lifecycle.constructPayload = {
        outcome: "ok",
        candidateTreeSha: p.candidateSliceTreeSha,
        candidateCommitSha: p.candidateSliceCommitSha,
      };
    }
  }

  const latestPhaseRun = runs
    .filter((r) => r.kind === p.phase)
    .sort((a, b) => b.startedAt - a.startedAt)[0];
  if (latestPhaseRun?.status === "error") {
    const message = latestPhaseRun.errorMessage ?? "Run failed";
    const code = message === "process_restart" ? "process_restart" : "run_error";
    lifecycle.lastError = { code, message };
    lifecycle[p.phase] = "error";
  }

  return lifecycle;
}

function useStore(): SessionStore {
  const ctx = useContext(SessionEventsContext);
  if (!ctx)
    throw new Error("useSessionEvents must be used inside SessionEventsProvider");
  return ctx.store;
}

export function usePartitionLifecycle(
  partitionId: number | null | undefined,
): Lifecycle | undefined {
  const store = useStore();
  const subscribe = store.subscribe;
  const getSnapshot = store.getSnapshot;
  return useSyncExternalStore(
    subscribe,
    useCallback(
      () =>
        partitionId == null
          ? undefined
          : getSnapshot().lifecycles.get(partitionId),
      [getSnapshot, partitionId],
    ),
  );
}

const EMPTY_LIFECYCLES: Lifecycle[] = [];

export function usePartitionLifecyclesByTarget(
  targetNodeId: string,
): Lifecycle[] {
  const store = useStore();
  const subscribe = store.subscribe;
  const getSnapshot = store.getSnapshot;
  const cacheRef = useRef<{
    map: Map<number, Lifecycle> | null;
    targetNodeId: string;
    value: Lifecycle[];
  }>({ map: null, targetNodeId: "", value: EMPTY_LIFECYCLES });
  return useSyncExternalStore(
    subscribe,
    useCallback(() => {
      const map = getSnapshot().lifecycles;
      const stale =
        cacheRef.current.map !== map ||
        cacheRef.current.targetNodeId !== targetNodeId;
      if (stale) {
        const filtered: Lifecycle[] = [];
        for (const l of map.values()) {
          if (l.targetNodeId === targetNodeId) filtered.push(l);
        }
        filtered.sort((a, b) => a.partitionId - b.partitionId);
        cacheRef.current = {
          map,
          targetNodeId,
          value: filtered.length === 0 ? EMPTY_LIFECYCLES : filtered,
        };
      }
      return cacheRef.current.value;
    }, [getSnapshot, targetNodeId]),
  );
}

export function useResetLifecycle(): (partitionId: number) => void {
  const store = useStore();
  return useCallback((id: number) => store.resetLifecycle(id), [store]);
}

export function useHydratePartition(): (p: Partition) => void {
  const store = useStore();
  return useCallback(
    (p: Partition) => store.hydrate([buildLifecycleFromSnapshot(p, [])]),
    [store],
  );
}

export function useApplyPartitionSnapshot(): (p: Partition) => void {
  const store = useStore();
  return useCallback((p: Partition) => store.applyPartitionSnapshot(p), [store]);
}

export function useConstructSubscription(cb: () => void): void {
  const store = useStore();
  useEffect(() => store.subscribeConstruct(cb), [store, cb]);
}
