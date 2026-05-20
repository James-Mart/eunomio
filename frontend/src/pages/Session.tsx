import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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

import { api, type Graph, type GraphNode, type Partition } from "@/lib/api";
import NodeCard, {
  type BadgeState,
  type NodeCardData,
} from "@/components/NodeCard";
import EdgePane from "@/components/EdgePane";
import ToolsCardList from "@/components/ToolsCardList";
import ToolsPane from "@/components/ToolsPane";
import {
  SessionEventsProvider,
  useConstructSubscription,
  useHydratePartition,
} from "@/components/SessionEventsProvider";
import { Skeleton } from "@/components/ui/skeleton";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";

const nodeTypes: NodeTypes = { eunomia: NodeCard };

const NODE_X = 0;
const NODE_Y_STEP = 140;
const CANDIDATE_X_OFFSET = 260;

type ActiveTab = "graph" | "diff" | "tools";

const TABS: { value: ActiveTab; label: string; icon: LucideIcon }[] = [
  { value: "graph", label: "Graph", icon: Network },
  { value: "diff", label: "Diff", icon: GitCompareArrows },
  { value: "tools", label: "Tools", icon: Wrench },
];

type Chain = {
  ordered: GraphNode[];
  positionByNodeId: Map<string, string>;
};

function computeChain(graph: Graph): Chain {
  const byParent = new Map<string | null, GraphNode[]>();
  for (const n of graph.nodes) {
    const key = n.parentNodeId ?? null;
    if (!byParent.has(key)) byParent.set(key, []);
    byParent.get(key)!.push(n);
  }
  const ordered: GraphNode[] = [];
  const visit = (parent: string | null) => {
    for (const n of byParent.get(parent) ?? []) {
      ordered.push(n);
      visit(n.nodeId);
    }
  };
  visit(null);

  const positionByNodeId = new Map<string, string>();
  ordered.forEach((node, idx) => {
    if (node.parentNodeId === null) {
      positionByNodeId.set(node.nodeId, "base");
    } else if (idx === ordered.length - 1 && node.title === "final") {
      positionByNodeId.set(node.nodeId, "final");
    } else {
      positionByNodeId.set(node.nodeId, String(idx));
    }
  });
  return { ordered, positionByNodeId };
}

type CanonicalLayout = {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
};

function canonicalLayout(
  chain: Chain,
  badgeByNode: Map<string, BadgeState>,
): CanonicalLayout {
  const total = chain.ordered.length;
  const nodes: Node<NodeCardData>[] = chain.ordered.map((n, idx) => ({
    id: n.nodeId,
    type: "eunomia",
    position: { x: NODE_X, y: (total - 1 - idx) * NODE_Y_STEP },
    data: {
      node: n,
      positionLabel: chain.positionByNodeId.get(n.nodeId) ?? "",
      badgeState: badgeByNode.get(n.nodeId) ?? "none",
    },
  }));
  const edges: FlowEdge[] = chain.ordered
    .filter((n) => n.parentNodeId !== null)
    .map((n) => ({
      id: `${n.parentNodeId}->${n.nodeId}`,
      source: n.parentNodeId!,
      target: n.nodeId,
    }));
  return { nodes, edges };
}

const CANDIDATE_SLICE_ID = "__candidate_slice__";
const CANDIDATE_TARGET_PREFIX = "__candidate_target__";

type CandidateLayout = {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
  candidateSliceNode: GraphNode;
  renamedTargetNode: GraphNode;
};

