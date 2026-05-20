import { useCallback, useEffect, useState } from "react";
import { CircleAlert, Loader2 } from "lucide-react";
import { toast } from "sonner";

import {
  api,
  ApiError,
  type ChangeSurvey,
  type Partition,
  type Plan,
  type Run,
} from "@/lib/api";
import {
  useResetLifecycle,
  usePartitionLifecycle,
  usePartitionLifecyclesByTarget,
  type Lifecycle,
} from "@/components/SessionEventsProvider";
import {
  LifecycleStepper,
  type LifecycleStates,
} from "@/components/PartitionLifecycle";
import {
  ConstructReview,
  PlanReview,
  SurveyReview,
} from "@/components/review";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

type Props = {
  sessionId: string;
  targetNodeId: string | null;
  activePartition: Partition | null;
  isCandidateSliceSelected: boolean;
  onPartitionStarted: (p: Partition) => void;
  onPartitionEnded: () => void;
};

export default function PartitionTab({
  sessionId,
  targetNodeId,
  activePartition,
  isCandidateSliceSelected,
  onPartitionStarted,
  onPartitionEnded,
}: Props) {
  const activeLifecycle = usePartitionLifecycle(activePartition?.id ?? null);
  const pendingForTarget = usePartitionLifecyclesByTarget(
    targetNodeId ?? "",
  ).filter((l) => !l.finishedAt && !l.cancelledAt);

  const [partition, setPartition] = useState<Partition | null>(null);
  const [runs, setRuns] = useState<Run[]>([]);
  const resetLifecycle = useResetLifecycle();

  const refresh = useCallback(async () => {
    if (!activeLifecycle) {
      setPartition(null);
      setRuns([]);
      return;
    }
    try {
      const [p, r] = await Promise.all([
        api.getPartition(sessionId, activeLifecycle.partitionId).catch(() => null),
        api.listRuns(sessionId, activeLifecycle.partitionId).catch(() => [] as Run[]),
      ]);
      if (p) setPartition(p);
      setRuns(r);
    } catch (e) {
      console.error(e);
    }
  }, [sessionId, activeLifecycle?.partitionId]);

  useEffect(() => {
    void refresh();
  }, [
    refresh,
    activeLifecycle?.survey,
    activeLifecycle?.plan,
    activeLifecycle?.construct,
  ]);

  const states: LifecycleStates = {
    survey: activeLifecycle?.survey ?? "pending",
    plan: activeLifecycle?.plan ?? "pending",
    construct: activeLifecycle?.construct ?? "pending",
  };

  const phase = derivePhase(activeLifecycle);

  const abandon = useCallback(async () => {
    if (!activeLifecycle) return;
    const partitionId = activeLifecycle.partitionId;
    try {
      await api.abandonPartition(sessionId, partitionId);
      resetLifecycle(partitionId);
      onPartitionEnded();
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        resetLifecycle(partitionId);
        onPartitionEnded();
        return;
      }
      toast.error(e instanceof Error ? e.message : "Failed to abandon");
    }
  }, [activeLifecycle, sessionId, resetLifecycle, onPartitionEnded]);

  return (
    <div className="space-y-4">
      <LifecycleStepper states={states} />
      {phase === "idle" || phase === "cancelled" || phase === "finished" ? (
        targetNodeId ? (
          <BeginView
            sessionId={sessionId}
            targetNodeId={targetNodeId}
            phase={phase}
            disabled={pendingForTarget.length > 0}
            onPartitionStarted={onPartitionStarted}
          />
        ) : null
      ) : phase === "error" ? (
        <ErrorView
          message={activeLifecycle?.lastError?.message ?? "Unknown error"}
          onAbandon={abandon}
        />
      ) : phase === "awaitingSurvey" ? (
        partition?.changeSurvey ? (
          <SurveyReview
            sessionId={sessionId}
            partitionId={partition.id}
            survey={partition.changeSurvey ?? readSurveyFromRuns(runs)!}
            surveyRunId={pickRunId(runs, "survey")!}
            onAbandon={abandon}
          />
        ) : (
          readSurveyFromRuns(runs) ? (
            <SurveyReview
              sessionId={sessionId}
              partitionId={activeLifecycle!.partitionId}
              survey={readSurveyFromRuns(runs)!}
              surveyRunId={pickRunId(runs, "survey")!}
              onAbandon={abandon}
            />
          ) : (
            <RunningView lifecycle={activeLifecycle!} onAbandon={abandon} />
          )
        )
      ) : phase === "awaitingPlan" ? (
        readPlanFromRuns(runs) ? (
          <PlanReview
            sessionId={sessionId}
            partitionId={activeLifecycle!.partitionId}
            plan={readPlanFromRuns(runs)!}
            planRunId={pickRunId(runs, "plan")!}
            onAbandon={abandon}
          />
        ) : (
          <RunningView lifecycle={activeLifecycle!} onAbandon={abandon} />
        )
      ) : phase === "awaitingConstruct" && activeLifecycle?.constructPayload ? (
        <ConstructReview
          sessionId={sessionId}
          partitionId={activeLifecycle.partitionId}
          payload={activeLifecycle.constructPayload}
          constructRunId={pickRunId(runs, "construct") ?? undefined}
          slicePlanEdge={partition?.plan?.edges[0] ?? null}
          showSlicePlan={isCandidateSliceSelected}
          onAbandon={abandon}
        />
      ) : (
        <RunningView lifecycle={activeLifecycle!} onAbandon={abandon} />
      )}
    </div>
  );
}

