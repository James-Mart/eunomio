import { useCallback, useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import {
  Background,
  ReactFlow,
  type Edge as FlowEdge,
  type Node,
  type NodeMouseHandler,
  type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { api, type Graph, type GraphNode } from "@/lib/api";
import NodeCard, { type NodeCardData } from "@/components/NodeCard";
import EdgePane from "@/components/EdgePane";
import ToolsPane from "@/components/ToolsPane";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";

const nodeTypes: NodeTypes = { eunomia: NodeCard };

const NODE_WIDTH = 240;
const NODE_GAP = 120;

function layout(graph: Graph): { nodes: Node<NodeCardData>[]; edges: FlowEdge[] } {
  const order: string[] = [];
  const byParent = new Map<string | null, GraphNode[]>();
  for (const n of graph.nodes) {
    const key = n.parentNodeId ?? null;
    if (!byParent.has(key)) byParent.set(key, []);
    byParent.get(key)!.push(n);
  }
  const visit = (parent: string | null) => {
    for (const n of byParent.get(parent) ?? []) {
      order.push(n.nodeId);
      visit(n.nodeId);
    }
  };
  visit(null);

  const x = new Map<string, number>();
  order.forEach((id, i) => x.set(id, i * (NODE_WIDTH + NODE_GAP)));

  const nodes: Node<NodeCardData>[] = graph.nodes.map((n) => ({
    id: n.nodeId,
    type: "eunomia",
    position: { x: x.get(n.nodeId) ?? 0, y: 80 },
    data: { node: n },
  }));
  const edges: FlowEdge[] = graph.edges.map((e) => ({
    id: `${e.from}->${e.to}`,
    source: e.from,
    target: e.to,
  }));
  return { nodes, edges };
}

function findLeafNodeId(graph: Graph): string | null {
  const parents = new Set<string>();
  for (const n of graph.nodes) {
    if (n.parentNodeId) parents.add(n.parentNodeId);
  }
  for (const n of graph.nodes) {
    if (!parents.has(n.nodeId)) return n.nodeId;
  }
  return graph.nodes[graph.nodes.length - 1]?.nodeId ?? null;
}

export default function Session() {
  const { id } = useParams<{ id: string }>();
  const [graph, setGraph] = useState<Graph | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);

  const refresh = useCallback(async () => {
    if (!id) return;
    try {
      const g = await api.getGraph(id);
      setGraph(g);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load graph");
    }
  }, [id]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!graph) return;
    if (selectedNodeId && graph.nodes.some((n) => n.nodeId === selectedNodeId)) return;
    setSelectedNodeId(findLeafNodeId(graph));
  }, [graph, selectedNodeId]);

  const flow = useMemo(() => (graph ? layout(graph) : null), [graph]);

  const nodesForFlow = useMemo<Node<NodeCardData>[] | null>(() => {
    if (!flow || !id) return null;
    return flow.nodes.map((n) => ({
      ...n,
      selected: n.id === selectedNodeId,
      data: {
        ...n.data,
        sessionId: id,
        onChange: refresh,
      },
    }));
  }, [flow, id, refresh, selectedNodeId]);

  const onNodeClick = useCallback<NodeMouseHandler>((_event, node) => {
    setSelectedNodeId(node.id);
    if (typeof window !== "undefined" && !window.matchMedia("(min-width: 768px)").matches) {
      setSheetOpen(true);
    }
  }, []);

  const selectedNode = useMemo<GraphNode | null>(() => {
    if (!graph || !selectedNodeId) return null;
    return graph.nodes.find((n) => n.nodeId === selectedNodeId) ?? null;
  }, [graph, selectedNodeId]);

  if (error) {
    return <div className="container py-10 text-destructive">{error}</div>;
  }

  if (!flow || !nodesForFlow || !id) {
    return <SessionSkeleton />;
  }

  const graphPane = (
    <ReactFlow
      nodes={nodesForFlow}
      edges={flow.edges}
      nodeTypes={nodeTypes}
      colorMode="dark"
      fitView
      nodesDraggable={false}
      proOptions={{ hideAttribution: true }}
      onNodeClick={onNodeClick}
    >
      <Background />
    </ReactFlow>
  );

  return (
    <>
      <div className="hidden md:grid grid-cols-[7fr_3fr] h-[calc(100vh-3.5rem)]">
        <div className="min-w-0 overflow-hidden">
          {selectedNodeId && <EdgePane key={selectedNodeId} sessionId={id} targetNodeId={selectedNodeId} />}
        </div>
        <div className="grid grid-rows-2 border-l min-w-0 overflow-hidden">
          <div className="min-h-0 overflow-hidden">{graphPane}</div>
          <div className="min-h-0 overflow-auto border-t">
            {selectedNode && (
              <ToolsPane
                key={selectedNode.nodeId}
                sessionId={id}
                nodeId={selectedNode.nodeId}
                nodeTitle={selectedNode.title}
                onChange={refresh}
              />
            )}
          </div>
        </div>
      </div>

      <div className="md:hidden h-[calc(100vh-3.5rem)] w-full">{graphPane}</div>

      <Sheet open={sheetOpen} onOpenChange={setSheetOpen}>
        <SheetContent side="top" className="md:hidden h-[80vh] flex flex-col p-0">
          <div className="flex-1 min-h-0 overflow-hidden">
            {selectedNodeId && (
              <EdgePane key={selectedNodeId} sessionId={id} targetNodeId={selectedNodeId} />
            )}
          </div>
          <div className="shrink-0 border-t">
            {selectedNode && (
              <ToolsPane
                key={selectedNode.nodeId}
                sessionId={id}
                nodeId={selectedNode.nodeId}
                nodeTitle={selectedNode.title}
                onChange={refresh}
              />
            )}
          </div>
        </SheetContent>
      </Sheet>
    </>
  );
}

function SessionSkeleton() {
  return (
    <div className="hidden md:grid grid-cols-[7fr_3fr] h-[calc(100vh-3.5rem)] gap-2 p-2">
      <Skeleton className="h-full" />
      <div className="grid grid-rows-2 gap-2">
        <Skeleton className="h-full" />
        <Skeleton className="h-full" />
      </div>
    </div>
  );
}