function candidateLayout(
  chain: Chain,
  partition: Partition,
  graph: Graph,
): CandidateLayout | null {
  const targetIdx = chain.ordered.findIndex(
    (n) => n.nodeId === partition.targetNodeId,
  );
  if (targetIdx < 0) return null;
  const target = chain.ordered[targetIdx];
  if (target.parentNodeId === null) return null;
  const parent = graph.nodes.find((n) => n.nodeId === target.parentNodeId);
  if (!parent) return null;
  if (
    !partition.candidateSliceTreeSha ||
    !partition.candidateSliceCommitSha ||
    !partition.plan
  ) {
    return null;
  }
  const parentPosition = chain.positionByNodeId.get(parent.nodeId) ?? "?";

  const candidateSlice: GraphNode = {
    nodeId: CANDIDATE_SLICE_ID,
    parentNodeId: parent.nodeId,
    treeSha: partition.candidateSliceTreeSha,
    commitSha: partition.candidateSliceCommitSha,
    title: partition.plan.edges[0].title,
  };
  const renamedTarget: GraphNode = {
    ...target,
    nodeId: CANDIDATE_TARGET_PREFIX + target.nodeId,
    parentNodeId: CANDIDATE_SLICE_ID,
    title: partition.plan.edges[1].title,
  };

  const isSeedFinal =
    targetIdx === chain.ordered.length - 1 && target.title === "final";
  const sliceLabel = String(targetIdx);
  const targetLabel = isSeedFinal ? "final" : String(targetIdx + 1);
  const parentLabel = parentPosition;

  const nodes: Node<NodeCardData>[] = [
    {
      id: parent.nodeId,
      type: "eunomia",
      position: { x: NODE_X, y: 2 * NODE_Y_STEP },
      data: {
        node: parent,
        positionLabel: parentLabel,
      },
    },
    {
      id: candidateSlice.nodeId,
      type: "eunomia",
      position: { x: NODE_X + CANDIDATE_X_OFFSET, y: NODE_Y_STEP },
      data: {
        node: candidateSlice,
        positionLabel: sliceLabel,
      },
    },
    {
      id: renamedTarget.nodeId,
      type: "eunomia",
      position: { x: NODE_X, y: 0 },
      data: {
        node: renamedTarget,
        positionLabel: targetLabel,
      },
    },
  ];

  const edges: FlowEdge[] = [
    {
      id: `${parent.nodeId}->${candidateSlice.nodeId}`,
      source: parent.nodeId,
      target: candidateSlice.nodeId,
    },
    {
      id: `${candidateSlice.nodeId}->${renamedTarget.nodeId}`,
      source: candidateSlice.nodeId,
      target: renamedTarget.nodeId,
    },
  ];

  return {
    nodes,
    edges,
    candidateSliceNode: candidateSlice,
    renamedTargetNode: renamedTarget,
  };
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

function willRenderCandidateLayout(p: Partition): boolean {
  return (
    p.phase === "construct" &&
    p.phaseState === "awaiting_review" &&
    !!p.candidateSliceTreeSha &&
    !!p.candidateSliceCommitSha &&
    !!p.plan
  );
}

function phaseLabel(p: Partition): string {
  if (p.phaseState === "error") return `${p.phase} error`;
  if (p.phaseState === "awaiting_review") return `${p.phase} review`;
  if (p.phase === "survey") return "surveying…";
  if (p.phase === "plan") return "planning…";
  return "constructing…";
}

export default function Session() {
  const { id } = useParams<{ id: string }>();
  if (!id) return null;
  return (
    <SessionEventsProvider sessionId={id}>
      <SessionInner sessionId={id} />
    </SessionEventsProvider>
  );
}

function SessionInner({ sessionId }: { sessionId: string }) {
  const id = sessionId;
  const [graph, setGraph] = useState<Graph | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [partitions, setPartitions] = useState<Partition[]>([]);
  const [candidatePartitionId, setCandidatePartitionId] = useState<
    number | null
  >(null);
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
    try {
      const g = await api.getGraph(id);
      setGraph(g);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load graph");
    }
  }, [id]);

  const refreshPartitions = useCallback(async () => {
    try {
      const list = await api.listPartitions(id);
      setPartitions(list);
    } catch {
      setPartitions([]);
    }
  }, [id]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    void refreshPartitions();
  }, [refreshPartitions]);

  useConstructSubscription(
    useCallback(() => {
      void refreshPartitions();
      void refresh();
    }, [refresh, refreshPartitions]),
  );

  const hydratePartition = useHydratePartition();

  useEffect(() => {
    if (candidatePartitionId === null) return;
    if (!partitions.some((p) => p.id === candidatePartitionId)) {
      setCandidatePartitionId(null);
    }
  }, [partitions, candidatePartitionId]);

  const chain = useMemo(() => (graph ? computeChain(graph) : null), [graph]);

  const candidatePartition = useMemo(
    () => partitions.find((p) => p.id === candidatePartitionId) ?? null,
    [partitions, candidatePartitionId],
  );

  const badgeByNode = useMemo(() => {
    const m = new Map<string, BadgeState>();
    for (const p of partitions) {
      const next: BadgeState =
        p.phaseState === "awaiting_review" || p.phaseState === "error"
          ? "awaiting"
          : "running";
      const prev = m.get(p.targetNodeId);
      if (prev === "awaiting") continue;
      m.set(p.targetNodeId, next);
    }
    return m;
  }, [partitions]);

  const layout = useMemo(() => {
    if (!chain || !graph) return null;
    if (candidatePartition) {
      const lay = candidateLayout(chain, candidatePartition, graph);
      if (lay) return { kind: "candidate" as const, ...lay };
    }
    const lay = canonicalLayout(chain, badgeByNode);
    return { kind: "canonical" as const, ...lay };
  }, [chain, graph, candidatePartition, badgeByNode]);

  const didInitSelectionRef = useRef(false);
  useEffect(() => {
    if (!graph || didInitSelectionRef.current) return;
    didInitSelectionRef.current = true;
    setSelectedNodeId(findLeafNodeId(graph));
  }, [graph]);

  useEffect(() => {
    if (!layout || !selectedNodeId) return;
    if (layout.nodes.some((n) => n.id === selectedNodeId)) return;
    if (selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)) {
      const stripped = selectedNodeId.slice(CANDIDATE_TARGET_PREFIX.length);
      if (layout.nodes.some((n) => n.id === stripped)) {
        setSelectedNodeId(stripped);
        return;
      }
    }
    setSelectedNodeId(null);
  }, [layout, selectedNodeId]);

  const nodesForFlow = useMemo<Node<NodeCardData>[] | null>(() => {
    if (!layout || !id) return null;
    return layout.nodes.map((n) => ({
      ...n,
      selected: n.id === selectedNodeId,
      data: {
        ...n.data,
        sessionId: id,
        onChange: refresh,
      },
    }));
  }, [layout, id, refresh, selectedNodeId]);

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

  const selectedCanonicalNode = useMemo<GraphNode | null>(() => {
    if (!graph || !selectedNodeId) return null;
    const resolved = selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)
      ? selectedNodeId.slice(CANDIDATE_TARGET_PREFIX.length)
      : selectedNodeId;
    return graph.nodes.find((n) => n.nodeId === resolved) ?? null;
  }, [graph, selectedNodeId]);

  const isCandidateSliceSelected =
    layout?.kind === "candidate" && selectedNodeId === CANDIDATE_SLICE_ID;

  const desktopSplitLayout = useDefaultLayout({
    id: "session-desktop-split-v3",
    panelIds: ["diff", "aux"],
  });

  const desktopAuxSplitLayout = useDefaultLayout({
    id: "session-desktop-aux-split-v1",
    panelIds: ["graph", "tools"],
  });

  if (error) {
    return <div className="container py-10 text-destructive">{error}</div>;
  }

  if (!layout || !nodesForFlow || !graph || !chain) {
    return <SessionSkeleton />;
  }

  const onSelectCandidate = (next: string) => {
    if (next === "canonical") {
      setCandidatePartitionId(null);
      setSelectedNodeId(null);
      return;
    }
    const pid = Number(next);
    const p = partitions.find((x) => x.id === pid);
    if (!p) return;
    setCandidatePartitionId(pid);
    setSelectedNodeId(
      willRenderCandidateLayout(p)
        ? CANDIDATE_TARGET_PREFIX + p.targetNodeId
        : p.targetNodeId,
    );
  };

  const onPartitionStarted = (p: Partition) => {
    hydratePartition(p);
    setPartitions((prev) =>
      prev.some((x) => x.id === p.id) ? prev : [...prev, p],
    );
    setSelectedNodeId(null);
  };

  const onPartitionEnded = () => {
    setCandidatePartitionId(null);
    setSelectedNodeId(null);
    void refreshPartitions();
  };

  const graphPane = (
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
                const strategy = p.strategy ?? "semantic";
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
          nodes={nodesForFlow}
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

  const diffPane = renderDiffPane({
    id,
    layout,
    selectedNodeId,
    selectedCanonicalNode,
    graph,
  });

  const toolsCardList = (
    <ToolsCardList
      sessionId={id}
      nodeId={selectedCanonicalNode?.nodeId ?? null}
      nodeTitle={selectedCanonicalNode?.title ?? null}
      activePartition={candidatePartition}
      isCandidateSliceSelected={isCandidateSliceSelected}
      onPartitionStarted={onPartitionStarted}
      onPartitionEnded={onPartitionEnded}
      onChange={refresh}
    />
  );

  return (
    <>
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
          <ResizableHandle withHandle aria-label="Resize panes" />
          <ResizablePanel
            id="aux"
            defaultSize="30%"
            minSize="15%"
            className="min-w-0"
          >
            <ResizablePanelGroup
              orientation="vertical"
              defaultLayout={desktopAuxSplitLayout.defaultLayout}
              onLayoutChanged={desktopAuxSplitLayout.onLayoutChanged}
              className="h-full min-w-0 overflow-hidden"
            >
              <ResizablePanel
                id="graph"
                defaultSize="50%"
                minSize="15%"
                maxSize="85%"
                className="min-h-0 overflow-hidden"
              >
                {graphPane}
              </ResizablePanel>
              <ResizableHandle withHandle aria-label="Resize tools and graph" />
              <ResizablePanel
                id="tools"
                defaultSize="50%"
                minSize="15%"
                maxSize="85%"
                className="min-h-0 overflow-auto"
              >
                <ToolsPane
                  sessionId={id}
                  nodeId={selectedCanonicalNode?.nodeId ?? null}
                  nodeTitle={selectedCanonicalNode?.title ?? null}
                  activePartition={candidatePartition}
                  isCandidateSliceSelected={isCandidateSliceSelected}
                  onPartitionStarted={onPartitionStarted}
                  onPartitionEnded={onPartitionEnded}
                  onChange={refresh}
                />
              </ResizablePanel>
            </ResizablePanelGroup>
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
    </>
  );
}

