import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { api } from "@/lib/api";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type Props = {
  sessionId: string;
  nodeId: string;
  nodeTitle: string;
  nodeDescription: string;
  onChange?: () => void;
};

const RENAME_DEBOUNCE_MS = 400;

export default function InfoTab({ sessionId, nodeId, nodeTitle, nodeDescription, onChange }: Props) {
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
        <Label htmlFor="node-title">Title</Label>
        <Input
          id="node-title"
          value={title}
          onChange={(e) => onTitleChange(e.target.value)}
        />
      </div>
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
