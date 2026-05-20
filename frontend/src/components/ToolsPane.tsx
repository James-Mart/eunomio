import { useEffect, useState } from "react";
import { Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import BranchTab from "@/components/BranchTab";
import InfoTab from "@/components/InfoTab";
import PartitionSettingsDialog from "@/components/PartitionSettingsDialog";
import PartitionTab from "@/components/PartitionTab";
import type { Partition } from "@/lib/api";

type ToolsTab = "partition" | "info" | "branch";

type Props = {
  sessionId: string;
  nodeId: string | null;
  nodeTitle: string | null;
  nodeDescription: string | null;
  activePartition: Partition | null;
  isCandidateSliceSelected: boolean;
  onPartitionStarted: (p: Partition) => void;
  onPartitionEnded: () => void;
  onChange?: () => void;
};

export default function ToolsPane({
  sessionId,
  nodeId,
  nodeTitle,
  nodeDescription,
  activePartition,
  isCandidateSliceSelected,
  onPartitionStarted,
  onPartitionEnded,
  onChange,
}: Props) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [tab, setTab] = useState<ToolsTab>("partition");
  const showNodeTabs = activePartition === null && nodeId !== null;

  useEffect(() => {
    if (!showNodeTabs && tab !== "partition") setTab("partition");
  }, [showNodeTabs, tab]);

  if (activePartition === null && nodeId === null) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">
        Select a node or partition to view tools.
      </div>
    );
  }

  return (
    <Tabs
      value={tab}
      onValueChange={(v) => setTab(v as ToolsTab)}
      className="flex h-full flex-col gap-3 p-4"
    >
      <div className="flex items-center justify-between gap-2">
        <TabsList>
          <TabsTrigger value="partition">Partition</TabsTrigger>
          {showNodeTabs && <TabsTrigger value="info">Info</TabsTrigger>}
          {showNodeTabs && <TabsTrigger value="branch">Branch</TabsTrigger>}
        </TabsList>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8 text-muted-foreground"
          onClick={() => setSettingsOpen(true)}
          aria-label="Partition settings"
        >
          <Settings className="h-4 w-4" aria-hidden="true" />
        </Button>
      </div>
      <TabsContent value="partition" className="mt-0 flex-1 min-h-0">
        <PartitionTab
          key={activePartition?.id ?? "none"}
          sessionId={sessionId}
          targetNodeId={nodeId}
          activePartition={activePartition}
          isCandidateSliceSelected={isCandidateSliceSelected}
          onPartitionStarted={onPartitionStarted}
          onPartitionEnded={onPartitionEnded}
        />
      </TabsContent>
      {showNodeTabs && (
        <TabsContent value="info" className="mt-0 flex-1 min-h-0">
          <InfoTab
            key={nodeId ?? "none"}
            sessionId={sessionId}
            nodeId={nodeId!}
            nodeTitle={nodeTitle!}
            nodeDescription={nodeDescription ?? ""}
            onChange={onChange}
          />
        </TabsContent>
      )}
      {showNodeTabs && (
        <TabsContent value="branch" className="mt-0 flex-1 min-h-0">
          <BranchTab
            key={nodeId ?? "none"}
            sessionId={sessionId}
            nodeId={nodeId!}
            nodeTitle={nodeTitle!}
          />
        </TabsContent>
      )}

      <PartitionSettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </Tabs>
  );
}
