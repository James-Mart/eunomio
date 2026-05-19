import { useState } from "react";
import { PauseCircle } from "lucide-react";
import { toast } from "sonner";

import { api, type Plan, type PartitionStrategy } from "@/lib/api";
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

type Props = {
  sessionId: string;
  partitionId: number;
  plan: Plan;
  planRunId: number;
  onAbandon: () => void;
};

const STRATEGY_OPTIONS: { value: "auto" | PartitionStrategy; label: string }[] = [
  { value: "auto", label: "Auto (let planner choose)" },
  { value: "semantic", label: "Semantic" },
  { value: "vertical", label: "Vertical" },
  { value: "horizontal", label: "Horizontal" },
];

export default function PlanReview({
  sessionId,
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

  const accept = async () => {
    setBusy(true);
    try {
      await api.acceptPlan(sessionId, partitionId, planRunId);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Accept failed");
    } finally {
      setBusy(false);
    }
  };

  const rerun = async () => {
    setBusy(true);
    try {
      await api.startRun(sessionId, partitionId, {
        kind: "plan",
        parentRunId: planRunId,
        userFeedback: feedback.trim() || undefined,
        strategyOverride: strategyOverride === "auto" ? undefined : strategyOverride,
      });
      setFeedback("");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Re-run failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <Alert>
        <PauseCircle className="h-4 w-4" />
        <AlertTitle>Plan ready for review</AlertTitle>
        <AlertDescription>
          Accept the plan to start constructing, or re-run with feedback.
        </AlertDescription>
      </Alert>

      <section className="space-y-2">
        <div className="text-sm">
          <span className="font-medium">Strategy:</span>{" "}
          <span className="capitalize">{plan.strategy}</span> —{" "}
          <span className="text-muted-foreground">{plan.strategyRationale}</span>
        </div>
        <div className="space-y-2">
          {plan.edges.map((edge, idx) => (
            <div
              key={edge.id}
              className="rounded-md border bg-muted/30 px-3 py-2"
            >
              <div className="text-xs text-muted-foreground">
                {idx === 0 ? "Slice (this Partition's new Node)" : "Leftover"}
              </div>
              <div className="text-sm font-medium">{edge.title}</div>
              <div className="text-xs text-muted-foreground">
                {edge.description}
              </div>
            </div>
          ))}
        </div>
      </section>

      <div className="space-y-3 rounded-md border bg-muted/30 p-3">
        <div className="space-y-1.5">
          <Label htmlFor="plan-feedback">Feedback for re-run (optional)</Label>
          <Textarea
            id="plan-feedback"
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
            placeholder="What did the planner get wrong?"
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
        <Button onClick={accept} disabled={busy}>
          Accept plan
        </Button>
        <Button variant="secondary" onClick={rerun} disabled={busy}>
          Re-run with feedback
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
