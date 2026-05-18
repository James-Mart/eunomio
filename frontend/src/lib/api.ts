export type Session = {
  id: string;
  baseRef: string;
  sourceRef: string;
  baseNodeId: string;
  createdAt: number;
};

export type GraphNode = {
  nodeId: string;
  parentNodeId: string | null;
  treeSha: string;
  commitSha: string;
  title: string;
  isFavorite: boolean;
};

export type GraphEdge = { from: string; to: string };

export type Graph = { nodes: GraphNode[]; edges: GraphEdge[] };

export type Edge = {
  targetNodeId: string;
  parentNodeId: string | null;
  diff: string;
};

export type BranchResult = { branchName: string; commitSha: string };

export class ApiError extends Error {
  status: number;
  body: unknown;
  constructor(status: number, body: unknown, message: string) {
    super(message);
    this.status = status;
    this.body = body;
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const init: RequestInit = {
    method,
    headers: body !== undefined ? { "content-type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  };
  const resp = await fetch(`/api${path}`, init);
  if (resp.status === 204) return undefined as T;
  const text = await resp.text();
  const json = text ? JSON.parse(text) : null;
  if (!resp.ok) {
    const msg = (json && typeof json === "object" && "error" in json && typeof (json as { error: unknown }).error === "string")
      ? (json as { error: string }).error
      : `HTTP ${resp.status}`;
    throw new ApiError(resp.status, json, msg);
  }
  return json as T;
}

export const api = {
  createSession: (baseRef: string, sourceRef: string) =>
    request<Session>("POST", "/sessions", { baseRef, sourceRef }),
  getSession: (id: string) => request<Session>("GET", `/sessions/${id}`),
  listSessions: () => request<Session[]>("GET", "/sessions"),
  getGraph: (id: string) => request<Graph>("GET", `/sessions/${id}/graph`),
  getEdge: (sessionId: string, targetNodeId: string) =>
    request<Edge>("GET", `/sessions/${sessionId}/edges/${targetNodeId}`),
  renameNode: (sessionId: string, nodeId: string, title: string) =>
    request<GraphNode>("PATCH", `/sessions/${sessionId}/nodes/${nodeId}`, { title }),
  branchFromNode: (sessionId: string, nodeId: string, branchName: string, force = false) =>
    request<BranchResult>("POST", `/sessions/${sessionId}/nodes/${nodeId}/branch`, {
      branchName,
      force,
    }),
  deleteSession: (id: string) => request<void>("DELETE", `/sessions/${id}`),
};