type Phase =
  | "idle"
  | "running"
  | "awaitingSurvey"
  | "awaitingPlan"
  | "awaitingConstruct"
  | "finished"
  | "cancelled"
  | "error";

function derivePhase(lifecycle: Lifecycle | undefined): Phase {
  if (!lifecycle) return "idle";
  if (lifecycle.lastError) return "error";
  if (lifecycle.cancelledAt) return "cancelled";
  if (lifecycle.finishedAt) return "finished";
  if (lifecycle.construct === "awaiting_review") return "awaitingConstruct";
  if (lifecycle.plan === "awaiting_review") return "awaitingPlan";
  if (lifecycle.survey === "awaiting_review") return "awaitingSurvey";
  return "running";
}

function BeginView({
  sessionId,
  targetNodeId,
  phase,
  disabled,
  onPartitionStarted,
}: {
  sessionId: string;
  targetNodeId: string;
  phase: Phase;
  disabled: boolean;
  onPartitionStarted: (p: Partition) => void;
}) {
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    setBusy(true);
    try {
      const created = await api.beginPartition(sessionId, targetNodeId);
      onPartitionStarted(created);
    } catch (e) {
      if (e instanceof ApiError && e.code === "partition_in_flight") {
        toast.error(
          "Another partition is already in flight for this session. Abandon it first.",
        );
      } else {
        toast.error(e instanceof Error ? e.message : "Failed to begin partition");
      }
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-3">
      {phase === "finished" && (
        <p className="text-sm text-muted-foreground">
          Partition accepted. Begin another?
        </p>
      )}
      {phase === "cancelled" && (
        <p className="text-sm text-muted-foreground">
          Partition was abandoned. Begin another?
        </p>
      )}
      {disabled && (
        <p className="text-sm text-muted-foreground">
          A partition is pending on this node — select it from the View dropdown
          above to review.
        </p>
      )}
      <Button onClick={submit} disabled={busy || disabled}>
        {busy ? "Starting…" : "Begin partition"}
      </Button>
    </div>
  );
}

function RunningView({
  lifecycle,
  onAbandon,
}: {
  lifecycle: Lifecycle;
  onAbandon: () => void;
}) {
  const label = runningLabel(lifecycle);
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 rounded-md border bg-muted/50 p-3 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        <span>{label}</span>
      </div>
      <Button
        variant="outline"
        className="border-destructive/60 bg-destructive/10 text-destructive hover:bg-destructive hover:text-destructive-foreground"
        onClick={onAbandon}
      >
        Abandon
      </Button>
    </div>
  );
}

function runningLabel(lifecycle: Lifecycle): string {
  if (lifecycle.construct === "running") return "Constructing candidate commit…";
  if (lifecycle.plan === "running") return "Planning partition…";
  if (lifecycle.survey === "running") return "Surveying changes…";
  return "Agent working…";
}

function ErrorView({
  message,
  onAbandon,
}: {
  message: string;
  onAbandon: () => void;
}) {
  return (
    <div className="space-y-3">
      <Alert variant="destructive">
        <CircleAlert className="h-4 w-4" />
        <AlertTitle>Partition error</AlertTitle>
        <AlertDescription>{message}</AlertDescription>
      </Alert>
      <Button
        variant="outline"
        className="border-destructive/60 bg-destructive/10 text-destructive hover:bg-destructive hover:text-destructive-foreground"
        onClick={onAbandon}
      >
        Abandon
      </Button>
    </div>
  );
}

function pickRunId(runs: Run[], kind: Run["kind"]): number | null {
  const r = runs.find((r) => r.kind === kind && r.status === "finished");
  return r?.id ?? null;
}

function readSurveyFromRuns(runs: Run[]): ChangeSurvey | null {
  const r = runs.find((r) => r.kind === "survey" && r.status === "finished");
  if (!r || !r.result) return null;
  return r.result as ChangeSurvey;
}

function readPlanFromRuns(runs: Run[]): Plan | null {
  const r = runs.find((r) => r.kind === "plan" && r.status === "finished");
  if (!r || !r.result) return null;
  return r.result as Plan;
}
