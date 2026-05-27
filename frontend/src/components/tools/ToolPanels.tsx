/* SPDX-License-Identifier: Apache-2.0 */

import type { Partition, PartitionStrategy, ReorderAudit } from "@/lib/api";
import InfoTab from "@/components/InfoTab";
import PartitionTab from "@/components/PartitionTab";

export type PendingPartitionOption = {
  partition: Partition;
  label: string;
};

export type ToolsContext = {
  sessionId: string;
  nodeId: string | null;
  nodeTitle: string | null;
  nodeDescription: string | null;
  nodeStrategy: PartitionStrategy | null;
  reorderAudit: ReorderAudit | null;
  activePartition: Partition | null;
  isCandidateSliceSelected: boolean;
  pendingPartitionOptions: PendingPartitionOption[];
  onSelectPartition: (p: Partition) => void;
  onPartitionStarted: (p: Partition) => void;
  onPartitionEnded: () => void;
};

export function showNodeTools(ctx: ToolsContext): boolean {
  return ctx.activePartition === null && ctx.nodeId !== null;
}

export function isToolsEmpty(ctx: ToolsContext): boolean {
  return ctx.activePartition === null && ctx.nodeId === null;
}

export function PartitionToolPanel(ctx: ToolsContext) {
  return (
    <PartitionTab
      key={ctx.activePartition?.id ?? "none"}
      sessionId={ctx.sessionId}
      targetNodeId={ctx.nodeId}
      activePartition={ctx.activePartition}
      isCandidateSliceSelected={ctx.isCandidateSliceSelected}
      pendingPartitionOptions={ctx.pendingPartitionOptions}
      onSelectPartition={ctx.onSelectPartition}
      onPartitionStarted={ctx.onPartitionStarted}
      onPartitionEnded={ctx.onPartitionEnded}
    />
  );
}

export function InfoToolPanel(ctx: ToolsContext) {
  return (
    <InfoTab
      key={ctx.nodeId ?? "none"}
      nodeId={ctx.nodeId!}
      nodeTitle={ctx.nodeTitle!}
      nodeDescription={ctx.nodeDescription ?? ""}
      nodeStrategy={ctx.nodeStrategy}
      reorderAudit={ctx.reorderAudit}
    />
  );
}

export function ToolsEmpty({ className }: { className?: string }) {
  return (
    <div
      className={
        className ??
        "flex h-full items-center justify-center bg-background p-6 text-sm text-muted-foreground"
      }
    >
      Select a node or partition to view tools.
    </div>
  );
}
