import { useEffect, useState } from "react";
import { Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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

type ToolsTab = "partition" | "info" | "branch";

export default function ToolsPane(ctx: ToolsContext) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [tab, setTab] = useState<ToolsTab>("partition");
  const showNodeTabs = showNodeTools(ctx);

  useEffect(() => {
    if (!showNodeTabs && tab !== "partition") setTab("partition");
  }, [showNodeTabs, tab]);

  if (isToolsEmpty(ctx)) {
    return (
      <ToolsEmpty className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground" />
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
        {PartitionToolPanel(ctx)}
      </TabsContent>
      {showNodeTabs && (
        <TabsContent value="info" className="mt-0 flex-1 min-h-0">
          {InfoToolPanel(ctx)}
        </TabsContent>
      )}
      {showNodeTabs && (
        <TabsContent value="branch" className="mt-0 flex-1 min-h-0">
          {BranchToolPanel(ctx)}
        </TabsContent>
      )}

      <PartitionSettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </Tabs>
  );
}
