import { useEffect, useState } from "react";
import { toast } from "sonner";

import { ApiError, api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sessionId: string;
  nodeId: string;
  defaultName: string;
};

export default function BranchDialog({
  open,
  onOpenChange,
  sessionId,
  nodeId,
  defaultName,
}: Props) {
  const [name, setName] = useState(defaultName);
  const [force, setForce] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) {
      setName(defaultName);
      setForce(false);
    }
  }, [open, defaultName]);

  const submit = async () => {
    if (!name.trim()) return;
    setSubmitting(true);
    try {
      const result = await api.branchFromNode(sessionId, nodeId, name.trim(), force);
      toast.success(`Created branch ${result.branchName}`);
      onOpenChange(false);
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) {
        toast.error("Branch already exists. Tick force to overwrite.");
        setForce(true);
      } else {
        toast.error(e instanceof Error ? e.message : "Branch creation failed");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Branch from this node</DialogTitle>
          <DialogDescription>
            Walks back through parents to base, replays each commit using its title, then creates a
            real branch in your repo.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="branch-name">Branch name</Label>
            <Input
              id="branch-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="my-feature"
              autoFocus
            />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <Checkbox checked={force} onChange={(e) => setForce(e.target.checked)} />
            <span>Force overwrite if a branch with this name already exists</span>
          </label>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={submitting}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={submitting || !name.trim()}>
            {submitting ? "Creating…" : "Create branch"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
