import { Handle, Position, type NodeProps } from "@xyflow/react";
import { BellRing, Loader2 } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { type GraphNode } from "@/lib/api";
import { cn } from "@/lib/utils";

export type BadgeState = "none" | "running" | "awaiting";

export type NodeCardData = {
  node: GraphNode;
  positionLabel: string;
  sessionId?: string;
  badgeState?: BadgeState;
  onChange?: () => void;
};

export default function NodeCard({ data, selected }: NodeProps) {
  const { positionLabel, badgeState = "none" } = data as NodeCardData;

  return (
    <>
      <Handle type="target" position={Position.Bottom} />
      <Card
        className={cn(
          "w-[180px] shadow-md",
          selected && "ring-2 ring-primary ring-offset-2 ring-offset-background",
          badgeState === "awaiting" && "ring-2 ring-red-500/80",
          badgeState === "running" && "ring-2 ring-amber-500/80",
        )}
      >
        <CardContent className="flex items-center justify-between gap-2 p-3">
          <span className="flex-1 truncate text-sm font-semibold">
            {positionLabel}
          </span>
          {badgeState === "awaiting" && (
            <BellRing
              className="h-4 w-4 text-red-500"
              aria-label="Partition awaiting input"
            />
          )}
          {badgeState === "running" && (
            <Loader2
              className="h-4 w-4 animate-spin text-amber-500"
              aria-label="Partition running"
            />
          )}
        </CardContent>
      </Card>
      <Handle type="source" position={Position.Top} />
    </>
  );
}
