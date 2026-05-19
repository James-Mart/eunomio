import {
  CircleAlert,
  CircleCheck,
  Code2,
  ListChecks,
  PauseCircle,
  Telescope,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import type { PhaseName, PhaseState } from "@/lib/sessionEvents";

const STEPS: { name: PhaseName; label: string; icon: LucideIcon }[] = [
  { name: "survey", label: "Survey", icon: Telescope },
  { name: "plan", label: "Plan", icon: ListChecks },
  { name: "construct", label: "Construct", icon: Code2 },
];

export type LifecycleStates = Record<PhaseName, PhaseState>;

export function LifecycleStepper({ states }: { states: LifecycleStates }) {
  return (
    <ol className="flex w-full items-center gap-2" aria-label="Partition lifecycle">
      {STEPS.map((step, idx) => {
        const state = states[step.name];
        const showSeparator = idx < STEPS.length - 1;
        return (
          <li key={step.name} className="flex flex-1 items-center gap-2">
            <Step label={step.label} icon={step.icon} state={state} />
            {showSeparator && <div className="h-px flex-1 bg-border" aria-hidden />}
          </li>
        );
      })}
    </ol>
  );
}

function Step({
  label,
  icon: Icon,
  state,
}: {
  label: string;
  icon: LucideIcon;
  state: PhaseState;
}) {
  const StatusIcon = statusIconFor(state, Icon);
  const color = colorFor(state);
  return (
    <div className="flex items-center gap-1.5 whitespace-nowrap">
      <StatusIcon className={cn("h-4 w-4 shrink-0", color)} aria-hidden />
      <span className={cn("text-xs md:text-sm", color)}>{label}</span>
    </div>
  );
}

function statusIconFor(state: PhaseState, fallback: LucideIcon): LucideIcon {
  switch (state) {
    case "awaiting_review":
      return PauseCircle;
    case "done":
      return CircleCheck;
    case "error":
      return CircleAlert;
    default:
      return fallback;
  }
}

function colorFor(state: PhaseState): string {
  switch (state) {
    case "pending":
      return "text-muted-foreground";
    case "running":
      return "text-primary";
    case "awaiting_review":
      return "text-primary";
    case "done":
      return "text-emerald-600";
    case "error":
      return "text-destructive";
  }
}
