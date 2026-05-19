import { useState } from "react";
import { Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import BranchTab from "@/components/BranchTab";
import InfoTab from "@/components/InfoTab";
import PartitionSettingsDialog from "@/components/PartitionSettingsDialog";
import PartitionTab from "@/components/PartitionTab";

type Props = {
  sessionId: string;
  nodeId: string;
  nodeTitle: string;
  onChange?: () => void;
};

export default function ToolsPane({ sessionId, nodeId, nodeTitle, onChange }: Props) {
  const [settingsOpen, setSettingsOpen] = useState(false);

  return (
    <Tabs defaultValue="partition" className="flex h-full flex-col gap-3 p-4">
      <div className="flex items-center justify-between gap-2">
        <TabsList>
          <TabsTrigger value="partition">Partition</TabsTrigger>
          <TabsTrigger value="info">Info</TabsTrigger>
          <TabsTrigger value="branch">Branch</TabsTrigger>
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
        <PartitionTab sessionId={sessionId} targetNodeId={nodeId} />
      </TabsContent>
      <TabsContent value="info" className="mt-0 flex-1 min-h-0">
        <InfoTab
          sessionId={sessionId}
          nodeId={nodeId}
          nodeTitle={nodeTitle}
          onChange={onChange}
        />
      </TabsContent>
      <TabsContent value="branch" className="mt-0 flex-1 min-h-0">
        <BranchTab sessionId={sessionId} nodeId={nodeId} nodeTitle={nodeTitle} />
      </TabsContent>

      <PartitionSettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        sessionId={sessionId}
      />
    </Tabs>
  );
}
