/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";

import { useAuth } from "@/components/AuthProvider";
import {
  createSessionFromPullRequest,
  createSessionPhaseLabel,
  type CreateSessionPhase,
} from "@/lib/createSessionFromPullRequest";
import { formatError } from "@/lib/errors";

export default function LaunchPullRequestBootstrap() {
  const { pendingLaunchPullRequestUrl, clearPendingLaunchPullRequest } = useAuth();
  const navigate = useNavigate();
  const [phase, setPhase] = useState<CreateSessionPhase>("resolving");
  const runningRef = useRef(false);

  useEffect(() => {
    if (!pendingLaunchPullRequestUrl || runningRef.current) return;

    runningRef.current = true;
    const url = pendingLaunchPullRequestUrl;

    void (async () => {
      try {
        const session = await createSessionFromPullRequest(url, setPhase);
        clearPendingLaunchPullRequest();
        navigate(`/sessions/${session.id}`, { replace: true });
      } catch (e) {
        clearPendingLaunchPullRequest();
        toast.error(formatError(e, "Failed to create session"));
      } finally {
        runningRef.current = false;
      }
    })();
  }, [pendingLaunchPullRequestUrl, clearPendingLaunchPullRequest, navigate]);

  if (!pendingLaunchPullRequestUrl) return null;

  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-3 bg-background">
      <p className="text-sm text-muted-foreground">{createSessionPhaseLabel(phase)}</p>
    </div>
  );
}
