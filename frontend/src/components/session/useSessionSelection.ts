import { useEffect, useMemo, useRef, useState } from "react";

import type { Graph, GraphNode } from "@/lib/api";

import {
  CANDIDATE_SLICE_ID,
  CANDIDATE_TARGET_PREFIX,
  findLeafNodeId,
  type SessionLayout,
} from "./layout";

export type SessionSelection = {
  selectedNodeId: string | null;
  setSelectedNodeId: (id: string | null) => void;
  selectedCanonicalNode: GraphNode | null;
  isCandidateSliceSelected: boolean;
};

export function useSessionSelection(
  graph: Graph | null,
  layout: SessionLayout | null,
): SessionSelection {
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  const didInitRef = useRef(false);
  useEffect(() => {
    if (!graph || didInitRef.current) return;
    didInitRef.current = true;
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

  const selectedCanonicalNode = useMemo<GraphNode | null>(() => {
    if (!graph || !selectedNodeId) return null;
    const resolved = selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)
      ? selectedNodeId.slice(CANDIDATE_TARGET_PREFIX.length)
      : selectedNodeId;
    return graph.nodes.find((n) => n.nodeId === resolved) ?? null;
  }, [graph, selectedNodeId]);

  const isCandidateSliceSelected =
    layout?.kind === "candidate" && selectedNodeId === CANDIDATE_SLICE_ID;

  return {
    selectedNodeId,
    setSelectedNodeId,
    selectedCanonicalNode,
    isCandidateSliceSelected,
  };
}
