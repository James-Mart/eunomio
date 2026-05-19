import { useMemo, useState } from "react";
import { CircleAlert, PauseCircle } from "lucide-react";
import { toast } from "sonner";

import { api, ApiError, type PartitionStrategy } from "@/lib/api";
import {
  useResetLifecycle,
  usePartitionLifecycle,
  type Lifecycle,
} from "@/components/SessionEventsProvider";
import {
  LifecycleStepper,
  type LifecycleStates,
} from "@/components/PartitionLifecycle";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Textarea } from "@/components/ui/textarea";

type Props = {
  sessionId: string;
  targetNodeId: string;
};

type DerivedPhase =
  | "idle"
  | "running"
  | "awaitingSurvey"
  | "awaitingPlan"
  | "finished"
  | "cancelled"
  | "error";

const STRATEGY_OPTIONS: { value: PartitionStrategy; label: string; description: string }[] = [
  {
    value: "semantic",
    label: "Semantic",
    description: "Slice this diff into N independent concerns.",
  },
  {
    value: "vertical",
    label: "Vertical",
    description: "A sequence of thin end-to-end tracer bullets.",
  },
  {
    value: "horizontal",
    label: "Horizontal",
    description: "Slice by architectural layer (native → service → UI).",
  },
];

export default function PartitionTab({ sessionId, targetNodeId }: Props) {
  const lifecycle = usePartitionLifecycle(targetNodeId);
  const derived = derivePhase(lifecycle);
  const states: LifecycleStates = {
    survey: lifecycle?.survey ?? "pending",
    plan: lifecycle?.plan ?? "pending",
    construct: lifecycle?.construct ?? "pending",
  };

  return (
    <div className="space-y-4">
      <LifecycleStepper states={states} />

      {derived === "idle" || derived === "cancelled" ? (
        <BeginForm sessionId={sessionId} targetNodeId={targetNodeId} />
      ) : (
        <ActivePartition
          sessionId={sessionId}
          targetNodeId={targetNodeId}
          lifecycle={lifecycle!}
          derived={derived}
        />
      )}
    </div>
  );
}

function derivePhase(lifecycle: Lifecycle | undefined): DerivedPhase {
  if (!lifecycle) return "idle";
  if (lifecycle.lastError) return "error";
  if (lifecycle.cancelledAt) return "cancelled";
  if (lifecycle.construct === "done") return "finished";
  if (lifecycle.survey === "awaiting_review") return "awaitingSurvey";
  if (lifecycle.plan === "awaiting_review") return "awaitingPlan";
  return "running";
}

