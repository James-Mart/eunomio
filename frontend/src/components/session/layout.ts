import type { Edge as FlowEdge, Node } from "@xyflow/react";

import type {
  Graph,
  GraphNode,
  Partition,
  PhaseName,
  PhaseState,
} from "@/lib/api";
import type { NodeCardData } from "@/components/NodeCard";

export const NODE_X = 0;
export const NODE_Y_STEP = 140;
export const CANDIDATE_X_OFFSET = 260;

export const CANDIDATE_SLICE_ID = "__candidate_slice__";
export const CANDIDATE_TARGET_PREFIX = "__candidate_target__";

export type Chain = {
  ordered: GraphNode[];
  positionByNodeId: Map<string, string>;
};

export type PhaseStatus = { phase: PhaseName; phaseState: PhaseState };

export type CanonicalLayout = {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
};

export type CandidateLayout = {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
  candidateSliceNode: GraphNode;
  renamedTargetNode: GraphNode;
};

export type SessionLayout =
  | (CanonicalLayout & { kind: "canonical" })
  | (CandidateLayout & { kind: "candidate" });

export function computeChain(graph: Graph): Chain {
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

export function canonicalLayout(
  chain: Chain,
  phaseStatusByNode: Map<string, PhaseStatus>,
): CanonicalLayout {
  const total = chain.ordered.length;
  const nodes: Node<NodeCardData>[] = chain.ordered.map((n, idx) => ({
    id: n.nodeId,
    type: "eunomia",
    position: { x: NODE_X, y: (total - 1 - idx) * NODE_Y_STEP },
    data: {
      positionLabel: chain.positionByNodeId.get(n.nodeId) ?? "",
      phaseStatus: phaseStatusByNode.get(n.nodeId) ?? null,
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

export function candidateLayout(
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
    !partition.plan ||
    partition.plan.outcome !== "split"
  ) {
    return null;
  }
  const parentPosition = chain.positionByNodeId.get(parent.nodeId) ?? "?";
  const planEdges = partition.plan.edges;

  const candidateSlice: GraphNode = {
    nodeId: CANDIDATE_SLICE_ID,
    parentNodeId: parent.nodeId,
    treeSha: partition.candidateSliceTreeSha,
    commitSha: partition.candidateSliceCommitSha,
    title: planEdges[0].title,
    description: planEdges[0].description,
  };
  const renamedTarget: GraphNode = {
    ...target,
    nodeId: CANDIDATE_TARGET_PREFIX + target.nodeId,
    parentNodeId: CANDIDATE_SLICE_ID,
    title: planEdges[1].title,
    description: planEdges[1].description,
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
      data: { positionLabel: parentLabel },
    },
    {
      id: candidateSlice.nodeId,
      type: "eunomia",
      position: { x: NODE_X + CANDIDATE_X_OFFSET, y: NODE_Y_STEP },
      data: { positionLabel: sliceLabel },
    },
    {
      id: renamedTarget.nodeId,
      type: "eunomia",
      position: { x: NODE_X, y: 0 },
      data: { positionLabel: targetLabel },
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

export function findLeafNodeId(graph: Graph): string | null {
  const parents = new Set<string>();
  for (const n of graph.nodes) {
    if (n.parentNodeId) parents.add(n.parentNodeId);
  }
  for (const n of graph.nodes) {
    if (!parents.has(n.nodeId)) return n.nodeId;
  }
  return graph.nodes[graph.nodes.length - 1]?.nodeId ?? null;
}

export function willRenderCandidateLayout(p: Partition): boolean {
  return (
    p.phase === "construct" &&
    p.phaseState === "awaiting_review" &&
    !!p.candidateSliceTreeSha &&
    !!p.candidateSliceCommitSha &&
    !!p.plan
  );
}

export function phaseLabel(p: Partition): string {
  if (p.phaseState === "error") return `${p.phase} error`;
  if (p.phaseState === "awaiting_review") return `${p.phase} review`;
  if (p.phase === "survey") return "surveying…";
  if (p.phase === "plan") return "planning…";
  return "constructing…";
}
