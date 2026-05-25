/* SPDX-License-Identifier: Apache-2.0 */

import EdgePane from "@/components/EdgePane";
import { useEdgeFileViewed } from "@/lib/useEdgeFileViewed";

type Props = {
  sessionId: string;
  targetNodeId: string;
};

export function CanonicalEdgePane({ sessionId, targetNodeId }: Props) {
  const { viewedPaths, toggleViewed } = useEdgeFileViewed(sessionId, targetNodeId);
  return (
    <EdgePane
      key={targetNodeId}
      sessionId={sessionId}
      targetNodeId={targetNodeId}
      viewedPaths={viewedPaths}
      onToggleViewed={toggleViewed}
    />
  );
}
