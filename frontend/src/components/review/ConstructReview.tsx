import { useState } from "react";
import { CircleAlert, PauseCircle } from "lucide-react";
import { toast } from "sonner";

import { api } from "@/lib/api";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { ConstructPayload } from "@/components/SessionEventsProvider";

type Props = {
  sessionId: string;
  partitionId: number;
  payload: ConstructPayload;
  constructRunId?: number;
  onAbandon: () => void;
};

export default function ConstructReview({
  sessionId,
  partitionId,
  payload,
  constructRunId,
  onAbandon,
}: Props) {
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);

  const accept = async () => {
    setBusy(true);
    try {
      await api.acceptConstruct(sessionId, partitionId);
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
        kind: "construct",
        parentRunId: constructRunId,
        userFeedback: feedback.trim() || undefined,
      });
      setFeedback("");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Re-run failed");
    } finally {
      setBusy(false);
    }
  };

  const blocked = payload.outcome === "blocked";

  return (
    <div className="space-y-3">
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
            Accept here, re-run the Constructor with feedback, or Abandon.
          </AlertDescription>
        </Alert>
      )}

      <div className="space-y-1.5">
        <Label htmlFor="construct-feedback">Feedback for re-run (optional)</Label>
        <Textarea
          id="construct-feedback"
          value={feedback}
          onChange={(e) => setFeedback(e.target.value)}
          placeholder="What did the constructor get wrong?"
          rows={3}
        />
      </div>

      <div className="flex flex-wrap gap-2">
        {!blocked && (
          <Button onClick={accept} disabled={busy}>
            Accept candidate
          </Button>
        )}
        <Button variant="secondary" onClick={rerun} disabled={busy}>
          Re-run Constructor
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
