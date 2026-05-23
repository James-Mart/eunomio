/* SPDX-License-Identifier: Apache-2.0 */

import type { Partition, PartitionStrategy } from "@/lib/api";
import BranchTab from "@/components/BranchTab";
import InfoTab from "@/components/InfoTab";
import PartitionTab from "@/components/PartitionTab";

export type PendingPartitionOption = {
  partition: Partition;
  label: string;
};

export type ToolsContext = {
  sessionId: string;
  isLocal: boolean;
  nodeId: string | null;
  nodeTitle: string | null;
  nodeDescription: string | null;
  nodeStrategy: PartitionStrategy | null;
  activePartition: Partition | null;
  isCandidateSliceSelected: boolean;
  pendingPartitionOptions: PendingPartitionOption[];
  onSelectPartition: (p: Partition) => void;
  onPartitionStarted: (p: Partition) => void;
  onPartitionEnded: () => void;
  onChange?: () => void;
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
      sessionId={ctx.sessionId}
      nodeId={ctx.nodeId!}
      nodeTitle={ctx.nodeTitle!}
      nodeDescription={ctx.nodeDescription ?? ""}
      nodeStrategy={ctx.nodeStrategy}
      onChange={ctx.onChange}
    />
  );
}

export function BranchToolPanel(ctx: ToolsContext) {
  return (
    <BranchTab
      key={ctx.nodeId ?? "none"}
      sessionId={ctx.sessionId}
      nodeId={ctx.nodeId!}
      nodeTitle={ctx.nodeTitle!}
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
