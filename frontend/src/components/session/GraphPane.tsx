import { useMemo } from "react";
import {
  Background,
  ReactFlow,
  type Node,
  type NodeMouseHandler,
  type NodeTypes,
} from "@xyflow/react";

import type { Partition } from "@/lib/api";
import NodeCard, { type NodeCardData } from "@/components/NodeCard";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

import { cn } from "@/lib/utils";

import {
  phaseLabel,
  type Chain,
  type SessionLayout,
  type View,
} from "./layout";

const nodeTypes: NodeTypes = { eunomia: NodeCard };

type Props = {
  layout: SessionLayout;
  chain: Chain;
  partitions: Partition[];
  view: View;
  onSelectView: (next: string) => void;
  selectedNodeId: string | null;
  onNodeClick: NodeMouseHandler;
};

export function GraphPane({
  layout,
  chain,
  partitions,
  view,
  onSelectView,
  selectedNodeId,
  onNodeClick,
}: Props) {
  const nodes = useMemo<Node<NodeCardData>[]>(
    () =>
      layout.nodes.map((n) =>
        n.id === selectedNodeId ? { ...n, selected: true } : n,
      ),
    [layout, selectedNodeId],
  );

  const viewSelectValue =
    view.kind === "candidate" ? String(view.partitionId) : view.kind;

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b px-3 py-2">
        <span className="text-xs text-muted-foreground">View</span>
        <Select value={viewSelectValue} onValueChange={onSelectView}>
          <SelectTrigger className="h-8 w-auto min-w-[12rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="canonical">Canonical</SelectItem>
            <SelectItem value="original">Original</SelectItem>
            {partitions.map((p) => {
              const targetPos =
                chain.positionByNodeId.get(p.targetNodeId) ?? "?";
              const strategy = p.strategy ?? "synthetic";
              return (
                <SelectItem key={p.id} value={String(p.id)}>
                  Partition on Node {targetPos} ({strategy}, {phaseLabel(p)})
                </SelectItem>
              );
            })}
          </SelectContent>
        </Select>
      </div>
      <div
        className={cn(
          "flex-1 min-h-0",
          layout.kind === "candidate" &&
            "rounded-md border-2 border-amber-500/60",
        )}
      >
        <ReactFlow
          nodes={nodes}
          edges={layout.edges}
          nodeTypes={nodeTypes}
          colorMode="dark"
          fitView
          nodesDraggable={false}
          proOptions={{ hideAttribution: true }}
          onNodeClick={onNodeClick}
        >
          <Background />
        </ReactFlow>
      </div>
    </div>
  );
}
