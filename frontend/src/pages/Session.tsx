import { useCallback, useEffect, useMemo } from "react";
import { useParams } from "react-router-dom";
import "@xyflow/react/dist/style.css";
import type { NodeMouseHandler } from "@xyflow/react";

import type { Partition } from "@/lib/api";
import ToolsCardList from "@/components/ToolsCardList";
import ToolsPane from "@/components/ToolsPane";
import { SessionEventsProvider } from "@/components/SessionEventsProvider";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";
import { isDesktopViewport } from "@/lib/useIsDesktop";

import { DiffPane } from "@/components/session/DiffPane";
import { GraphPane } from "@/components/session/GraphPane";
import {
  BottomTabBar,
  TabPanel,
} from "@/components/session/MobileTabBar";
import { SessionSkeleton } from "@/components/session/SessionSkeleton";
import {
  candidateLayout,
  partitionSiblingNumbers,
  partitionViewLabel,
} from "@/components/session/layout";
import { useSessionActiveTab } from "@/components/session/useSessionActiveTab";
import { useSessionData } from "@/components/session/useSessionData";
import { useSessionSelection } from "@/components/session/useSessionSelection";

export default function Session() {
  const { id } = useParams<{ id: string }>();
  const sessionId = id!;
  return (
    <SessionEventsProvider sessionId={sessionId}>
      <SessionInner sessionId={sessionId} />
    </SessionEventsProvider>
  );
}

function SessionInner({ sessionId }: { sessionId: string }) {
  const data = useSessionData(sessionId);
  const {
    graph,
    error,
    partitions,
    view,
    setView,
    candidatePartition,
    layout,
    chain,
    refresh,
    refreshPartitions,
    registerStartedPartition,
  } = data;

  const selection = useSessionSelection(graph, layout, view, candidatePartition);
  const {
    selectedNodeId,
    setSelectedNodeId,
    selectedCanonicalNode,
    isCandidateSliceSelected,
  } = selection;

  const { activeTab, setActiveTab } = useSessionActiveTab();

  const onNodeClick = useCallback<NodeMouseHandler>(
    (_event, node) => {
      setSelectedNodeId(node.id);
      if (!isDesktopViewport()) setActiveTab("diff");
    },
    [setActiveTab, setSelectedNodeId],
  );

  const desktopSplitLayout = useDefaultLayout({
    id: "session-desktop-split-v3",
    panelIds: ["diff", "aux"],
  });

  const desktopAuxSplitLayout = useDefaultLayout({
    id: "session-desktop-aux-split-v1",
    panelIds: ["graph", "tools"],
  });

  useEffect(() => {
    if (view.kind !== "candidate" || !candidatePartition || !chain || !graph) {
      return;
    }
    if (candidateLayout(chain, candidatePartition, graph) === null) {
      setView({ kind: "canonical" });
      setSelectedNodeId(null);
    }
  }, [view, candidatePartition, chain, graph, setSelectedNodeId, setView]);

  const siblingNumbers = useMemo(
    () => partitionSiblingNumbers(partitions),
    [partitions],
  );

  const pendingPartitionOptions = useMemo(
    () =>
      view.kind !== "canonical" || !selectedCanonicalNode || !chain
        ? []
        : partitions
            .filter((p) => p.targetNodeId === selectedCanonicalNode.nodeId)
            .map((p) => ({
              partition: p,
              label: partitionViewLabel(
                p,
                chain,
                siblingNumbers.get(p.id) ?? 1,
              ),
            })),
    [partitions, selectedCanonicalNode, chain, view.kind, siblingNumbers],
  );

  if (error) {
    return <div className="container py-10 text-destructive">{error}</div>;
  }

  if (!layout || !graph || !chain) {
    return <SessionSkeleton />;
  }

  const selectPartition = (p: Partition) => {
    setView({ kind: "candidate", partitionId: p.id });
    setSelectedNodeId(p.targetNodeId);
  };

  const onSelectView = (next: string) => {
    if (next === "canonical") {
      setView({ kind: "canonical" });
      setSelectedNodeId(null);
      return;
    }
    if (next === "original") {
      const final = chain.ordered[chain.ordered.length - 1] ?? null;
      setView({ kind: "original" });
      setSelectedNodeId(final?.nodeId ?? null);
      return;
    }
    const pid = Number(next);
    const p = partitions.find((x) => x.id === pid);
    if (!p) return;
    selectPartition(p);
  };

  const onPartitionStarted = (p: Partition) => {
    registerStartedPartition(p);
    selectPartition(p);
  };

  const onPartitionEnded = () => {
    setView((prev) => (prev.kind === "candidate" ? { kind: "canonical" } : prev));
    setSelectedNodeId(null);
    void refreshPartitions();
  };

  const toolsContext = {
    sessionId,
    nodeId: selectedCanonicalNode?.nodeId ?? null,
    nodeTitle: selectedCanonicalNode?.title ?? null,
    nodeDescription: selectedCanonicalNode?.description ?? null,
    nodeStrategy: selectedCanonicalNode?.strategy ?? null,
    activePartition: candidatePartition,
    isCandidateSliceSelected,
    pendingPartitionOptions,
    onSelectPartition: selectPartition,
    onPartitionStarted,
    onPartitionEnded,
    onChange: refresh,
  };

  const graphPane = (
    <GraphPane
      layout={layout}
      chain={chain}
      partitions={partitions}
      view={view}
      onSelectView={onSelectView}
      selectedNodeId={selectedNodeId}
      onNodeClick={onNodeClick}
    />
  );

  const diffPane = (
    <DiffPane
      sessionId={sessionId}
      layout={layout}
      selectedNodeId={selectedNodeId}
      selectedCanonicalNode={selectedCanonicalNode}
      graph={graph}
    />
  );

  const toolsCardList = <ToolsCardList {...toolsContext} />;

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
            <div className="h-full min-w-0 overflow-hidden pb-2 pr-2">{diffPane}</div>
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
                <ToolsPane {...toolsContext} />
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
