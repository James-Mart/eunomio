import { useEffect, useRef, useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { Star } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { api, type GraphNode } from "@/lib/api";
import { cn, shortSha } from "@/lib/utils";
import BranchDialog from "@/components/BranchDialog";

export type NodeCardData = {
  node: GraphNode;
  sessionId?: string;
  onChange?: () => void;
};

const RENAME_DEBOUNCE_MS = 400;

export default function NodeCard({ data }: NodeProps) {
  const cardData = data as NodeCardData;
  const { node, sessionId, onChange } = cardData;
  const [title, setTitle] = useState(node.title);
  const [editing, setEditing] = useState(false);
  const [favorite, setFavorite] = useState(node.isFavorite);
  const [branchOpen, setBranchOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<number | null>(null);

  useEffect(() => {
    setTitle(node.title);
  }, [node.title]);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  const commit = (next: string) => {
    if (!sessionId) return;
    if (next.trim() === "" || next === node.title) return;
    api
      .renameNode(sessionId, node.nodeId, next)
      .then(() => onChange?.())
      .catch((e) => toast.error(e instanceof Error ? e.message : "Rename failed"));
  };

  const onTitleChange = (next: string) => {
    setTitle(next);
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => commit(next), RENAME_DEBOUNCE_MS);
  };

  const flushRename = () => {
    if (debounceRef.current) {
      window.clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
    commit(title);
    setEditing(false);
  };

  return (
    <>
      <Handle type="target" position={Position.Left} />
      <Card className="w-[280px] shadow-md">
        <CardContent className="p-4 space-y-3">
          <div className="flex items-center justify-between gap-2">
            {editing ? (
              <Input
                ref={inputRef}
                value={title}
                onChange={(e) => onTitleChange(e.target.value)}
                onBlur={flushRename}
                onKeyDown={(e) => {
                  if (e.key === "Enter") flushRename();
                  if (e.key === "Escape") {
                    setTitle(node.title);
                    setEditing(false);
                  }
                }}
                className="h-7 px-2 text-base"
              />
            ) : (
              <button
                type="button"
                onClick={() => setEditing(true)}
                className="text-base font-semibold truncate text-left flex-1 hover:underline"
              >
                {node.title}
              </button>
            )}
            <button
              type="button"
              onClick={() => setFavorite((v) => !v)}
              aria-label={favorite ? "Unfavorite" : "Favorite"}
              className="text-muted-foreground hover:text-yellow-500"
            >
              <Star
                className={cn("h-4 w-4", favorite && "fill-yellow-500 text-yellow-500")}
              />
            </button>
          </div>
          <dl className="grid grid-cols-[auto,1fr] gap-x-2 gap-y-0.5 text-xs text-muted-foreground font-mono">
            <dt>commit</dt>
            <dd>{shortSha(node.commitSha)}</dd>
            <dt>tree</dt>
            <dd>{shortSha(node.treeSha)}</dd>
          </dl>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="w-full"
            onClick={() => setBranchOpen(true)}
            disabled={!sessionId}
          >
            Branch from here…
          </Button>
        </CardContent>
      </Card>
      <Handle type="source" position={Position.Right} />
      {sessionId && (
        <BranchDialog
          open={branchOpen}
          onOpenChange={setBranchOpen}
          sessionId={sessionId}
          nodeId={node.nodeId}
          defaultName={`from-${node.title.toLowerCase().replace(/[^a-z0-9-]+/g, "-")}`}
        />
      )}
    </>
  );
}
