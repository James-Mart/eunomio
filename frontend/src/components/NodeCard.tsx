import { memo } from "react";
import { Handle, Position, type NodeProps, type Node } from "@xyflow/react";

import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export type NodePartitionGlance = {
  count: number;
  status: "running" | "blocked";
};

export type NodeCardData = {
  positionLabel: string;
  partitionGlance?: NodePartitionGlance | null;
};

type NodeCardProps = NodeProps<Node<NodeCardData>>;

function NodeCard({ data, selected }: NodeCardProps) {
  const { positionLabel, partitionGlance } = data;
  const count = partitionGlance?.count ?? 0;
  const blocked = partitionGlance?.status === "blocked";

  return (
    <>
      <Handle type="target" position={Position.Bottom} />
      <Card
        className={cn(
          "w-[180px] shadow-md",
          blocked && "ring-2 ring-danger/80",
          selected && "ring-offset-2 ring-offset-background",
          selected && !blocked && "ring-2 ring-primary",
        )}
      >
        <CardContent className="flex items-center justify-between gap-2 p-3">
          <span className="flex-1 truncate text-sm font-semibold">
            {positionLabel}
          </span>
          {count > 0 && partitionGlance ? (
            <PartitionCountChip
              count={count}
              status={partitionGlance.status}
            />
          ) : null}
        </CardContent>
      </Card>
      <Handle type="source" position={Position.Top} />
    </>
  );
}

function PartitionCountChip({
  count,
  status,
}: {
  count: number;
  status: "running" | "blocked";
}) {
  const blocked = status === "blocked";
  return (
    <span
      className={cn(
        "flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold",
        blocked
          ? "text-danger ring-2 ring-danger/80"
          : "animate-pulse bg-attention/15 text-attention",
      )}
      aria-label={
        blocked
          ? `${count} partitions, one or more awaiting review`
          : `${count} partitions in progress`
      }
    >
      {count}
    </span>
  );
}

export default memo(NodeCard);
