import type { Graph, GraphNode } from "@/lib/api";
import EdgePane from "@/components/EdgePane";

import {
  CANDIDATE_SLICE_ID,
  CANDIDATE_TARGET_PREFIX,
  type SessionLayout,
} from "./layout";

export const CANDIDATE_ROOT_DIFF_MESSAGE = "No diff — root of partition view.";

type Props = {
  sessionId: string;
  layout: SessionLayout;
  selectedNodeId: string | null;
  selectedCanonicalNode: GraphNode | null;
  graph: Graph;
};

export function DiffPane({
  sessionId,
  layout,
  selectedNodeId,
  selectedCanonicalNode,
  graph,
}: Props) {
  if (!selectedNodeId) return <DiffPaneEmpty />;

  if (layout.kind === "original") {
    if (selectedNodeId === layout.finalNode.nodeId) {
      return (
        <EdgePane
          key={`original-${layout.baseNode.nodeId}->${layout.finalNode.nodeId}`}
          sessionId={sessionId}
          fromTree={layout.baseNode.treeSha}
          toTree={layout.finalNode.treeSha}
        />
      );
    }
    return <DiffPaneEmpty />;
  }

  if (layout.kind === "candidate") {
    if (selectedNodeId === layout.rootNodeId) {
      return <DiffPaneEmpty message={CANDIDATE_ROOT_DIFF_MESSAGE} />;
    }

    if (layout.stage === "pending") {
      if (
        selectedNodeId === layout.targetNode.nodeId &&
        selectedCanonicalNode
      ) {
        return (
          <EdgePane
            key={selectedCanonicalNode.nodeId}
            sessionId={sessionId}
            targetNodeId={selectedCanonicalNode.nodeId}
          />
        );
      }
      return <DiffPaneEmpty />;
    }

    const slice = layout.candidateSliceNode;
    const renamed = layout.renamedTargetNode;
    const parent = graph.nodes.find((n) => n.nodeId === slice.parentNodeId);
    if (selectedNodeId === CANDIDATE_SLICE_ID) {
      if (!parent) return <DiffPaneEmpty />;
      return (
        <EdgePane
          key="candidate-slice"
          sessionId={sessionId}
          fromTree={parent.treeSha}
          toTree={slice.treeSha}
          beforeRef={parent.treeSha}
          afterRef={renamed.treeSha}
        />
      );
    }
    if (selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)) {
      if (!parent) return <DiffPaneEmpty />;
      return (
        <EdgePane
          key="candidate-target"
          sessionId={sessionId}
          fromTree={slice.treeSha}
          toTree={renamed.treeSha}
          beforeRef={parent.treeSha}
          afterRef={renamed.treeSha}
        />
      );
    }
    return <DiffPaneEmpty />;
  }

  if (!selectedCanonicalNode) return <DiffPaneEmpty />;
  return (
    <EdgePane
      key={selectedCanonicalNode.nodeId}
      sessionId={sessionId}
      targetNodeId={selectedCanonicalNode.nodeId}
    />
  );
}

function DiffPaneEmpty({
  message = "Select a node or partition to view diff.",
}: {
  message?: string;
}) {
  return (
    <div className="flex h-full items-center justify-center bg-background p-6 text-sm text-muted-foreground">
      {message}
    </div>
  );
}
