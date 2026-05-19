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

export type SurveyorSettings = { model: string };

export type HumanInTheLoopSettings = {
  afterSurvey: boolean;
  afterPlanning: boolean;
};

export type CoordinatorSettings = {
  humanInTheLoop: HumanInTheLoopSettings;
};

export type PartitionSettings = {
  coordinator: CoordinatorSettings;
  surveyor: SurveyorSettings;
  planner: unknown;
  constructor: unknown;
};

export type PartitionSettingsPatch = Partial<PartitionSettings>;

export type PartitionStrategy = "semantic" | "vertical" | "horizontal";

export type MockPartition = {
  sessionId: string;
  targetNodeId: string;
  strategy: PartitionStrategy;
  userConcern: string | null;
  startedAt: number;
};

export type CursorModel = { id: string };
export type CursorModels = { models: CursorModel[] };

export type TunnelState = "idle" | "running" | "error";

export type TunnelStatus = {
  state: TunnelState;
  url?: string;
  token?: string;
  startedAt?: number;
  errorMessage?: string;
};

export class ApiError extends Error {
  status: number;
  body: unknown;
  code?: string;
  constructor(status: number, body: unknown, message: string, code?: string) {
    super(message);
    this.status = status;
    this.body = body;
    this.code = code;
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
    const code = (json && typeof json === "object" && "code" in json && typeof (json as { code: unknown }).code === "string")
      ? (json as { code: string }).code
      : undefined;
    if (code && resp.status >= 500) {
      const { registerSystemError } = await import("./systemErrors");
      registerSystemError(code, msg);
    }
    throw new ApiError(resp.status, json, msg, code);
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
  getPartitionSettings: (sessionId: string) =>
    request<PartitionSettings>("GET", `/sessions/${sessionId}/partition-settings`),
  updatePartitionSettings: (sessionId: string, patch: PartitionSettingsPatch) =>
    request<PartitionSettings>("PATCH", `/sessions/${sessionId}/partition-settings`, patch),
  listCursorModels: () => request<CursorModels>("GET", "/cursor-models"),
  startMockPartition: (
    sessionId: string,
    targetNodeId: string,
    body: { strategy: PartitionStrategy; userConcern?: string },
  ) =>
    request<MockPartition>(
      "POST",
      `/sessions/${sessionId}/edges/${targetNodeId}/mock-partition`,
      body,
    ),
  continueMockPartition: (sessionId: string, targetNodeId: string) =>
    request<void>(
      "POST",
      `/sessions/${sessionId}/edges/${targetNodeId}/mock-partition/continue`,
      {},
    ),
  rerunMockPartition: (
    sessionId: string,
    targetNodeId: string,
    body: { userFeedback?: string } = {},
  ) =>
    request<void>(
      "POST",
      `/sessions/${sessionId}/edges/${targetNodeId}/mock-partition/rerun`,
      body,
    ),
  abandonMockPartition: (sessionId: string, targetNodeId: string) =>
    request<void>(
      "POST",
      `/sessions/${sessionId}/edges/${targetNodeId}/mock-partition/abandon`,
      {},
    ),
  getTunnel: () => request<TunnelStatus>("GET", "/tunnel"),
  startTunnel: () => request<TunnelStatus>("POST", "/tunnel"),
  stopTunnel: () => request<void>("DELETE", "/tunnel"),
};
