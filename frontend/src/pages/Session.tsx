/* SPDX-License-Identifier: Apache-2.0 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { Navigate, useParams } from "react-router-dom";
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
import { isDesktopViewport, useIsDesktop } from "@/lib/useIsDesktop";

import { DiffPane } from "@/components/session/DiffPane";
import { GraphPane } from "@/components/session/GraphPane";
import {
  BottomTabBar,
  mobileTabBarInsetClass,
  TabPanel,
} from "@/components/session/MobileTabBar";
import { SessionSkeleton } from "@/components/session/SessionSkeleton";
import {
  candidateLayout,
  partitionSiblingNumbers,
  comparePartitionsForView,
  partitionViewLabel,
} from "@/components/session/layout";
import { useSessionActiveTab } from "@/components/session/useSessionActiveTab";
import { useSessionData } from "@/components/session/useSessionData";
import { useSessionSelection } from "@/components/session/useSessionSelection";
import { sessionNotFoundHomePath } from "@/lib/sessionNotFound";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

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
    notFound,
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
    setNodeReviewed,
  } = data;

  const selection = useSessionSelection(graph, layout, view, candidatePartition);
  const {
    selectedNodeId,
    setSelectedNodeId,
    selectedCanonicalNode,
    isCandidateSliceSelected,
  } = selection;

  const { activeTab, setActiveTab } = useSessionActiveTab();
  const [isLocal, setIsLocal] = useState(true);
  const isDesktop = useIsDesktop();

  useEffect(() => {
    let cancelled = false;
    api.getSession(sessionId).then((s) => {
      if (!cancelled) setIsLocal(s.isLocal);
    }).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

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
            .sort((a, b) =>
              comparePartitionsForView(a, b, chain, siblingNumbers),
            )
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

  if (notFound) {
    return <Navigate to={sessionNotFoundHomePath()} replace />;
  }

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
    const p = partitions.find((x) => x.id === next);
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
    isLocal,
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
      onNodeReviewedChange={setNodeReviewed}
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
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="hidden min-h-0 flex-1 md:flex md:flex-col">
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
                {isDesktop ? graphPane : null}
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

      <div
        className={cn(
          "flex min-h-0 flex-1 flex-col overflow-hidden md:hidden",
          mobileTabBarInsetClass,
        )}
      >
        <div className="relative min-h-0 flex-1 overflow-hidden">
          <TabPanel id="graph" active={activeTab === "graph"}>
            {!isDesktop ? graphPane : null}
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
    </div>
  );
}
