/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, useMemo, useRef } from "react";
import {
  Background,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Node,
  type NodeMouseHandler,
  type NodeTypes,
  type Viewport,
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

const nodeTypes: NodeTypes = { eunomio: NodeCard };

type Props = {
  layout: SessionLayout;
  chain: Chain;
  partitions: Partition[];
  view: View;
  onSelectView: (next: string) => void;
  selectedNodeId: string | null;
  onNodeClick: NodeMouseHandler;
};

const FIT_VIEW_OPTIONS = { padding: 0.2 } as const;

/**
 * - Fit when entering original or candidate layouts.
 * - On first canonical load, fit once.
 * - When returning to canonical, restore pan/zoom from before leaving.
 */
function GraphViewportFit({ layout }: { layout: SessionLayout }) {
  const { fitView, getViewport, setViewport } = useReactFlow();
  const canonicalViewportRef = useRef<Viewport | null>(null);
  const didInitialCanonicalFit = useRef(false);

  const enterTrigger =
    layout.kind === "candidate"
      ? candidateLayoutFingerprint(layout)
      : layout.kind === "original"
        ? "original"
        : null;

  useEffect(() => {
    return () => {
      if (layout.kind === "canonical") {
        canonicalViewportRef.current = getViewport();
      }
    };
  }, [layout.kind, getViewport]);

  useEffect(() => {
    if (enterTrigger !== null) {
      void fitView(FIT_VIEW_OPTIONS);
      return;
    }
    if (layout.kind !== "canonical") return;

    const saved = canonicalViewportRef.current;
    if (saved) {
      void setViewport(saved);
      return;
    }
    if (!didInitialCanonicalFit.current) {
      didInitialCanonicalFit.current = true;
      void fitView(FIT_VIEW_OPTIONS);
    }
  }, [enterTrigger, layout.kind, fitView, setViewport]);

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

  return (
    <ReactFlow
      nodes={nodes}
      edges={layout.edges}
      nodeTypes={nodeTypes}
      colorMode="dark"
      nodesDraggable={false}
      proOptions={{ hideAttribution: true }}
      onNodeClick={onNodeClick}
    >
      <Background />
      <GraphViewportFit layout={layout} />
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
    view.kind === "candidate" ? view.partitionId : view.kind;
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
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
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
