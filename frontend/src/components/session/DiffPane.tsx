import type { Graph, GraphNode } from "@/lib/api";
import EdgePane from "@/components/EdgePane";

import {
  CANDIDATE_SLICE_ID,
  CANDIDATE_TARGET_PREFIX,
  type SessionLayout,
} from "./layout";

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
  if (layout.kind === "candidate") {
    if (selectedNodeId === CANDIDATE_SLICE_ID) {
      const slice = layout.candidateSliceNode;
      const parent = graph.nodes.find((n) => n.nodeId === slice.parentNodeId);
      if (!parent) return <DiffPaneEmpty />;
      return (
        <EdgePane
          key="candidate-slice"
          sessionId={sessionId}
          fromTree={parent.treeSha}
          toTree={slice.treeSha}
          referenceTree={layout.renamedTargetNode.treeSha}
        />
      );
    }
    if (selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)) {
      const slice = layout.candidateSliceNode;
      const renamed = layout.renamedTargetNode;
      return (
        <EdgePane
          key="candidate-target"
          sessionId={sessionId}
          fromTree={slice.treeSha}
          toTree={renamed.treeSha}
          referenceTree={renamed.treeSha}
        />
      );
    }
    if (selectedCanonicalNode) {
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
  if (!selectedCanonicalNode) return <DiffPaneEmpty />;
  return (
    <EdgePane
      key={selectedCanonicalNode.nodeId}
      sessionId={sessionId}
      targetNodeId={selectedCanonicalNode.nodeId}
    />
  );
}

function DiffPaneEmpty() {
  return (
    <div className="flex h-full items-center justify-center bg-background p-6 text-sm text-muted-foreground">
      Select a node or partition to view diff.
    </div>
  );
}
