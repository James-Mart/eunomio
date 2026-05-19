import { useCallback, useEffect, useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import {
  Background,
  ReactFlow,
  type Edge as FlowEdge,
  type Node,
  type NodeMouseHandler,
  type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  GitCompareArrows,
  Network,
  Wrench,
  type LucideIcon,
} from "lucide-react";

import { api, type Graph, type GraphNode } from "@/lib/api";
import NodeCard, { type NodeCardData } from "@/components/NodeCard";
import EdgePane from "@/components/EdgePane";
import ToolsCardList from "@/components/ToolsCardList";
import ToolsPane from "@/components/ToolsPane";
import { SessionEventsProvider } from "@/components/SessionEventsProvider";
import { Skeleton } from "@/components/ui/skeleton";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";
import { cn } from "@/lib/utils";

const nodeTypes: NodeTypes = { eunomia: NodeCard };

const NODE_WIDTH = 240;
const NODE_GAP = 120;

type ActiveTab = "graph" | "diff" | "tools";

const TABS: { value: ActiveTab; label: string; icon: LucideIcon }[] = [
  { value: "graph", label: "Graph", icon: Network },
  { value: "diff", label: "Diff", icon: GitCompareArrows },
  { value: "tools", label: "Tools", icon: Wrench },
];

function layout(graph: Graph): {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
} {
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

function parseActiveTab(raw: string | null): ActiveTab {
  return raw === "diff" || raw === "tools" ? raw : "graph";
}

export default function Session() {
  const { id } = useParams<{ id: string }>();
  const [graph, setGraph] = useState<Graph | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab = parseActiveTab(searchParams.get("tab"));

  const setActiveTab = useCallback(
    (next: ActiveTab) => {
      setSearchParams(
        (prev) => {
          const updated = new URLSearchParams(prev);
          if (next === "graph") updated.delete("tab");
          else updated.set("tab", next);
          return updated;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );

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
    if (selectedNodeId && graph.nodes.some((n) => n.nodeId === selectedNodeId))
      return;
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

  const onNodeClick = useCallback<NodeMouseHandler>(
    (_event, node) => {
      setSelectedNodeId(node.id);
      if (
        typeof window !== "undefined" &&
        !window.matchMedia("(min-width: 768px)").matches
      ) {
        setActiveTab("diff");
      }
    },
    [setActiveTab],
  );

  const selectedNode = useMemo<GraphNode | null>(() => {
    if (!graph || !selectedNodeId) return null;
    return graph.nodes.find((n) => n.nodeId === selectedNodeId) ?? null;
  }, [graph, selectedNodeId]);
  const desktopSplitLayout = useDefaultLayout({
    id: "session-desktop-split-v3",
    panelIds: ["diff", "aux"],
  });

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

  const diffPane = selectedNodeId ? (
    <EdgePane
      key={selectedNodeId}
      sessionId={id}
      targetNodeId={selectedNodeId}
    />
  ) : null;

  const toolsCardList = selectedNode ? (
    <ToolsCardList
      key={selectedNode.nodeId}
      sessionId={id}
      nodeId={selectedNode.nodeId}
      nodeTitle={selectedNode.title}
      onChange={refresh}
    />
  ) : null;

  return (
    <SessionEventsProvider sessionId={id}>
      <div className="hidden md:block h-[calc(100vh-3.5rem)]">
        <ResizablePanelGroup
          orientation="horizontal"
          defaultLayout={desktopSplitLayout.defaultLayout}
          onLayoutChanged={desktopSplitLayout.onLayoutChanged}
          className="h-full"
        >
          <ResizablePanel
            id="diff"
            defaultSize="70%"
            minSize="30%"
            maxSize="85%"
            className="min-w-0"
          >
            <div className="h-full min-w-0 overflow-hidden">{diffPane}</div>
          </ResizablePanel>
          <ResizableHandle
            withHandle
            aria-label="Resize panes"
            className="mx-4"
          />
          <ResizablePanel
            id="aux"
            defaultSize="30%"
            minSize="15%"
            className="min-w-0"
          >
            <div className="grid h-full min-w-0 overflow-hidden grid-rows-2">
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
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>

      <div className="md:hidden flex h-[calc(100dvh-3.5rem)] flex-col">
        <div className="relative flex-1 min-h-0">
          <TabPanel id="graph" active={activeTab === "graph"}>
            {graphPane}
          </TabPanel>
          <TabPanel id="diff" active={activeTab === "diff"}>
            {diffPane}
          </TabPanel>
          <TabPanel id="tools" active={activeTab === "tools"}>
            {toolsCardList}
          </TabPanel>
        </div>
        <BottomTabBar value={activeTab} onChange={setActiveTab} />
      </div>
    </SessionEventsProvider>
  );
}

function TabPanel({
  id,
  active,
  children,
}: {
  id: ActiveTab;
  active: boolean;
  children: React.ReactNode;
}) {
  return (
    <div
      role="tabpanel"
      id={`session-panel-${id}`}
      aria-labelledby={`session-tab-${id}`}
      aria-hidden={!active}
      className={cn(
        "absolute inset-0",
        !active && "invisible pointer-events-none",
      )}
    >
      {children}
    </div>
  );
}

function BottomTabBar({
  value,
  onChange,
}: {
  value: ActiveTab;
  onChange: (next: ActiveTab) => void;
}) {
  return (
    <nav
      role="tablist"
      aria-label="Session view"
      className="flex h-16 shrink-0 items-stretch border-t bg-background pb-[env(safe-area-inset-bottom)]"
    >
      {TABS.map(({ value: tabValue, label, icon: Icon }) => {
        const isActive = tabValue === value;
        return (
          <button
            key={tabValue}
            type="button"
            role="tab"
            id={`session-tab-${tabValue}`}
            aria-selected={isActive}
            aria-controls={`session-panel-${tabValue}`}
            onClick={() => onChange(tabValue)}
            className={cn(
              "flex flex-1 flex-col items-center justify-center gap-0.5 text-xs transition-colors",
              isActive
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <span
              className={cn(
                "flex h-7 w-12 items-center justify-center rounded-full",
                isActive && "bg-secondary",
              )}
            >
              <Icon className="h-5 w-5" aria-hidden="true" />
            </span>
            <span className={cn(isActive && "font-medium")}>{label}</span>
          </button>
        );
      })}
    </nav>
  );
}

function SessionSkeleton() {
  return (
    <>
      <div className="hidden md:grid grid-cols-[7fr_3fr] h-[calc(100vh-3.5rem)] gap-2 p-2">
        <Skeleton className="h-full" />
        <div className="grid grid-rows-2 gap-2">
          <Skeleton className="h-full" />
          <Skeleton className="h-full" />
        </div>
      </div>
      <div className="md:hidden flex h-[calc(100dvh-3.5rem)] flex-col gap-2 p-2">
        <Skeleton className="flex-1" />
        <Skeleton className="h-16 shrink-0" />
      </div>
    </>
  );
}
