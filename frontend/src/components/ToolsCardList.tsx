import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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
  if (isToolsEmpty(ctx)) return <ToolsEmpty />;
  const showCards = showNodeTools(ctx);

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto bg-background p-4">
      <Card>
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-base font-semibold">Partition</CardTitle>
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

      {showCards && ctx.isLocal && (
        <Card>
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-base font-semibold">Branch</CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-2">{BranchToolPanel(ctx)}</CardContent>
        </Card>
      )}
    </div>
  );
}
