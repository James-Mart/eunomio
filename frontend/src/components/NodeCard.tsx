/* SPDX-License-Identifier: Apache-2.0 */

import { memo } from "react";
import {
  AlertIcon,
  CheckCircleIcon,
  EyeIcon,
  PauseIcon,
} from "@primer/octicons-react";
import { Handle, Position, type NodeProps, type Node } from "@xyflow/react";

import { CANDIDATE_SLICE_ID } from "@/components/session/layout";
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

function isTerminalLabel(label: string): boolean {
  return label === "base" || label === "final";
}

function NodeCard({ id, data, selected, onReviewedChange }: NodeCardProps) {
  const { positionLabel, partitionGlance, reviewed } = data;
  const isCanonicalShell = onReviewedChange !== undefined;
  const showReviewedControl = reviewed !== undefined;
  const count = partitionGlance?.count ?? 0;
  const isCandidateSlice = id === CANDIDATE_SLICE_ID;
  const terminal = isTerminalLabel(positionLabel);

  return (
    <>
      <Handle type="target" position={Position.Bottom} />
      <div
        className={cn(
          "rounded-md border border-border bg-card px-2.5 py-2 transition-colors duration-150",
          isCanonicalShell ? "w-[140px]" : "w-[100px]",
          isCanonicalShell && "flex flex-col",
          isCanonicalShell &&
            !showReviewedControl &&
            "min-h-[3.75rem] justify-center",
          selected && "bg-secondary",
          isCandidateSlice && "border-attention/40",
        )}
      >
        <div
          className={cn(
            "truncate text-center font-mono tabular-nums",
            terminal
              ? "text-xs uppercase tracking-wide text-muted-foreground"
              : "text-sm font-medium text-foreground",
          )}
        >
          {positionLabel}
        </div>
        {isCanonicalShell && showReviewedControl ? (
          <div
            className="mt-1.5 flex h-6 items-center justify-center gap-2"
            onClick={stopNodeSelection}
            onPointerDown={stopNodeSelection}
          >
            <button
              type="button"
              className={cn(
                "flex h-6 w-6 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:text-foreground",
                reviewed && "text-success",
              )}
              aria-label="Mark node as viewed"
              aria-pressed={reviewed}
              onClick={() => onReviewedChange?.(!reviewed)}
            >
              {reviewed ? (
                <CheckCircleIcon className="h-4 w-4" />
              ) : (
                <EyeIcon className="h-4 w-4" />
              )}
            </button>
            {count > 0 && partitionGlance ? (
              <PartitionStatusIcon
                count={count}
                status={partitionGlance.status}
              />
            ) : null}
          </div>
        ) : null}
      </div>
      <Handle type="source" position={Position.Top} />
    </>
  );
}

function partitionAriaLabel(
  count: number,
  status: "running" | "blocked" | "error",
): string {
  if (status === "error") {
    return `${count} partitions, one or more failed`;
  }
  if (status === "blocked") {
    return `${count} partitions, one or more awaiting review`;
  }
  return `${count} partitions in progress`;
}

function PartitionStatusIcon({
  count,
  status,
}: {
  count: number;
  status: "running" | "blocked" | "error";
}) {
  return (
    <span
      className="flex shrink-0 items-center gap-0.5"
      aria-label={partitionAriaLabel(count, status)}
    >
      {status === "error" ? (
        <AlertIcon className="h-4 w-4 text-danger" aria-hidden />
      ) : status === "blocked" ? (
        <PauseIcon className="h-4 w-4 text-attention" aria-hidden />
      ) : (
        <span
          className="h-1.5 w-1.5 animate-pulse rounded-full bg-attention"
          aria-hidden
        />
      )}
      {count > 1 ? (
        <span className="text-[10px] tabular-nums text-muted-foreground">
          {count}
        </span>
      ) : null}
    </span>
  );
}

export default memo(NodeCard);