function renderDiffPane({
  id,
  layout,
  selectedNodeId,
  selectedCanonicalNode,
  graph,
}: {
  id: string;
  layout:
    | (CanonicalLayout & { kind: "canonical" })
    | (CandidateLayout & { kind: "candidate" });
  selectedNodeId: string | null;
  selectedCanonicalNode: GraphNode | null;
  graph: Graph;
}) {
  if (!selectedNodeId) return <DiffPaneEmpty />;
  if (layout.kind === "candidate") {
    if (selectedNodeId === CANDIDATE_SLICE_ID) {
      const slice = layout.candidateSliceNode;
      const parent = graph.nodes.find((n) => n.nodeId === slice.parentNodeId);
      if (!parent) return <DiffPaneEmpty />;
      return (
        <EdgePane
          key={`candidate-slice`}
          sessionId={id}
          fromTree={parent.treeSha}
          toTree={slice.treeSha}
        />
      );
    }
    if (selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)) {
      const slice = layout.candidateSliceNode;
      const renamed = layout.renamedTargetNode;
      return (
        <EdgePane
          key={`candidate-target`}
          sessionId={id}
          fromTree={slice.treeSha}
          toTree={renamed.treeSha}
        />
      );
    }
    if (selectedCanonicalNode) {
      return (
        <EdgePane
          key={selectedCanonicalNode.nodeId}
          sessionId={id}
          targetNodeId={selectedCanonicalNode.nodeId}
        />
      );
    }
    return <DiffPaneEmpty />;
  }
  if (!selectedCanonicalNode) return <DiffPaneEmpty />;
  return (
    <EdgePane
      key={selectedCanonicalNode.nodeId}
      sessionId={id}
      targetNodeId={selectedCanonicalNode.nodeId}
    />
  );
}

function DiffPaneEmpty() {
  return (
    <div className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">
      Select a node or partition to view diff.
    </div>
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
