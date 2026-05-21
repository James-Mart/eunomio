import { useState } from "react";
import { CircleAlert, PauseCircle } from "lucide-react";
import { toast } from "sonner";

import {
  api,
  type Partition,
  type PartitionStrategy,
  type PlanEdge,
} from "@/lib/api";
import { formatError } from "@/lib/errors";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { ConstructPayload } from "@/components/SessionEventsProvider";
import { STRATEGY_OPTIONS } from "@/components/review/strategyOptions";

type Props = {
  partitionId: number;
  sessionId: string;
  targetNodeId: string;
  payload: ConstructPayload;
  constructRunId?: number;
  slicePlanEdge?: PlanEdge | null;
  showSlicePlan?: boolean;
  onAbandon: () => void;
};

type RerunMode = "constructor" | "planner";

export default function ConstructReview({
  partitionId,
  sessionId,
  targetNodeId,
  payload,
  constructRunId,
  slicePlanEdge,
  showSlicePlan,
  onAbandon,
}: Props) {
  const [feedback, setFeedback] = useState("");
  const [rerunMode, setRerunMode] = useState<RerunMode>("constructor");
  const [strategyOverride, setStrategyOverride] = useState<
    "auto" | PartitionStrategy
  >("auto");
  const [busy, setBusy] = useState(false);
  const [siblingPrompt, setSiblingPrompt] = useState<Partition[] | null>(null);

  const performAccept = async () => {
    setBusy(true);
    try {
      await api.acceptConstruct(partitionId);
    } catch (e) {
      toast.error(formatError(e, "Accept failed"));
    } finally {
      setBusy(false);
    }
  };

  const accept = async () => {
    setBusy(true);
    try {
      const all = await api
        .listPartitions(sessionId, targetNodeId)
        .catch(() => [] as Partition[]);
      const siblings = all.filter((p) => p.id !== partitionId);
      if (siblings.length === 0) {
        await api.acceptConstruct(partitionId);
        return;
      }
      setSiblingPrompt(siblings);
    } catch (e) {
      toast.error(formatError(e, "Accept failed"));
    } finally {
      setBusy(false);
    }
  };

  const confirmAcceptDiscardSiblings = async () => {
    setSiblingPrompt(null);
    await performAccept();
  };

  const rerunConstructor = async () => {
    setBusy(true);
    setRerunMode("constructor");
    try {
      await api.startRun(partitionId, {
        kind: "construct",
        parentRunId: constructRunId,
        userFeedback: feedback.trim() || undefined,
      });
      setFeedback("");
    } catch (e) {
      toast.error(formatError(e, "Re-run failed"));
    } finally {
      setBusy(false);
    }
  };

  const rerunPlanner = async () => {
    setBusy(true);
    setRerunMode("planner");
    try {
      await api.startRun(partitionId, {
        kind: "plan",
        parentRunId: constructRunId,
        userFeedback: feedback.trim() || undefined,
        strategyOverride:
          strategyOverride === "auto" ? undefined : strategyOverride,
      });
      setFeedback("");
    } catch (e) {
      toast.error(formatError(e, "Re-run failed"));
    } finally {
      setBusy(false);
    }
  };

  const blocked = payload.outcome === "blocked";

  return (
    <div className="space-y-3">
      {showSlicePlan && slicePlanEdge && (
        <section className="space-y-1.5">
          <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Slice plan
          </div>
          <div className="rounded-md border bg-muted/30 px-3 py-2">
            <div className="text-sm font-medium">{slicePlanEdge.title}</div>
            <div className="text-xs text-muted-foreground">
              {slicePlanEdge.description}
            </div>
          </div>
        </section>
      )}
      {blocked ? (
        <Alert variant="destructive">
          <CircleAlert className="h-4 w-4" />
          <AlertTitle>Constructor blocked</AlertTitle>
          <AlertDescription>{payload.reason}</AlertDescription>
        </Alert>
      ) : (
        <Alert>
          <PauseCircle className="h-4 w-4" />
          <AlertTitle>Candidate ready for review</AlertTitle>
          <AlertDescription>
            Inspect the candidate via the candidate view in the graph pane, then
            Accept here, re-run the Constructor with feedback, re-run the
            Planner for a different slice, or Abandon.
          </AlertDescription>
        </Alert>
      )}

      <div className="space-y-3 rounded-md border bg-muted/30 p-3">
        <div className="space-y-1.5">
          <Label htmlFor="construct-feedback">Feedback for re-run (optional)</Label>
          <Textarea
            id="construct-feedback"
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
            placeholder="What did the constructor or planner get wrong?"
            rows={3}
          />
        </div>
        {rerunMode === "planner" && (
          <div className="space-y-1.5">
            <Label htmlFor="construct-strategy-override">Strategy override</Label>
            <Select
              value={strategyOverride}
              onValueChange={(v) =>
                setStrategyOverride(v as "auto" | PartitionStrategy)
              }
            >
              <SelectTrigger id="construct-strategy-override">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {STRATEGY_OPTIONS.map((o) => (
                  <SelectItem key={o.value} value={o.value}>
                    {o.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        )}
      </div>

      <div className="flex flex-wrap gap-2">
        {!blocked && (
          <Button onClick={accept} disabled={busy}>
            Accept candidate
          </Button>
        )}
        <Button
          variant="secondary"
          onClick={rerunConstructor}
          onMouseEnter={() => setRerunMode("constructor")}
          onFocus={() => setRerunMode("constructor")}
          disabled={busy}
        >
          Re-run Constructor
        </Button>
        <Button
          variant="secondary"
          onClick={rerunPlanner}
          onMouseEnter={() => setRerunMode("planner")}
          onFocus={() => setRerunMode("planner")}
          disabled={busy}
        >
          Re-run Planner
        </Button>
        <Button
          variant="outline"
          className="border-destructive/60 bg-destructive/10 text-destructive hover:bg-destructive hover:text-destructive-foreground"
          onClick={onAbandon}
        >
          Abandon
        </Button>
      </div>

      <Dialog
        open={siblingPrompt !== null}
        onOpenChange={(open) => {
          if (!open) setSiblingPrompt(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Discard other pending refinements?</DialogTitle>
            <DialogDescription>
              Accepting this candidate will discard{" "}
              {siblingPrompt?.length ?? 0} other pending refinement
              {(siblingPrompt?.length ?? 0) === 1 ? "" : "s"} on this Node.
            </DialogDescription>
          </DialogHeader>
          <ul className="space-y-1 text-sm">
            {siblingPrompt?.map((p) => (
              <li key={p.id}>
                — Partition {p.id} ({p.strategy ?? "pending"}, {p.phase}{" "}
                {p.phaseState})
              </li>
            ))}
          </ul>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setSiblingPrompt(null)}
              disabled={busy}
            >
              Cancel
            </Button>
            <Button onClick={confirmAcceptDiscardSiblings} disabled={busy}>
              Accept and discard others
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
