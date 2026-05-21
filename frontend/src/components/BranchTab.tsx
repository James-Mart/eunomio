import { useState } from "react";
import { toast } from "sonner";

import { ApiError, api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type Props = {
  sessionId: string;
  nodeId: string;
  nodeTitle: string;
};

export default function BranchTab({ sessionId, nodeId, nodeTitle }: Props) {
  const [name, setName] = useState(() => defaultBranchName(nodeTitle));
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    setSubmitting(true);
    try {
      const result = await api.branchFromNode(sessionId, nodeId, trimmed);
      toast.success(`Created branch ${result.branchName}`);
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) {
        toast.error("Branch already exists — choose another name.");
      } else {
        toast.error(e instanceof Error ? e.message : "Branch creation failed");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <Label htmlFor="branch-name">Branch name</Label>
        <Input
          id="branch-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="my-feature"
        />
      </div>
      <Button onClick={submit} disabled={submitting || !name.trim()}>
        {submitting ? "Creating…" : "Create branch"}
      </Button>
    </div>
  );
}

function defaultBranchName(title: string): string {
  return `from-${title.toLowerCase().replace(/[^a-z0-9-]+/g, "-")}`;
}
