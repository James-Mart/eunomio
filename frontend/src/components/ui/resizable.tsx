import * as React from "react"
import { GripVertical } from "lucide-react"
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels"

import { cn } from "@/lib/utils"

const ResizablePanelGroup = ({
  className,
  ...props
}: React.ComponentProps<typeof Group>) => (
  <Group
    className={cn(
      "flex h-full w-full data-[orientation=vertical]:flex-col",
      className
    )}
    {...props}
  />
)

const ResizablePanel = Panel

const ResizableHandle = ({
  withHandle,
  className,
  ...props
}: React.ComponentProps<typeof Separator> & {
  withHandle?: boolean
}) => (
  <Separator
    className={cn(
      "relative flex w-px items-center justify-center bg-border focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-offset-1 data-[orientation=vertical]:h-px data-[orientation=vertical]:w-full",
      className
    )}
    {...props}
  >
    {withHandle ? (
      <div className="z-10 flex h-6 w-4 items-center justify-center rounded-sm border bg-background">
        <GripVertical className="h-3 w-3" />
      </div>
    ) : null}
  </Separator>
)

export { ResizablePanelGroup, ResizablePanel, ResizableHandle, useDefaultLayout }
