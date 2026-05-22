import { useEffect, useMemo } from "react";
import {
  Background,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
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
  candidateLayoutFingerprint,
  comparePartitionsForView,
  partitionSiblingNumbers,
  partitionViewLabel,
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

function CandidateFitView({ fingerprint }: { fingerprint: string }) {
  const { fitView } = useReactFlow();
  useEffect(() => {
    void fitView({ padding: 0.2 });
  }, [fingerprint, fitView]);
  return null;
}

function GraphFlow({
  layout,
  selectedNodeId,
  onNodeClick,
}: {
  layout: SessionLayout;
  selectedNodeId: string | null;
  onNodeClick: NodeMouseHandler;
}) {
  const nodes = useMemo<Node<NodeCardData>[]>(
    () =>
      layout.nodes.map((n) =>
        n.id === selectedNodeId ? { ...n, selected: true } : n,
      ),
    [layout, selectedNodeId],
  );

  const fitFingerprint =
    layout.kind === "candidate" ? candidateLayoutFingerprint(layout) : null;

  return (
    <ReactFlow
      nodes={nodes}
      edges={layout.edges}
      nodeTypes={nodeTypes}
      colorMode="dark"
      fitView={fitFingerprint === null}
      nodesDraggable={false}
      proOptions={{ hideAttribution: true }}
      onNodeClick={onNodeClick}
    >
      <Background />
      {fitFingerprint !== null ? (
        <CandidateFitView fingerprint={fitFingerprint} />
      ) : null}
    </ReactFlow>
  );
}

export function GraphPane({
  layout,
  chain,
  partitions,
  view,
  onSelectView,
  selectedNodeId,
  onNodeClick,
}: Props) {
  const viewSelectValue =
    view.kind === "candidate" ? String(view.partitionId) : view.kind;
  const siblingNumbers = useMemo(
    () => partitionSiblingNumbers(partitions),
    [partitions],
  );
  const sortedPartitions = useMemo(
    () =>
      [...partitions].sort((a, b) =>
        comparePartitionsForView(a, b, chain, siblingNumbers),
      ),
    [partitions, chain, siblingNumbers],
  );

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
            {sortedPartitions.map((p) => (
              <SelectItem key={p.id} value={String(p.id)}>
                {partitionViewLabel(p, chain, siblingNumbers.get(p.id) ?? 1)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div
        className={cn(
          "flex-1 min-h-0",
          layout.kind === "candidate" &&
            "rounded-md border-2 border-attention/60",
        )}
      >
        <ReactFlowProvider>
          <GraphFlow
            layout={layout}
            selectedNodeId={selectedNodeId}
            onNodeClick={onNodeClick}
          />
        </ReactFlowProvider>
      </div>
    </div>
  );
}
