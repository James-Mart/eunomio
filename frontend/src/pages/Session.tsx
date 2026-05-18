import { useCallback, useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import {
  Background,
  Controls,
  ReactFlow,
  type Edge,
  type Node,
  type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { api, type Graph, type GraphNode } from "@/lib/api";
import NodeCard, { type NodeCardData } from "@/components/NodeCard";

const nodeTypes: NodeTypes = { eunomia: NodeCard };

const NODE_WIDTH = 280;
const NODE_GAP = 120;

function layout(graph: Graph): { nodes: Node<NodeCardData>[]; edges: Edge[] } {
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
  const edges: Edge[] = graph.edges.map((e) => ({
    id: `${e.from}->${e.to}`,
    source: e.from,
    target: e.to,
  }));
  return { nodes, edges };
}

export default function Session() {
  const { id } = useParams<{ id: string }>();
  const [graph, setGraph] = useState<Graph | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  const flow = useMemo(() => (graph ? layout(graph) : null), [graph]);
  const nodesWithHandlers = useMemo<Node<NodeCardData>[] | null>(() => {
    if (!flow || !id) return null;
    return flow.nodes.map((n) => ({
      ...n,
      data: {
        ...n.data,
        sessionId: id,
        onChange: refresh,
      },
    }));
  }, [flow, id, refresh]);

  if (error) {
    return <div className="container py-10 text-destructive">{error}</div>;
  }
  if (!flow || !nodesWithHandlers) {
    return <div className="container py-10 text-muted-foreground">Loading session…</div>;
  }

  return (
    <div className="h-[calc(100vh-3.5rem)] w-full">
      <ReactFlow
        nodes={nodesWithHandlers}
        edges={flow.edges}
        nodeTypes={nodeTypes}
        colorMode="dark"
        fitView
        proOptions={{ hideAttribution: true }}
      >
        <Background />
        <Controls />
      </ReactFlow>
    </div>
  );
}
