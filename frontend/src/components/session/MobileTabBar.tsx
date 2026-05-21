import type { ReactNode } from "react";
import {
  GitCompareArrows,
  Network,
  Wrench,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";

import type { ActiveTab } from "./useSessionActiveTab";

const TABS: { value: ActiveTab; label: string; icon: LucideIcon }[] = [
  { value: "graph", label: "Graph", icon: Network },
  { value: "diff", label: "Diff", icon: GitCompareArrows },
  { value: "tools", label: "Tools", icon: Wrench },
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
        "absolute inset-0",
        !active && "invisible pointer-events-none",
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
      className="flex h-16 shrink-0 items-stretch border-t bg-background pb-[env(safe-area-inset-bottom)]"
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
