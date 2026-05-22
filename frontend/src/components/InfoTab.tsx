import { CopyIcon } from "@primer/octicons-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { api, type PartitionStrategy } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type Props = {
  sessionId: string;
  nodeId: string;
  nodeTitle: string;
  nodeDescription: string;
  nodeStrategy: PartitionStrategy | null;
  onChange?: () => void;
};

const RENAME_DEBOUNCE_MS = 400;

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success("Copied");
  } catch {
    toast.error("Copy failed");
  }
}

function CopyTextButton({
  text,
  ariaLabel,
}: {
  text: string;
  ariaLabel: string;
}) {
  return (
    <Button
      size="icon"
      variant="ghost"
      className="h-8 w-8 shrink-0"
      aria-label={ariaLabel}
      onClick={() => void copyText(text)}
    >
      <CopyIcon className="h-3.5 w-3.5" />
    </Button>
  );
}

export default function InfoTab({
  sessionId,
  nodeId,
  nodeTitle,
  nodeDescription,
  nodeStrategy,
  onChange,
}: Props) {
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
    <div className="space-y-3">
      <div className="space-y-1.5">
        <Label htmlFor="node-id">Node ID</Label>
        <div className="flex items-center gap-1.5">
          <Input id="node-id" readOnly value={nodeId} className="font-mono text-xs" />
          <CopyTextButton text={nodeId} ariaLabel="Copy node ID" />
        </div>
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="node-title">Title</Label>
        <Input
          id="node-title"
          value={title}
          onChange={(e) => onTitleChange(e.target.value)}
        />
      </div>
      {nodeStrategy && (
        <div className="space-y-1.5">
          <Label>Strategy</Label>
          <p className="text-sm text-muted-foreground capitalize">
            {nodeStrategy}
          </p>
        </div>
      )}
      {nodeDescription.trim() !== "" && (
        <div className="space-y-1.5">
          <Label>Description</Label>
          <p className="text-sm text-muted-foreground whitespace-pre-wrap">
            {nodeDescription}
          </p>
        </div>
      )}
    </div>
  );
}
