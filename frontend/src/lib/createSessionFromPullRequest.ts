/* SPDX-License-Identifier: Apache-2.0 */

import { api, type ResolvedPullRequest, type Session } from "./api";
import { normalizeGithubPullRequestUrl } from "./remoteRepoHost";

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
  const normalized = normalizeGithubPullRequestUrl(pullRequestUrl);
  if (!normalized) {
    throw new Error(
      "expected a GitHub pull request URL (https://github.com/org/repo/pull/N)",
    );
  }
  onPhase?.("resolving");
  const resolved = await api.resolvePullRequest(normalized);
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
