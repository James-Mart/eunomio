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
      "relative flex shrink-0 items-center justify-center bg-zinc-300 hover:bg-zinc-400 dark:bg-zinc-600 dark:hover:bg-zinc-500 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-offset-1",
      // v4 sets aria-orientation on the separator, not data-orientation
      "w-0.5 aria-[orientation=vertical]:h-full",
      "aria-[orientation=horizontal]:h-[3px] aria-[orientation=horizontal]:w-full",
      "aria-[orientation=horizontal]:[&>div]:h-4 aria-[orientation=horizontal]:[&>div]:w-6",
      "aria-[orientation=horizontal]:[&_svg]:rotate-90",
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
