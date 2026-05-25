/* SPDX-License-Identifier: Apache-2.0 */

export type Session = {
  id: string;
  normalizedRemote: string;
  literalRemote: string;
  isLocal: boolean;
  repoOwner?: string;
  repoName: string;
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
  description: string;
  strategy: PartitionStrategy | null;
};

export type GraphEdge = { from: string; to: string };

export type Graph = { nodes: GraphNode[]; edges: GraphEdge[] };

export type LineRanges = {
  line: number;
  spans: [number, number][];
};

export type FileLineRanges = {
  path: string;
  lines: LineRanges[];
};

export type SynthesizedRanges = {
  child: FileLineRanges[];
  parent: FileLineRanges[];
};

export type FileBlob = {
  oldPath: string | null;
  newPath: string | null;
  oldContent: string | null;
  newContent: string | null;
};

export type Edge = {
  targetNodeId: string;
  parentNodeId: string | null;
  diff: string;
  files: FileBlob[];
  synthesized: SynthesizedRanges;
};

export type EdgeViewedFiles = {
  paths: string[];
};

export type Diff = {
  fromTree: string;
  toTree: string;
  diff: string;
  files: FileBlob[];
  synthesized: SynthesizedRanges;
};

export type BranchResult = { branchName: string; commitSha: string };

export type GeneralSettings = {
  transcriptsEnabled: boolean;
};

export type SubagentSettings = {
  overrideModel: boolean;
  model: string;
};

export type HumanInTheLoopSettings = {
  afterSurvey: boolean;
  afterPlanning: boolean;
  afterConstruct: boolean;
  afterIndivisible: boolean;
};

export type IterationLimit =
  | { kind: "count"; count: number }
  | { kind: "auto" };

export type CoordinatorSettings = {
  model: string;
  humanInTheLoop: HumanInTheLoopSettings;
  maxIterations: IterationLimit;
  surveyorEnabled: boolean;
};

export interface PartitionSettings {
  general: GeneralSettings;
  coordinator: CoordinatorSettings;
  surveyor: SubagentSettings;
  planner: SubagentSettings;
  constructor: SubagentSettings;
}

export interface PartitionSettingsPatch {
  general?: GeneralSettings;
  coordinator?: CoordinatorSettings;
  surveyor?: SubagentSettings;
  planner?: SubagentSettings;
  constructor?: SubagentSettings;
}

export type PartitionStrategy = "synthetic" | "vertical" | "horizontal";
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

export type Plan =
  | {
      outcome: "split";
      strategy: PartitionStrategy;
      strategyRationale: string;
      edges: PlanEdge[];
    }
  | {
      outcome: "indivisible";
      rationale: string;
    };

export type Principal = {
  userId: string;
  orgId: string;
  role: string;
  username: string;
};

export type AuthSetup = {
  suggestedUsername: string;
  hasEnvKey: boolean;
};

export type Partition = {
  id: string;
  sessionId: string;
  targetNodeId: string;
  strategy: PartitionStrategy | null;
  changeSurvey: ChangeSurvey | null;
  plan: Plan | null;
  phase: PhaseName;
  phaseState: PhaseState;
  candidateSliceTreeSha: string | null;
  candidateSliceCommitSha: string | null;
  remainingDepth: number | null;
  createdAt: number;
};

export type Run = {
  id: string;
  partitionId: string;
  kind: RunKind;
  status: RunStatus;
  result: unknown;
  errorMessage: string | null;
  startedAt: number;
  finishedAt: number | null;
};

export type Transcript = {
  runId: string;
  kind: RunKind;
  prompt: string | null;
  transcriptText: string | null;
  rawResult: string | null;
  parsedResult: unknown;
  errorMessage: string | null;
};

export type StartRunRequest = {
  kind: RunKind;
  parentRunId?: string;
  userFeedback?: string;
  strategyOverride?: PartitionStrategy;
};

export type CursorModel = { id: string };
export type CursorModels = { models: CursorModel[] };

export type RepoHints = {
  suggestedRemoteUrl?: string;
  suggestedSourceRef?: string;
  suggestedBaseRef?: string;
};

export type ResolvedPullRequest = {
  remoteUrl: string;
  sourceRef: string;
  baseRef: string;
};

export type LaunchPullRequest = {
  pullRequestUrl: string | null;
};

export type TunnelState = "idle" | "running" | "error";

