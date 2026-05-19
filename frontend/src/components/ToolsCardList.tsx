import { useState } from "react";
import { Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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

export default function ToolsCardList({ sessionId, nodeId, nodeTitle, onChange }: Props) {
  const [settingsOpen, setSettingsOpen] = useState(false);

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
          <PartitionTab sessionId={sessionId} targetNodeId={nodeId} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-base font-semibold">Info</CardTitle>
        </CardHeader>
        <CardContent className="p-4 pt-2">
          <InfoTab
            sessionId={sessionId}
            nodeId={nodeId}
            nodeTitle={nodeTitle}
            onChange={onChange}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-base font-semibold">Branch</CardTitle>
        </CardHeader>
        <CardContent className="p-4 pt-2">
          <BranchTab sessionId={sessionId} nodeId={nodeId} nodeTitle={nodeTitle} />
        </CardContent>
      </Card>

      <PartitionSettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        sessionId={sessionId}
      />
    </div>
  );
}
