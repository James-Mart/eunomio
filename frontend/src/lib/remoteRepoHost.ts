/* SPDX-License-Identifier: Apache-2.0 */

export type RemoteRepoHost =
  | "github"
  | "gist"
  | "gitlab"
  | "bitbucket"
  | "azure"
  | "unknown";

function parseHostname(url: string): string | null {
  const trimmed = url.trim();
  if (!trimmed) return null;

  if (trimmed.includes("://")) {
    try {
      return new URL(trimmed).hostname.toLowerCase();
    } catch {
      return null;
    }
  }

  if (trimmed.includes("@")) {
    const afterAt = trimmed.split("@").pop();
    if (!afterAt) return null;
    return afterAt.split(":")[0]?.split("/")[0]?.toLowerCase() ?? null;
  }

  return null;
}

function hostIs(host: string, domain: string): boolean {
  return host === domain || host.endsWith(`.${domain}`);
}

export function remoteRepoHost(url: string): RemoteRepoHost {
  const host = parseHostname(url);
  if (!host) return "unknown";

  if (hostIs(host, "gist.github.com")) return "gist";
  if (hostIs(host, "github.com")) return "github";
  if (hostIs(host, "gitlab.com") || host.includes("gitlab")) return "gitlab";
  if (hostIs(host, "bitbucket.org") || host.includes("bitbucket")) return "bitbucket";
  if (hostIs(host, "dev.azure.com") || host.includes("visualstudio.com")) {
    return "azure";
  }

  return "unknown";
}

const GITHUB_PULL_URL_RE =
  /^https:\/\/github\.com\/[^/]+\/[^/]+?(?:\.git)?\/pull\/\d+\/?$/;

export function isGithubPullRequestUrl(url: string): boolean {
  return GITHUB_PULL_URL_RE.test(url.trim());
}
