/* SPDX-License-Identifier: Apache-2.0 */

import type { Edge as FlowEdge, Node } from "@xyflow/react";

import type {
  Graph,
  GraphNode,
  Partition,
} from "@/lib/api";
import type { NodeCardData, NodePartitionGlance } from "@/components/NodeCard";

export const NODE_X = 0;
export const NODE_Y_STEP = 140;

export const CANDIDATE_SLICE_ID = "__candidate_slice__";
export const CANDIDATE_TARGET_PREFIX = "__candidate_target__";

export type Chain = {
  ordered: GraphNode[];
  positionByNodeId: Map<string, string>;
};

export function partitionGlanceByNode(
  partitions: Partition[],
): Map<string, NodePartitionGlance> {
  const byTarget = new Map<string, Partition[]>();
  for (const p of partitions) {
    const group = byTarget.get(p.targetNodeId);
    if (group) group.push(p);
    else byTarget.set(p.targetNodeId, [p]);
  }
  const out = new Map<string, NodePartitionGlance>();
  for (const [targetNodeId, siblings] of byTarget) {
    const blocked = siblings.some(
      (p) =>
        p.phaseState === "awaiting_review" || p.phaseState === "error",
    );
    out.set(targetNodeId, {
      count: siblings.length,
      status: blocked ? "blocked" : "running",
    });
  }
  return out;
}

export type CanonicalLayout = {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
};

export type CandidateLayoutBase = {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
  rootNodeId: string;
  parentNode: GraphNode;
  targetNode: GraphNode;
};

export type CandidateLayout =
  | (CandidateLayoutBase & { stage: "pending" })
  | (CandidateLayoutBase & {
      stage: "proposed";
      candidateSliceNode: GraphNode;
      renamedTargetNode: GraphNode;
    });

export type OriginalLayout = {
  nodes: Node<NodeCardData>[];
  edges: FlowEdge[];
  baseNode: GraphNode;
  finalNode: GraphNode;
};

export type SessionLayout =
  | (CanonicalLayout & { kind: "canonical" })
  | (OriginalLayout & { kind: "original" })
  | (CandidateLayout & { kind: "candidate" });

export type View =
  | { kind: "canonical" }
  | { kind: "original" }
  | { kind: "candidate"; partitionId: string };

