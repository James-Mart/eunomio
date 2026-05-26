/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, useMemo, useRef } from "react";
import {
  Background,
  MarkerType,
  ReactFlow,
  ReactFlowProvider,
  useNodesInitialized,
  useReactFlow,
  useStore,
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

type Props = {
  layout: SessionLayout;
  chain: Chain;
  partitions: Partition[];
  view: View;
  onSelectView: (next: string) => void;
  selectedNodeId: string | null;
  onNodeClick: NodeMouseHandler;
  onNodeReviewedChange?: (nodeId: string, reviewed: boolean) => void;
};

const FIT_VIEW_OPTIONS = { padding: 0.2 } as const;

function isUntransformedViewport({ x, y, zoom }: Viewport): boolean {
  return zoom === 1 && Math.abs(x) < 0.5 && Math.abs(y) < 0.5;
}

/**
 * - Fit when entering original or candidate layouts.
 * - On first canonical load, fit once.
 * - When returning to canonical, restore pan/zoom from before leaving.
 */
function GraphViewportFit({ layout }: { layout: SessionLayout }) {
  const { fitView, getViewport, setViewport, viewportInitialized } =
    useReactFlow();
  const nodesInitialized = useNodesInitialized();
  const width = useStore((state) => state.width);
  const height = useStore((state) => state.height);
  const canonicalViewportRef = useRef<Viewport | null>(null);
  const didInitialCanonicalFit = useRef(false);

  const viewportReady =
    viewportInitialized && nodesInitialized && width > 0 && height > 0;

  const layoutNodeIds = useMemo(
    () => layout.nodes.map((node) => node.id).join("\0"),
    [layout.nodes],
  );

  const enterTrigger =
    layout.kind === "candidate"
      ? candidateLayoutFingerprint(layout)
      : layout.kind === "original"
        ? "original"
        : null;

  useEffect(() => {
    if (layout.kind !== "canonical") return;
    return () => {
      const viewport = getViewport();
      if (!isUntransformedViewport(viewport)) {
        canonicalViewportRef.current = viewport;
      }
    };
  }, [layout.kind]);

  useEffect(() => {
    if (!viewportReady) return;

    if (enterTrigger !== null) {
      void fitView(FIT_VIEW_OPTIONS);
      return;
    }
    if (layout.kind !== "canonical") return;

    const saved = canonicalViewportRef.current;
    if (saved && !isUntransformedViewport(saved)) {
      canonicalViewportRef.current = null;
      didInitialCanonicalFit.current = true;
      void setViewport(saved);
      return;
    }
    if (didInitialCanonicalFit.current) return;

    void fitView({
      ...FIT_VIEW_OPTIONS,
      nodes: layout.nodes.map((node) => ({ id: node.id })),
    }).then(() => {
      if (!isUntransformedViewport(getViewport())) {
        didInitialCanonicalFit.current = true;
      }
    });
  }, [
    enterTrigger,
    layout.kind,
    layoutNodeIds,
    fitView,
    setViewport,
    viewportReady,
    width,
    height,
    layout.nodes,
  ]);

  return null;
}

function GraphFlow({
  layout,
  selectedNodeId,
  onNodeClick,
  onNodeReviewedChange,
}: {
  layout: SessionLayout;
  selectedNodeId: string | null;
  onNodeClick: NodeMouseHandler;
  onNodeReviewedChange?: (nodeId: string, reviewed: boolean) => void;
}) {
  const nodeTypes = useMemo<NodeTypes>(() => {
    if (layout.kind !== "canonical" || !onNodeReviewedChange) {
      return { eunomio: NodeCard };
    }
    return {
      eunomio: (props) => (
        <NodeCard
          {...props}
          onReviewedChange={(reviewed) =>
            onNodeReviewedChange(props.id, reviewed)
          }
        />
      ),
    };
  }, [layout.kind, onNodeReviewedChange]);

  const nodes = useMemo<Node<NodeCardData>[]>(
    () =>
      layout.nodes.map((n) =>
        n.id === selectedNodeId ? { ...n, selected: true } : n,
      ),
    [layout, selectedNodeId],
  );

  return (
    <ReactFlow
      className="h-full w-full"
      nodes={nodes}
      edges={layout.edges}
      nodeTypes={nodeTypes}
      defaultEdgeOptions={{
        style: { stroke: "hsl(var(--border))", strokeWidth: 1 },
        markerEnd: {
          type: MarkerType.ArrowClosed,
          width: 16,
          height: 16,
          color: "hsl(var(--border))",
        },
      }}
      colorMode="dark"
      nodesDraggable={false}
      proOptions={{ hideAttribution: true }}
      onNodeClick={onNodeClick}
    >
      <Background
        gap={24}
        size={0.75}
        color="hsl(var(--border) / 0.25)"
      />
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
  onNodeReviewedChange,
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
            onNodeReviewedChange={onNodeReviewedChange}
          />
        </ReactFlowProvider>
      </div>
    </div>
  );
}
