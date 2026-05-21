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

import {
  phaseLabel,
  type Chain,
  type SessionLayout,
} from "./layout";

const nodeTypes: NodeTypes = { eunomia: NodeCard };

type Props = {
  layout: SessionLayout;
  chain: Chain;
  partitions: Partition[];
  candidatePartitionId: number | null;
  onSelectCandidate: (next: string) => void;
  selectedNodeId: string | null;
  onNodeClick: NodeMouseHandler;
};

export function GraphPane({
  layout,
  chain,
  partitions,
  candidatePartitionId,
  onSelectCandidate,
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

  return (
    <div className="flex h-full flex-col">
      {partitions.length > 0 && (
        <div className="flex shrink-0 items-center gap-2 border-b px-3 py-2">
          <span className="text-xs text-muted-foreground">View</span>
          <Select
            value={
              candidatePartitionId === null
                ? "canonical"
                : String(candidatePartitionId)
            }
            onValueChange={onSelectCandidate}
          >
            <SelectTrigger className="h-8 w-auto min-w-[12rem]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="canonical">Canonical</SelectItem>
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
      )}
      <div className="flex-1 min-h-0">
        <ReactFlow
          nodes={nodes}
          edges={layout.edges}
          nodeTypes={nodeTypes}
          colorMode="dark"
          fitView
          nodesDraggable={false}
          proOptions={{ hideAttribution: true }}
          onNodeClick={onNodeClick}
          style={
            layout.kind === "candidate"
              ? { backgroundColor: "#1f2226" }
              : undefined
          }
        >
          <Background />
        </ReactFlow>
      </div>
    </div>
  );
}
