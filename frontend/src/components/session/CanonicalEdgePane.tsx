/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, useState } from "react";

import EdgePane from "@/components/EdgePane";
import { ShavingTimelineBar } from "@/components/session/ShavingTimelineBar";
import { ApiError, api, type GraphNode, type ShavingTrack } from "@/lib/api";
import { useEdgeFileViewed } from "@/lib/useEdgeFileViewed";

type Props = {
  sessionId: string;
  node: GraphNode;
};

export function CanonicalEdgePane({ sessionId, node }: Props) {
  const targetNodeId = node.nodeId;
  const { viewedPaths, toggleViewed } = useEdgeFileViewed(sessionId, targetNodeId);
  const [track, setTrack] = useState<ShavingTrack | null>(null);
  const [stepIndex, setStepIndex] = useState(0);
  const titleHeader = <NodeDiffTitleBar title={node.title} />;

  useEffect(() => {
    let cancelled = false;
    setTrack(null);
    if (!node.hasShavingTrack) return;
    api
      .getShavingTrack(sessionId, targetNodeId)
      .then((next) => {
        if (cancelled) return;
        setTrack(next);
        setStepIndex(next.steps.length);
      })
      .catch((error) => {
        if (cancelled) return;
        if (!(error instanceof ApiError && error.status === 404)) {
          console.warn("failed to load shaving track", error);
        }
        setTrack(null);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, targetNodeId, node.hasShavingTrack]);

  useEffect(() => {
    setStepIndex(track ? track.steps.length : 0);
  }, [targetNodeId, track]);

  if (track && track.steps.length > 0) {
    const selectedIndex = Math.min(stepIndex, track.stepDiffs.length - 1);
    const selectedDiff = track.stepDiffs[selectedIndex];
    return (
      <EdgePane
        sessionId={sessionId}
        fromTree={selectedDiff.fromTree}
        toTree={selectedDiff.toTree}
        beforeRef={track.parentTreeSha}
        afterRef={track.headTreeSha}
        loadedEdge={selectedDiff}
        viewedPaths={viewedPaths}
        onToggleViewed={toggleViewed}
        header={titleHeader}
        footer={
          <ShavingTimelineBar
            track={track}
            stepIndex={selectedIndex}
            onStepIndexChange={setStepIndex}
          />
        }
      />
    );
  }

  return (
    <EdgePane
      key={targetNodeId}
      sessionId={sessionId}
      targetNodeId={targetNodeId}
      viewedPaths={viewedPaths}
      onToggleViewed={toggleViewed}
      header={titleHeader}
    />
  );
}

function NodeDiffTitleBar({ title }: { title: string }) {
  const trimmed = title.trim();
  if (!trimmed) return null;
  return (
    <div className="shrink-0 border-b bg-background px-4 py-2">
      <div className="h-4 truncate text-center text-xs text-foreground">
        {trimmed}
      </div>
    </div>
  );
}