export function originalLayout(chain: Chain): OriginalLayout | null {
  const baseNode = chain.ordered[0];
  const finalNode = chain.ordered[chain.ordered.length - 1];
  if (!baseNode || !finalNode || baseNode.nodeId === finalNode.nodeId) {
    return null;
  }
  const nodes: Node<NodeCardData>[] = [
    {
      id: finalNode.nodeId,
      type: "eunomio",
      position: { x: NODE_X, y: 0 },
      data: { positionLabel: "final" },
    },
    {
      id: baseNode.nodeId,
      type: "eunomio",
      position: { x: NODE_X, y: NODE_Y_STEP },
      data: { positionLabel: "base" },
    },
  ];
  const edges: FlowEdge[] = [
    {
      id: `${baseNode.nodeId}->${finalNode.nodeId}`,
      source: baseNode.nodeId,
      target: finalNode.nodeId,
    },
  ];
  return { nodes, edges, baseNode, finalNode };
}

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
  partitionGlanceByNodeId: Map<string, NodePartitionGlance>,
): CanonicalLayout {
  const total = chain.ordered.length;
  const nodes: Node<NodeCardData>[] = chain.ordered.map((n, idx) => ({
    id: n.nodeId,
    type: "eunomio",
    position: { x: NODE_X, y: (total - 1 - idx) * NODE_Y_STEP },
    data: {
      positionLabel: chain.positionByNodeId.get(n.nodeId) ?? "",
      partitionGlance: partitionGlanceByNodeId.get(n.nodeId) ?? null,
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

function resolveCandidateEdge(
  chain: Chain,
  partition: Partition,
  graph: Graph,
): { targetIdx: number; target: GraphNode; parent: GraphNode } | null {
  const targetIdx = chain.ordered.findIndex(
    (n) => n.nodeId === partition.targetNodeId,
  );
  if (targetIdx < 0) return null;
  const target = chain.ordered[targetIdx];
  if (target.parentNodeId === null) return null;
  const parent = graph.nodes.find((n) => n.nodeId === target.parentNodeId);
  if (!parent) return null;
  return { targetIdx, target, parent };
}

export function candidateLayout(
  chain: Chain,
  partition: Partition,
  graph: Graph,
): CandidateLayout | null {
  const edge = resolveCandidateEdge(chain, partition, graph);
  if (!edge) return null;
  const { targetIdx, target, parent } = edge;
  const rootNodeId = parent.nodeId;

  if (
    isProposedCandidateStage(partition) &&
    partition.plan?.outcome === "split"
  ) {
    const planEdges = partition.plan.edges;
    const candidateSlice: GraphNode = {
      nodeId: CANDIDATE_SLICE_ID,
      parentNodeId: parent.nodeId,
      treeSha: partition.candidateSliceTreeSha!,
      commitSha: partition.candidateSliceCommitSha!,
      title: planEdges[0].title,
      description: planEdges[0].description,
      strategy: null,
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
    const parentLabel = chain.positionByNodeId.get(parent.nodeId) ?? "?";

    return {
      stage: "proposed",
      rootNodeId,
      parentNode: parent,
      targetNode: target,
      nodes: [
        {
          id: parent.nodeId,
          type: "eunomio",
          position: { x: NODE_X, y: 2 * NODE_Y_STEP },
          data: { positionLabel: parentLabel },
        },
        {
          id: candidateSlice.nodeId,
          type: "eunomio",
          position: { x: NODE_X, y: NODE_Y_STEP },
          data: { positionLabel: sliceLabel },
        },
        {
          id: renamedTarget.nodeId,
          type: "eunomio",
          position: { x: NODE_X, y: 0 },
          data: { positionLabel: targetLabel },
        },
      ],
      edges: [
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
      ],
      candidateSliceNode: candidateSlice,
      renamedTargetNode: renamedTarget,
    };
  }

  const parentLabel = chain.positionByNodeId.get(parent.nodeId) ?? "?";
  const targetLabel = chain.positionByNodeId.get(target.nodeId) ?? "?";

  return {
    stage: "pending",
    rootNodeId,
    parentNode: parent,
    targetNode: target,
    nodes: [
      {
        id: target.nodeId,
        type: "eunomio",
        position: { x: NODE_X, y: 0 },
        data: { positionLabel: targetLabel },
      },
      {
        id: parent.nodeId,
        type: "eunomio",
        position: { x: NODE_X, y: 2 * NODE_Y_STEP },
        data: { positionLabel: parentLabel },
      },
    ],
    edges: [
      {
        id: `${parent.nodeId}->${target.nodeId}`,
        source: parent.nodeId,
        target: target.nodeId,
      },
    ],
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

export function isProposedCandidateStage(p: Partition): boolean {
  return (
    p.phase === "construct" &&
    p.phaseState === "awaiting_review" &&
    !!p.candidateSliceTreeSha &&
    !!p.candidateSliceCommitSha &&
    p.plan?.outcome === "split"
  );
}

export function candidateLayoutFingerprint(layout: CandidateLayout): string {
  return `${layout.stage}:${layout.nodes.map((n) => n.id).join(",")}`;
}

export function partitionSiblingNumbers(
  partitions: Partition[],
): Map<string, number> {
  const byTarget = new Map<string, Partition[]>();
  for (const p of partitions) {
    const group = byTarget.get(p.targetNodeId);
    if (group) group.push(p);
    else byTarget.set(p.targetNodeId, [p]);
  }
  const out = new Map<string, number>();
  for (const siblings of byTarget.values()) {
    siblings.sort(
      (a, b) => a.createdAt - b.createdAt || a.id.localeCompare(b.id),
    );
    siblings.forEach((p, i) => out.set(p.id, i + 1));
  }
  return out;
}

export function comparePartitionsForView(
  a: Partition,
  b: Partition,
  chain: Chain,
  siblingNumbers: Map<string, number>,
): number {
  const idxA = chain.ordered.findIndex((n) => n.nodeId === a.targetNodeId);
  const idxB = chain.ordered.findIndex((n) => n.nodeId === b.targetNodeId);
  if (idxA !== idxB) return idxA - idxB;
  return (siblingNumbers.get(a.id) ?? 1) - (siblingNumbers.get(b.id) ?? 1);
}

export function partitionViewLabel(
  p: Partition,
  chain: Chain,
  siblingNumber: number,
): string {
  const positionLabel = chain.positionByNodeId.get(p.targetNodeId) ?? "?";
  return `Partitioning ${positionLabel} - #${siblingNumber}`;
}
