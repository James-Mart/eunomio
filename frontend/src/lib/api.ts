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
};

export type GraphEdge = { from: string; to: string };

export type Graph = { nodes: GraphNode[]; edges: GraphEdge[] };

export type Edge = {
  targetNodeId: string;
  parentNodeId: string | null;
  diff: string;
};

export type Diff = {
  fromTree: string;
  toTree: string;
  diff: string;
};

export type BranchResult = { branchName: string; commitSha: string };

export type SubagentSettings = {
  overrideModel: boolean;
  model: string;
};

export type HumanInTheLoopSettings = {
  afterSurvey: boolean;
  afterPlanning: boolean;
  afterConstruct: boolean;
};

export type CoordinatorSettings = {
  model: string;
  humanInTheLoop: HumanInTheLoopSettings;
};

export interface PartitionSettings {
  coordinator: CoordinatorSettings;
  surveyor: SubagentSettings;
  planner: SubagentSettings;
  constructor: SubagentSettings;
}

export interface PartitionSettingsPatch {
  coordinator?: CoordinatorSettings;
  surveyor?: SubagentSettings;
  planner?: SubagentSettings;
  constructor?: SubagentSettings;
}

export type PartitionStrategy = "semantic" | "vertical" | "horizontal";
export type StrategyOverride = PartitionStrategy | "auto";

export type PhaseName = "survey" | "plan" | "construct";
export type PhaseState = "running" | "awaiting_review" | "error";

export type RunKind = "survey" | "plan" | "construct";
export type RunStatus = "running" | "finished" | "error" | "cancelled";

export type ChangeSurveyTheme = {
  id: string;
  title: string;
  description: string;
};

export type ChangeSurvey = {
  summary: string;
  themes: ChangeSurveyTheme[];
};

export type PlanEdge = {
  id: string;
  title: string;
  description: string;
};

export type Plan = {
  strategy: PartitionStrategy;
  strategyRationale: string;
  edges: PlanEdge[];
};

export type Partition = {
  id: number;
  sessionId: string;
  targetNodeId: string;
  strategy: PartitionStrategy | null;
  changeSurvey: ChangeSurvey | null;
  plan: Plan | null;
  phase: PhaseName;
  phaseState: PhaseState;
  candidateSliceTreeSha: string | null;
  candidateSliceCommitSha: string | null;
  createdAt: number;
};

export type Run = {
  id: number;
  partitionId: number;
  kind: RunKind;
  status: RunStatus;
  result: unknown;
  errorMessage: string | null;
  startedAt: number;
  finishedAt: number | null;
};

export type StartRunRequest = {
  kind: RunKind;
  parentRunId?: number;
  userFeedback?: string;
  strategyOverride?: PartitionStrategy;
};

export type CursorModel = { id: string };
export type CursorModels = { models: CursorModel[] };

export type RepoInfo = { currentBranch?: string };

export type TunnelState = "idle" | "running" | "error";

export type TunnelStatus = {
  state: TunnelState;
  tokenRequired: boolean;
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
  getDiff: (sessionId: string, fromTree: string, toTree: string) =>
    request<Diff>(
      "GET",
      `/sessions/${sessionId}/diff?fromTree=${encodeURIComponent(fromTree)}&toTree=${encodeURIComponent(toTree)}`,
    ),
  renameNode: (sessionId: string, nodeId: string, title: string) =>
    request<GraphNode>("PATCH", `/sessions/${sessionId}/nodes/${nodeId}`, { title }),
  branchFromNode: (sessionId: string, nodeId: string, branchName: string, force = false) =>
    request<BranchResult>("POST", `/sessions/${sessionId}/nodes/${nodeId}/branch`, {
      branchName,
      force,
    }),
  deleteSession: (id: string) => request<void>("DELETE", `/sessions/${id}`),
  getPartitionSettings: () => request<PartitionSettings>("GET", `/partition-settings`),
  updatePartitionSettings: (patch: PartitionSettingsPatch) =>
    request<PartitionSettings>("PATCH", `/partition-settings`, patch),
  listCursorModels: () => request<CursorModels>("GET", "/cursor-models"),
  getRepoInfo: () => request<RepoInfo>("GET", "/repo"),
  beginPartition: (sessionId: string, targetNodeId: string) =>
    request<Partition>(
      "POST",
      `/sessions/${sessionId}/edges/${targetNodeId}/partition`,
    ),
  listPartitions: (sessionId: string, targetNodeId?: string) =>
    request<Partition[]>(
      "GET",
      targetNodeId
        ? `/sessions/${sessionId}/partitions?targetNodeId=${encodeURIComponent(targetNodeId)}`
        : `/sessions/${sessionId}/partitions`,
    ),
  getPartition: (partitionId: number) =>
    request<Partition>("GET", `/partitions/${partitionId}`),
  listRuns: (partitionId: number) =>
    request<Run[]>("GET", `/partitions/${partitionId}/runs`),
  startRun: (partitionId: number, body: StartRunRequest) =>
    request<Run>("POST", `/partitions/${partitionId}/runs`, body),
  acceptSurvey: (partitionId: number, runId: number) =>
    request<Partition>("POST", `/partitions/${partitionId}/survey/accept`, { runId }),
  acceptPlan: (partitionId: number, runId: number) =>
    request<Partition>("POST", `/partitions/${partitionId}/plan/accept`, { runId }),
  acceptConstruct: (partitionId: number) =>
    request<void>("POST", `/partitions/${partitionId}/construct/accept`),
  abandonPartition: (partitionId: number) =>
    request<void>("POST", `/partitions/${partitionId}/abandon`),
  getTunnel: () => request<TunnelStatus>("GET", "/tunnel"),
  startTunnel: () => request<TunnelStatus>("POST", "/tunnel"),
  stopTunnel: () => request<void>("DELETE", "/tunnel"),
};
