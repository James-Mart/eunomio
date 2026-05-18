import { useEffect, useRef, useState } from "react";
import { Construction } from "lucide-react";
import { toast } from "sonner";

import { ApiError, api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

type Props = {
  sessionId: string;
  nodeId: string;
  nodeTitle: string;
  onChange?: () => void;
};

const RENAME_DEBOUNCE_MS = 400;

export default function ToolsPane({ sessionId, nodeId, nodeTitle, onChange }: Props) {
  return (
    <Tabs defaultValue="partition" className="flex h-full flex-col gap-3 p-4">
      <TabsList className="self-start">
        <TabsTrigger value="partition">Partition</TabsTrigger>
        <TabsTrigger value="info">Info</TabsTrigger>
        <TabsTrigger value="branch">Branch</TabsTrigger>
      </TabsList>
      <TabsContent value="partition" className="mt-0 flex-1 min-h-0">
        <PartitionTab />
      </TabsContent>
      <TabsContent value="info" className="mt-0 flex-1 min-h-0">
        <InfoTab
          sessionId={sessionId}
          nodeId={nodeId}
          nodeTitle={nodeTitle}
          onChange={onChange}
        />
      </TabsContent>
      <TabsContent value="branch" className="mt-0 flex-1 min-h-0">
        <BranchTab sessionId={sessionId} nodeId={nodeId} nodeTitle={nodeTitle} />
      </TabsContent>
    </Tabs>
  );
}

function InfoTab({
  sessionId,
  nodeId,
  nodeTitle,
  onChange,
}: {
  sessionId: string;
  nodeId: string;
  nodeTitle: string;
  onChange?: () => void;
}) {
  const [title, setTitle] = useState(nodeTitle);
  const debounceRef = useRef<number | null>(null);
  const baselineRef = useRef(nodeTitle);

  useEffect(() => {
    setTitle(nodeTitle);
    baselineRef.current = nodeTitle;
    return () => {
      if (debounceRef.current) {
        window.clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
    };
  }, [nodeId]); // eslint-disable-line react-hooks/exhaustive-deps

  const commit = (next: string) => {
    const trimmed = next.trim();
    if (trimmed === "" || trimmed === baselineRef.current) return;
    api
      .renameNode(sessionId, nodeId, trimmed)
      .then(() => {
        baselineRef.current = trimmed;
        onChange?.();
      })
      .catch((e) => toast.error(e instanceof Error ? e.message : "Rename failed"));
  };

  const onTitleChange = (next: string) => {
    setTitle(next);
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => commit(next), RENAME_DEBOUNCE_MS);
  };

  return (
    <div className="space-y-1.5">
      <Label htmlFor="node-title">Title</Label>
      <Input
        id="node-title"
        value={title}
        onChange={(e) => onTitleChange(e.target.value)}
      />
    </div>
  );
}

function PartitionTab() {
  return (
    <div className="flex items-center gap-2 text-sm text-muted-foreground">
      <Construction className="h-4 w-4" aria-hidden="true" />
      <span>Partition is not implemented yet.</span>
    </div>
  );
}

function BranchTab({
  sessionId,
  nodeId,
  nodeTitle,
}: {
  sessionId: string;
  nodeId: string;
  nodeTitle: string;
}) {
  const [name, setName] = useState(() => defaultBranchName(nodeTitle));
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    setName(defaultBranchName(nodeTitle));
  }, [nodeId]); // eslint-disable-line react-hooks/exhaustive-deps

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
