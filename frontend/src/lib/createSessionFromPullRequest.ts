import { api, type ResolvedPullRequest, type Session } from "./api";

export type CreateSessionPhase = "resolving" | "fetching" | "creating";

export async function createSessionFromResolved(
  resolved: ResolvedPullRequest,
  onPhase?: (phase: Exclude<CreateSessionPhase, "resolving">) => void,
): Promise<Session> {
  onPhase?.("fetching");
  const session = await api.createSession(
    resolved.remoteUrl,
    resolved.baseRef,
    resolved.sourceRef,
  );
  onPhase?.("creating");
  return session;
}

export async function createSessionFromPullRequest(
  pullRequestUrl: string,
  onPhase?: (phase: CreateSessionPhase) => void,
): Promise<Session> {
  onPhase?.("resolving");
  const resolved = await api.resolvePullRequest(pullRequestUrl);
  return createSessionFromResolved(resolved, onPhase);
}

export function createSessionPhaseLabel(phase: CreateSessionPhase): string {
  switch (phase) {
    case "resolving":
      return "Resolving PR…";
    case "fetching":
      return "Fetching…";
    case "creating":
      return "Creating…";
  }
}
