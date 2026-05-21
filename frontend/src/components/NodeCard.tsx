import { memo } from "react";
import { Handle, Position, type NodeProps, type Node } from "@xyflow/react";

import { Card, CardContent } from "@/components/ui/card";
import {
  LifecycleStepper,
  lifecycleStatesFromPhase,
} from "@/components/PartitionLifecycle";
import { type PhaseName, type PhaseState } from "@/lib/api";
import { cn } from "@/lib/utils";

export type NodeCardData = {
  positionLabel: string;
  phaseStatus?: { phase: PhaseName; phaseState: PhaseState } | null;
};

type NodeCardProps = NodeProps<Node<NodeCardData>>;

function NodeCard({ data, selected }: NodeCardProps) {
  const { positionLabel, phaseStatus } = data;
  const needsAttention =
    !!phaseStatus &&
    (phaseStatus.phaseState === "awaiting_review" ||
      phaseStatus.phaseState === "error");

  return (
    <>
      <Handle type="target" position={Position.Bottom} />
      <Card
        className={cn(
          "w-[180px] shadow-md",
          selected && "ring-2 ring-primary ring-offset-2 ring-offset-background",
          needsAttention && "ring-2 ring-red-500/80",
        )}
      >
        <CardContent className="flex items-center justify-between gap-2 p-3">
          <span className="flex-1 truncate text-sm font-semibold">
            {positionLabel}
          </span>
          {phaseStatus && (
            <LifecycleStepper
              states={lifecycleStatesFromPhase(
                phaseStatus.phase,
                phaseStatus.phaseState,
              )}
              compact
            />
          )}
        </CardContent>
      </Card>
      <Handle type="source" position={Position.Top} />
    </>
  );
}

export default memo(NodeCard);
