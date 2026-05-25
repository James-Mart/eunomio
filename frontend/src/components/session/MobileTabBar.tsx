/* SPDX-License-Identifier: Apache-2.0 */

import type { ReactNode } from "react";
import {
  GitCompareIcon,
  GraphIcon,
  ToolsIcon,
  type IconProps,
} from "@primer/octicons-react";

import { cn } from "@/lib/utils";

import type { ActiveTab } from "./useSessionActiveTab";

/** Reserve space below mobile session panels for the fixed tab bar. */
export const mobileTabBarInsetClass =
  "pb-[calc(4rem+env(safe-area-inset-bottom,0px))]";

type IconComponent = React.ComponentType<IconProps>;

const TABS: { value: ActiveTab; label: string; icon: IconComponent }[] = [
  { value: "graph", label: "Graph", icon: GraphIcon },
  { value: "diff", label: "Diff", icon: GitCompareIcon },
  { value: "tools", label: "Tools", icon: ToolsIcon },
];

export function TabPanel({
  id,
  active,
  children,
}: {
  id: ActiveTab;
  active: boolean;
  children: ReactNode;
}) {
  return (
    <div
      role="tabpanel"
      id={`session-panel-${id}`}
      aria-labelledby={`session-tab-${id}`}
      aria-hidden={!active}
      className={cn(
        "absolute inset-0 flex min-h-0 flex-col overflow-hidden",
        active ? "z-10 bg-background" : "hidden",
      )}
    >
      {children}
    </div>
  );
}

export function BottomTabBar({
  value,
  onChange,
}: {
  value: ActiveTab;
  onChange: (next: ActiveTab) => void;
}) {
  return (
    <nav
      role="tablist"
      aria-label="Session view"
      className="fixed inset-x-0 bottom-0 z-40 flex h-16 shrink-0 items-stretch border-t bg-background pb-[env(safe-area-inset-bottom)]"
    >
      {TABS.map(({ value: tabValue, label, icon: Icon }) => {
        const isActive = tabValue === value;
        return (
          <button
            key={tabValue}
            type="button"
            role="tab"
            id={`session-tab-${tabValue}`}
            aria-selected={isActive}
            aria-controls={`session-panel-${tabValue}`}
            onClick={() => onChange(tabValue)}
            className={cn(
              "flex flex-1 flex-col items-center justify-center gap-0.5 text-xs transition-colors",
              isActive
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <span
              className={cn(
                "flex h-7 w-12 items-center justify-center rounded-full",
                isActive && "bg-secondary",
              )}
            >
              <Icon className="h-5 w-5" aria-hidden="true" />
            </span>
            <span className={cn(isActive && "font-medium")}>{label}</span>
          </button>
        );
      })}
    </nav>
  );
}
