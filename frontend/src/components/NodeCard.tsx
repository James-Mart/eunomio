/* SPDX-License-Identifier: Apache-2.0 */

import { memo, useId } from "react";
import { Handle, Position, type NodeProps, type Node } from "@xyflow/react";

import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { cn } from "@/lib/utils";

export type NodePartitionGlance = {
  count: number;
  status: "running" | "blocked" | "error";
};

export type NodeCardData = {
  positionLabel: string;
  partitionGlance?: NodePartitionGlance | null;
  reviewed?: boolean;
};

type NodeCardProps = NodeProps<Node<NodeCardData>> & {
  onReviewedChange?: (reviewed: boolean) => void;
};

function stopNodeSelection(event: React.SyntheticEvent) {
  event.stopPropagation();
}

function NodeCard({ data, selected, onReviewedChange }: NodeCardProps) {
  const { positionLabel, partitionGlance, reviewed } = data;
  const checkboxId = useId();
  const showReviewedControl = reviewed !== undefined;
  const count = partitionGlance?.count ?? 0;
  const status = partitionGlance?.status;
  const needsAttention = status === "blocked" || status === "error";

  return (
    <>
      <Handle type="target" position={Position.Bottom} />
      <Card
        className={cn(
          showReviewedControl ? "w-[220px]" : "w-[180px]",
          "shadow-md",
          status === "error" && "ring-2 ring-danger",
          status === "blocked" && "ring-2 ring-danger/80",
          selected && "ring-offset-2 ring-offset-background",
          selected && !needsAttention && "ring-2 ring-primary",
        )}
      >
        <CardContent className="flex items-center gap-2 p-3">
          {showReviewedControl ? (
            <div
              className="flex shrink-0 items-center gap-1.5"
              onClick={stopNodeSelection}
              onPointerDown={stopNodeSelection}
            >
              <Checkbox
                id={checkboxId}
                checked={reviewed}
                aria-label="Mark node as viewed"
                onChange={(event) =>
                  onReviewedChange?.(event.currentTarget.checked)
                }
              />
              <label
                htmlFor={checkboxId}
                className="cursor-pointer text-xs text-muted-foreground"
              >
                Viewed
              </label>
            </div>
          ) : null}
          <span className="min-w-0 flex-1 truncate text-sm font-semibold">
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
  status: "running" | "blocked" | "error";
}) {
  return (
    <span
      className={cn(
        "flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold",
        status === "error" && "bg-danger/20 text-danger ring-2 ring-danger",
        status === "blocked" && "text-danger ring-2 ring-danger/80",
        status === "running" && "animate-pulse bg-attention/15 text-attention",
      )}
      aria-label={
        status === "error"
          ? `${count} partitions, one or more failed`
          : status === "blocked"
            ? `${count} partitions, one or more awaiting review`
            : `${count} partitions in progress`
      }
    >
      {count}
    </span>
  );
}

export default memo(NodeCard);
