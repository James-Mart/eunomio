import { useState } from "react";
import { CircleAlert, PauseCircle } from "lucide-react";
import { toast } from "sonner";

import { api, type Plan, type PartitionStrategy } from "@/lib/api";
import { formatError } from "@/lib/errors";
import { useApplyPartitionSnapshot } from "@/components/SessionEventsProvider";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import CollapsibleItem from "@/components/review/CollapsibleItem";
import { STRATEGY_OPTIONS } from "@/components/review/strategyOptions";

type Props = {
  partitionId: number;
  plan: Plan;
  planRunId: number;
  onAbandon: () => void;
};

export default function PlanReview({
  partitionId,
  plan,
  planRunId,
  onAbandon,
}: Props) {
  const [feedback, setFeedback] = useState("");
  const [strategyOverride, setStrategyOverride] = useState<
    "auto" | PartitionStrategy
  >("auto");
  const [busy, setBusy] = useState(false);
  const applyPartitionSnapshot = useApplyPartitionSnapshot();

  const accept = async () => {
    setBusy(true);
    try {
      const updated = await api.acceptPlan(partitionId, planRunId);
      applyPartitionSnapshot(updated);
    } catch (e) {
      toast.error(formatError(e, "Accept failed"));
    } finally {
      setBusy(false);
    }
  };

  const rerun = async () => {
    setBusy(true);
    try {
      await api.startRun(partitionId, {
        kind: "plan",
        parentRunId: planRunId,
        userFeedback: feedback.trim() || undefined,
        strategyOverride: strategyOverride === "auto" ? undefined : strategyOverride,
      });
      setFeedback("");
    } catch (e) {
      toast.error(formatError(e, "Re-run failed"));
    } finally {
      setBusy(false);
    }
  };

  const isIndivisible = plan.outcome === "indivisible";

  return (
    <div className="space-y-3">
      {isIndivisible ? (
        <Alert>
          <CircleAlert className="h-4 w-4" />
          <AlertTitle>Planner declined to split</AlertTitle>
          <AlertDescription>{plan.rationale}</AlertDescription>
        </Alert>
      ) : (
        <Alert>
          <PauseCircle className="h-4 w-4" />
          <AlertTitle>Plan ready for review</AlertTitle>
          <AlertDescription>
            Accept the plan to start constructing, or re-run with feedback.
          </AlertDescription>
        </Alert>
      )}

      {!isIndivisible && (
        <section className="space-y-2">
          <div className="text-sm">
            <span className="font-medium">Strategy:</span>{" "}
            <span className="capitalize">{plan.strategy}</span> —{" "}
            <span className="text-muted-foreground">{plan.strategyRationale}</span>
          </div>
          <div className="space-y-2">
            {plan.edges.map((edge, idx) => (
              <CollapsibleItem
                key={edge.id}
                leadingLabel={
                  idx === 0 ? "Slice (this Partition's new Node)" : "Leftover"
                }
                title={edge.title}
                description={edge.description}
              />
            ))}
          </div>
        </section>
      )}

      <div className="space-y-3 rounded-md border bg-muted/30 p-3">
        <div className="space-y-1.5">
          <Label htmlFor="plan-feedback">Feedback for re-run (optional)</Label>
          <Textarea
            id="plan-feedback"
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
            placeholder={
              isIndivisible
                ? "Tell the Planner to try harder to find a split, or accept the verdict and Abandon."
                : "What did the planner get wrong?"
            }
            rows={3}
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="plan-strategy-override">Strategy override</Label>
          <Select
            value={strategyOverride}
            onValueChange={(v) =>
              setStrategyOverride(v as "auto" | PartitionStrategy)
            }
          >
            <SelectTrigger id="plan-strategy-override">
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
      </div>

      <div className="flex flex-wrap gap-2">
        {!isIndivisible && (
          <Button onClick={accept} disabled={busy}>
            Accept plan
          </Button>
        )}
        <Button variant="secondary" onClick={rerun} disabled={busy}>
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
    </div>
  );
}
