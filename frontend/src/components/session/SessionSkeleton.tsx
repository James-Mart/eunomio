/* SPDX-License-Identifier: Apache-2.0 */

import { DiffPaneSkeleton } from "@/components/session/DiffPaneSkeleton";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

import {
  BottomTabBar,
  mobileTabBarInsetClass,
  TabPanel,
} from "./MobileTabBar";
import { useSessionActiveTab } from "./useSessionActiveTab";

function SessionGraphSkeleton() {
  return (
    <div className="flex h-full flex-col gap-3 p-3">
      <Skeleton className="h-9 w-48" />
      <Skeleton className="min-h-0 flex-1" />
    </div>
  );
}

function SessionToolsSkeleton() {
  return (
    <div className="flex h-full flex-col gap-3 p-4">
      <Skeleton className="h-8 w-full max-w-xs" />
      <Skeleton className="h-24 w-full" />
      <Skeleton className="h-24 w-full" />
    </div>
  );
}

export function SessionSkeleton() {
  const desktopSplitLayout = useDefaultLayout({
    id: "session-desktop-split-v3",
    panelIds: ["diff", "aux"],
  });
  const desktopAuxSplitLayout = useDefaultLayout({
    id: "session-desktop-aux-split-v1",
    panelIds: ["graph", "tools"],
  });
  const { activeTab, setActiveTab } = useSessionActiveTab();

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="hidden min-h-0 flex-1 md:flex md:flex-col">
        <ResizablePanelGroup
          orientation="horizontal"
          defaultLayout={desktopSplitLayout.defaultLayout}
          onLayoutChanged={desktopSplitLayout.onLayoutChanged}
          className="h-full"
        >
          <ResizablePanel
            id="diff"
            defaultSize="70%"
            minSize="30%"
            maxSize="85%"
            className="min-w-0"
          >
            <div className="h-full min-w-0 overflow-hidden pb-2 pr-2">
              <DiffPaneSkeleton />
            </div>
          </ResizablePanel>
          <ResizableHandle withHandle aria-label="Resize panes" />
          <ResizablePanel
            id="aux"
            defaultSize="30%"
            minSize="15%"
            className="min-w-0"
          >
            <ResizablePanelGroup
              orientation="vertical"
              defaultLayout={desktopAuxSplitLayout.defaultLayout}
              onLayoutChanged={desktopAuxSplitLayout.onLayoutChanged}
              className="h-full min-w-0 overflow-hidden"
            >
              <ResizablePanel
                id="graph"
                defaultSize="50%"
                minSize="15%"
                maxSize="85%"
                className="min-h-0 overflow-hidden"
              >
                <SessionGraphSkeleton />
              </ResizablePanel>
              <ResizableHandle
                withHandle
                aria-label="Resize tools and graph"
              />
              <ResizablePanel
                id="tools"
                defaultSize="50%"
                minSize="15%"
                maxSize="85%"
                className="min-h-0 overflow-auto"
              >
                <SessionToolsSkeleton />
              </ResizablePanel>
            </ResizablePanelGroup>
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>

      <div
        className={cn(
          "flex min-h-0 flex-1 flex-col overflow-hidden md:hidden",
          mobileTabBarInsetClass,
        )}
      >
        <div className="relative min-h-0 flex-1 overflow-hidden">
          <TabPanel id="graph" active={activeTab === "graph"}>
            <SessionGraphSkeleton />
          </TabPanel>
          <TabPanel id="diff" active={activeTab === "diff"}>
            <DiffPaneSkeleton />
          </TabPanel>
          <TabPanel id="tools" active={activeTab === "tools"}>
            <SessionToolsSkeleton />
          </TabPanel>
        </div>
        <BottomTabBar value={activeTab} onChange={setActiveTab} />
      </div>
    </div>
  );
}
