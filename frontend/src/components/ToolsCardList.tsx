import { useState } from "react";
import { Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import PartitionSettingsDialog from "@/components/PartitionSettingsDialog";
import {
  BranchToolPanel,
  InfoToolPanel,
  PartitionToolPanel,
  ToolsEmpty,
  isToolsEmpty,
  showNodeTools,
  type ToolsContext,
} from "@/components/tools/ToolPanels";

export default function ToolsCardList(ctx: ToolsContext) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  if (isToolsEmpty(ctx)) return <ToolsEmpty />;
  const showCards = showNodeTools(ctx);

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
          {PartitionToolPanel(ctx)}
        </CardContent>
      </Card>

      {showCards && (
        <Card>
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-base font-semibold">Info</CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-2">{InfoToolPanel(ctx)}</CardContent>
        </Card>
      )}

      {showCards && (
        <Card>
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-base font-semibold">Branch</CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-2">{BranchToolPanel(ctx)}</CardContent>
        </Card>
      )}

      <PartitionSettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </div>
  );
}
