import { useState } from "react";
import { Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import BranchTab from "@/components/BranchTab";
import InfoTab from "@/components/InfoTab";
import PartitionSettingsDialog from "@/components/PartitionSettingsDialog";
import PartitionTab from "@/components/PartitionTab";
import type { Partition } from "@/lib/api";

type Props = {
  sessionId: string;
  nodeId: string | null;
  nodeTitle: string | null;
  activePartition: Partition | null;
  isCandidateSliceSelected: boolean;
  onPartitionStarted: (p: Partition) => void;
  onPartitionEnded: () => void;
  onChange?: () => void;
};

export default function ToolsCardList({
  sessionId,
  nodeId,
  nodeTitle,
  activePartition,
  isCandidateSliceSelected,
  onPartitionStarted,
  onPartitionEnded,
  onChange,
}: Props) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const showNodeCards = activePartition === null && nodeId !== null;

  if (activePartition === null && nodeId === null) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">
        Select a node or partition to view tools.
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto bg-background p-4">
      <Card>
        <CardHeader className="flex-row items-center justify-between space-y-0 p-4 pb-2">
          <CardTitle className="text-base font-semibold">Partition</CardTitle>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 text-muted-foreground"
            onClick={() => setSettingsOpen(true)}
            aria-label="Partition settings"
          >
            <Settings className="h-4 w-4" aria-hidden="true" />
          </Button>
        </CardHeader>
        <CardContent className="p-4 pt-2">
          <PartitionTab
            key={activePartition?.id ?? "none"}
            sessionId={sessionId}
            targetNodeId={nodeId}
            activePartition={activePartition}
            isCandidateSliceSelected={isCandidateSliceSelected}
            onPartitionStarted={onPartitionStarted}
            onPartitionEnded={onPartitionEnded}
          />
        </CardContent>
      </Card>

      {showNodeCards && (
        <Card>
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-base font-semibold">Info</CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-2">
            <InfoTab
              key={nodeId ?? "none"}
              sessionId={sessionId}
              nodeId={nodeId!}
              nodeTitle={nodeTitle!}
              onChange={onChange}
            />
          </CardContent>
        </Card>
      )}

      {showNodeCards && (
        <Card>
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-base font-semibold">Branch</CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-2">
            <BranchTab
              key={nodeId ?? "none"}
              sessionId={sessionId}
              nodeId={nodeId!}
              nodeTitle={nodeTitle!}
            />
          </CardContent>
        </Card>
      )}

      <PartitionSettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </div>
  );
}
