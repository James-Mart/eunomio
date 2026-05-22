import { useState } from "react";
import { StopIcon } from "@primer/octicons-react";
import { toast } from "sonner";

import { api, type ChangeSurvey } from "@/lib/api";
import { formatError } from "@/lib/errors";
import { useApplyPartitionSnapshot } from "@/components/SessionEventsProvider";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import CollapsibleItem from "@/components/review/CollapsibleItem";

type Props = {
  partitionId: number;
  survey: ChangeSurvey;
  surveyRunId: number;
  onAbandon: () => void;
};

export default function SurveyReview({
  partitionId,
  survey,
  surveyRunId,
  onAbandon,
}: Props) {
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const applyPartitionSnapshot = useApplyPartitionSnapshot();

  const accept = async () => {
    setBusy(true);
    try {
      const updated = await api.acceptSurvey(partitionId, surveyRunId);
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
        kind: "survey",
        parentRunId: surveyRunId,
        userFeedback: feedback.trim() || undefined,
      });
      setFeedback("");
    } catch (e) {
      toast.error(formatError(e, "Re-run failed"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <Alert>
        <StopIcon className="h-4 w-4" />
        <AlertTitle>Survey ready for review</AlertTitle>
        <AlertDescription>
          Accept the survey to start planning, or re-run with feedback.
        </AlertDescription>
      </Alert>

      <section className="space-y-2">
        <p className="text-sm">{survey.summary}</p>
        <div className="space-y-2">
          {survey.themes.map((theme) => (
            <CollapsibleItem
              key={theme.id}
              title={theme.title}
              description={theme.description}
            />
          ))}
        </div>
      </section>

      <div className="space-y-1.5">
        <Label htmlFor="survey-feedback">Feedback for re-run (optional)</Label>
        <Textarea
          id="survey-feedback"
          value={feedback}
          onChange={(e) => setFeedback(e.target.value)}
          placeholder="What did the survey miss?"
          rows={3}
        />
      </div>

      <div className="flex flex-wrap gap-2">
        <Button onClick={accept} disabled={busy}>
          Accept survey
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