function BeginForm({ sessionId, targetNodeId }: Props) {
  const [strategy, setStrategy] = useState<PartitionStrategy>("semantic");
  const [concern, setConcern] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const resetLifecycle = useResetLifecycle();

  const submit = async () => {
    setSubmitting(true);
    try {
      resetLifecycle(targetNodeId);
      await api.startMockPartition(sessionId, targetNodeId, {
        strategy,
        userConcern: concern.trim() || undefined,
      });
    } catch (e) {
      if (e instanceof ApiError && e.code === "partition_in_flight") {
        toast.error(
          "Another partition is already in flight for this session. Abandon it first.",
        );
      } else {
        toast.error(e instanceof Error ? e.message : "Failed to start partition");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label>Strategy</Label>
        <RadioGroup
          value={strategy}
          onValueChange={(v) => setStrategy(v as PartitionStrategy)}
          className="gap-3"
        >
          {STRATEGY_OPTIONS.map((opt) => (
            <div key={opt.value} className="flex items-start gap-3">
              <RadioGroupItem id={`strategy-${opt.value}`} value={opt.value} className="mt-0.5" />
              <div className="space-y-0.5">
                <Label htmlFor={`strategy-${opt.value}`} className="font-normal">
                  {opt.label}
                </Label>
                <p className="text-xs text-muted-foreground">{opt.description}</p>
              </div>
            </div>
          ))}
        </RadioGroup>
      </div>

      <div className="space-y-1.5">
        <Label htmlFor="user-concern">Concern (optional)</Label>
        <Textarea
          id="user-concern"
          value={concern}
          onChange={(e) => setConcern(e.target.value)}
          placeholder="What coupling do you want untangled?"
          rows={3}
        />
      </div>

      <Button onClick={submit} disabled={submitting}>
        {submitting ? "Starting…" : "Begin partition"}
      </Button>
    </div>
  );
}

function ActivePartition({
  sessionId,
  targetNodeId,
  lifecycle,
  derived,
}: {
  sessionId: string;
  targetNodeId: string;
  lifecycle: Lifecycle;
  derived: DerivedPhase;
}) {
  const resetLifecycle = useResetLifecycle();

  const abandon = async () => {
    try {
      await api.abandonMockPartition(sessionId, targetNodeId);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        resetLifecycle(targetNodeId);
        return;
      }
      toast.error(e instanceof Error ? e.message : "Failed to abandon");
    }
  };

  const beginAnother = () => resetLifecycle(targetNodeId);

  return (
    <div className="space-y-4">
      {derived === "running" && (
        <RunningView lifecycle={lifecycle} onAbandon={abandon} />
      )}

      {derived === "awaitingSurvey" && (
        <ReviewGate
          gate="survey"
          payload={lifecycle.surveyPayload}
          sessionId={sessionId}
          targetNodeId={targetNodeId}
          onAbandon={abandon}
        />
      )}

      {derived === "awaitingPlan" && (
        <ReviewGate
          gate="plan"
          payload={lifecycle.planPayload}
          sessionId={sessionId}
          targetNodeId={targetNodeId}
          onAbandon={abandon}
        />
      )}

      {derived === "finished" && <FinishedView onBeginAnother={beginAnother} />}

      {derived === "error" && (
        <ErrorView
          message={lifecycle.lastError?.message ?? "Unknown error"}
          onAbandon={abandon}
        />
      )}
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
  return (
    <div className="space-y-3">
      <EventTail lifecycle={lifecycle} />
      <Button variant="ghost" className="text-destructive" onClick={onAbandon}>
        Abandon
      </Button>
    </div>
  );
}

function ReviewGate({
  gate,
  payload,
  sessionId,
  targetNodeId,
  onAbandon,
}: {
  gate: "survey" | "plan";
  payload: unknown;
  sessionId: string;
  targetNodeId: string;
  onAbandon: () => void;
}) {
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const label = gate === "survey" ? "survey" : "plan";

  const accept = async () => {
    setBusy(true);
    try {
      await api.continueMockPartition(sessionId, targetNodeId);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to continue");
    } finally {
      setBusy(false);
    }
  };

  const rerun = async () => {
    setBusy(true);
    try {
      await api.rerunMockPartition(sessionId, targetNodeId, {
        userFeedback: feedback.trim() || undefined,
      });
      setFeedback("");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to re-run");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <Alert>
        <PauseCircle className="h-4 w-4" />
        <AlertTitle>Waiting on your review</AlertTitle>
        <AlertDescription>
          {`Accept the ${label} to ${gate === "survey" ? "start planning" : "start constructing"}, or re-run with feedback.`}
        </AlertDescription>
      </Alert>

      <PayloadPreview payload={payload} />

      <div className="space-y-1.5">
        <Label htmlFor={`${gate}-feedback`}>Feedback for re-run (optional)</Label>
        <Textarea
          id={`${gate}-feedback`}
          value={feedback}
          onChange={(e) => setFeedback(e.target.value)}
          placeholder={`What did the ${label} miss?`}
          rows={3}
        />
      </div>

      <div className="flex flex-wrap gap-2">
        <Button onClick={accept} disabled={busy}>
          {`Accept ${label}`}
        </Button>
        <Button variant="secondary" onClick={rerun} disabled={busy}>
          Re-run with feedback
        </Button>
        <Button variant="ghost" className="text-destructive" onClick={onAbandon}>
          Abandon
        </Button>
      </div>
    </div>
  );
}

function FinishedView({ onBeginAnother }: { onBeginAnother: () => void }) {
  return (
    <div className="space-y-3">
      <p className="text-sm text-muted-foreground">Partition complete (mock).</p>
      <Button onClick={onBeginAnother}>Begin another</Button>
    </div>
  );
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
      <Button variant="ghost" className="text-destructive" onClick={onAbandon}>
        Abandon
      </Button>
    </div>
  );
}

function EventTail({ lifecycle }: { lifecycle: Lifecycle }) {
  const text = useMemo(() => {
    const lines: string[] = [];
    for (const msg of lifecycle.recentMessages) {
      lines.push(typeof msg === "string" ? msg : JSON.stringify(msg));
    }
    for (const p of lifecycle.constructProgress) {
      lines.push(`${p.itemId}: ${p.status}`);
    }
    return lines.join("\n");
  }, [lifecycle.recentMessages, lifecycle.constructProgress]);

  if (!text) return null;
  return (
    <pre className="max-h-40 overflow-auto rounded-md border bg-muted/50 p-2 text-xs">
      {text}
    </pre>
  );
}

function PayloadPreview({ payload }: { payload: unknown }) {
  if (payload === undefined || payload === null) return null;
  return (
    <pre className="max-h-60 overflow-auto rounded-md border bg-muted/50 p-2 text-xs">
      {JSON.stringify(payload, null, 2)}
    </pre>
  );
}