export type TunnelStatus = {
  enabled: boolean;
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
  const headers: Record<string, string> = { "X-Eunomio-Request": "1" };
  if (body !== undefined) headers["content-type"] = "application/json";
  const init: RequestInit = {
    method,
    credentials: "include",
    headers,
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

export type LoginRequest = {
  username: string;
  cursorApiKey?: string;
  useEnvKey?: boolean;
};

export type PatchCredentialsRequest = {
  cursorApiKey: string;
};

export const api = {
  getMe: () => request<Principal>("GET", "/me"),
  getAuthSetup: () => request<AuthSetup>("GET", "/auth/setup"),
  consumeLaunchPullRequest: () =>
    request<LaunchPullRequest>("GET", "/launch/pull-request"),
  login: (body: LoginRequest) => request<{ ok: true }>("POST", "/auth/login", body),
  logout: () => request<{ ok: true }>("POST", "/auth/logout"),
  patchCredentials: (body: PatchCredentialsRequest) =>
    request<{ ok: true }>("PATCH", "/auth/credentials", body),
  createSession: (remoteUrl: string, baseRef: string, sourceRef: string) =>
    request<Session>("POST", "/sessions", { remoteUrl, baseRef, sourceRef }),
  validateSession: (remoteUrl: string, baseRef: string, sourceRef: string) =>
    request<void>("POST", "/sessions/validate", { remoteUrl, baseRef, sourceRef }),
  getSession: (id: string) => request<Session>("GET", `/sessions/${id}`),
  listSessions: () => request<Session[]>("GET", "/sessions"),
  getGraph: (id: string) => request<Graph>("GET", `/sessions/${id}/graph`),
  getEdge: (sessionId: string, targetNodeId: string) =>
    request<Edge>("GET", `/sessions/${sessionId}/edges/${targetNodeId}`),
  getEdgeViewedFiles: (sessionId: string, targetNodeId: string) =>
    request<EdgeViewedFiles>(
      "GET",
      `/sessions/${sessionId}/edges/${targetNodeId}/viewed`,
    ),
  setEdgeFileViewed: (
    sessionId: string,
    targetNodeId: string,
    filePath: string,
    viewed: boolean,
  ) =>
    request<void>(
      "PUT",
      `/sessions/${sessionId}/edges/${targetNodeId}/viewed/${encodeURIComponent(filePath)}`,
      { viewed },
    ),
  getDiff: (
    sessionId: string,
    fromTree: string,
    toTree: string,
    beforeRef?: string,
    afterRef?: string,
  ) => {
    const params = new URLSearchParams({ fromTree, toTree });
    if (beforeRef) params.set("beforeRef", beforeRef);
    if (afterRef) params.set("afterRef", afterRef);
    return request<Diff>(
      "GET",
      `/sessions/${sessionId}/diff?${params.toString()}`,
    );
  },
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
  getRepoHints: () => request<RepoHints>("GET", "/repo"),
  resolvePullRequest: (pullRequestUrl: string) =>
    request<ResolvedPullRequest>("POST", "/repo/resolve-pull-request", {
      pullRequestUrl,
    }),
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
  getPartition: (partitionId: string) =>
    request<Partition>("GET", `/partitions/${partitionId}`),
  listRuns: (partitionId: string) =>
    request<Run[]>("GET", `/partitions/${partitionId}/runs`),
  startRun: (partitionId: string, body: StartRunRequest) =>
    request<Run>("POST", `/partitions/${partitionId}/runs`, body),
  cancelRun: (partitionId: string, runId: string) =>
    request<void>("DELETE", `/partitions/${partitionId}/runs/${runId}`),
  getRunTranscript: (partitionId: string, runId: string) =>
    request<Transcript>(
      "GET",
      `/partitions/${partitionId}/runs/${runId}/transcript`,
    ),
  acceptSurvey: (partitionId: string, runId: string) =>
    request<Partition>("POST", `/partitions/${partitionId}/survey/accept`, { runId }),
  acceptPlan: (partitionId: string, runId: string) =>
    request<Partition>("POST", `/partitions/${partitionId}/plan/accept`, { runId }),
  acceptConstruct: (partitionId: string) =>
    request<void>("POST", `/partitions/${partitionId}/construct/accept`),
  abandonPartition: (partitionId: string) =>
    request<void>("POST", `/partitions/${partitionId}/abandon`),
  getTunnel: () => request<TunnelStatus>("GET", "/tunnel"),
  startTunnel: () => request<TunnelStatus>("POST", "/tunnel"),
  stopTunnel: () => request<void>("DELETE", "/tunnel"),
};
