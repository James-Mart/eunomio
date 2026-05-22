import {
  FileDirectoryIcon,
  GlobeIcon,
  LogoGistIcon,
  MarkGithubIcon,
} from "@primer/octicons-react";

import { remoteRepoHost } from "@/lib/remoteRepoHost";

import {
  AzureDevOpsIcon,
  BitbucketIcon,
  GitLabIcon,
} from "./repoProviderIcons";

function RemoteRepoIcon({
  remoteUrl,
  className,
}: {
  remoteUrl?: string;
  className?: string;
}) {
  switch (remoteUrl ? remoteRepoHost(remoteUrl) : "unknown") {
    case "github":
      return <MarkGithubIcon className={className} aria-hidden="true" />;
    case "gist":
      return <LogoGistIcon className={className} aria-hidden="true" />;
    case "gitlab":
      return <GitLabIcon className={className} />;
    case "bitbucket":
      return <BitbucketIcon className={className} />;
    case "azure":
      return <AzureDevOpsIcon className={className} />;
    default:
      return <GlobeIcon className={className} aria-hidden="true" />;
  }
}

export function RepoKindIcon({
  isLocal,
  remoteUrl,
  className,
}: {
  isLocal: boolean;
  remoteUrl?: string;
  className?: string;
}) {
  if (isLocal) {
    return <FileDirectoryIcon className={className} aria-hidden="true" />;
  }
  return <RemoteRepoIcon remoteUrl={remoteUrl} className={className} />;
}
