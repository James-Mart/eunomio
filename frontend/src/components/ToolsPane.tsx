import { useEffect, useState } from "react";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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
      className="flex min-h-full w-full flex-col gap-3 px-4 pt-4 pb-6"
    >
      <TabsList variant="underline" className="w-full">
        <TabsTrigger variant="underline" value="partition">
          Partition
        </TabsTrigger>
        {showNodeTabs && (
          <TabsTrigger variant="underline" value="info">
            Info
          </TabsTrigger>
        )}
        {showNodeTabs && (
          <TabsTrigger variant="underline" value="branch">
            Branch
          </TabsTrigger>
        )}
      </TabsList>
      <TabsContent value="partition" className="mt-0 w-full flex-1 min-h-0">
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
    </Tabs>
  );
}
