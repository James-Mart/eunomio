import { Handle, Position, type NodeProps } from "@xyflow/react";
import { PauseCircle } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { type GraphNode } from "@/lib/api";
import { cn } from "@/lib/utils";

export type NodeCardData = {
  node: GraphNode;
  positionLabel: string;
  sessionId?: string;
  candidateBadge?: boolean;
  onChange?: () => void;
};

export default function NodeCard({ data, selected }: NodeProps) {
  const { positionLabel, candidateBadge } = data as NodeCardData;

  return (
    <>
      <Handle type="target" position={Position.Bottom} />
      <Card
        className={cn(
          "w-[180px] shadow-md",
          selected && "ring-2 ring-primary ring-offset-2 ring-offset-background",
          candidateBadge && "ring-2 ring-amber-500/80",
        )}
      >
        <CardContent className="flex items-center justify-between gap-2 p-3">
          <span className="flex-1 truncate text-sm font-semibold">
            {positionLabel}
          </span>
          {candidateBadge && (
            <PauseCircle
              className="h-4 w-4 text-amber-500"
              aria-label="Candidate ready for review"
            />
          )}
        </CardContent>
      </Card>
      <Handle type="source" position={Position.Top} />
    </>
  );
}
