import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";
import { Skeleton } from "@/components/ui/skeleton";
import { useIsDesktop } from "@/lib/useIsDesktop";

export function DiffPaneSkeleton() {
  const isDesktop = useIsDesktop();
  const treeSplitLayout = useDefaultLayout({
    id: "edge-pane-tree-split-v1",
    panelIds: ["tree", "diff"],
  });

  return (
    <div className="h-full min-h-0 w-full">
      <ResizablePanelGroup
        orientation="horizontal"
        defaultLayout={treeSplitLayout.defaultLayout}
        onLayoutChanged={treeSplitLayout.onLayoutChanged}
        className="h-full"
      >
        {isDesktop && (
          <>
            <ResizablePanel
              id="tree"
              defaultSize="16rem"
              minSize="10rem"
              maxSize="40%"
              className="min-w-0 border-r"
            >
              <div className="flex h-full flex-col gap-2 p-3">
                {Array.from({ length: 8 }).map((_, i) => (
                  <Skeleton
                    key={i}
                    className="h-4"
                    style={{ width: `${60 + ((i * 13) % 35)}%` }}
                  />
                ))}
              </div>
            </ResizablePanel>
            <ResizableHandle
              withHandle
              aria-label="Resize file tree"
              className="mx-1"
            />
          </>
        )}
        <ResizablePanel id="diff" minSize="30%" className="min-w-0">
          <div className="flex h-full min-w-0 flex-col">
            <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-b px-3 py-1.5 pr-12 md:pr-3">
              <Skeleton className="h-7 w-[8.5rem]" />
              <Skeleton className="h-7 w-[8.5rem]" />
            </div>
            <div className="min-h-0 flex-1 space-y-4 overflow-hidden p-3">
              {Array.from({ length: 3 }).map((_, i) => (
                <div key={i} className="space-y-2">
                  <Skeleton className="h-6 w-1/3" />
                  <Skeleton className="h-4 w-full" />
                  <Skeleton className="h-4 w-5/6" />
                  <Skeleton className="h-4 w-2/3" />
                </div>
              ))}
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
