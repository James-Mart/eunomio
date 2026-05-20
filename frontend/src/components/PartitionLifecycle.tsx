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

export type LifecycleStateValue = PhaseState | "pending" | "done";
export type LifecycleStates = Record<PhaseName, LifecycleStateValue>;

const PHASE_ORDER: readonly PhaseName[] = ["survey", "plan", "construct"];

export function lifecycleStatesFromPhase(
  phase: PhaseName,
  phaseState: PhaseState,
): LifecycleStates {
  const activeIdx = PHASE_ORDER.indexOf(phase);
  const out = {} as LifecycleStates;
  PHASE_ORDER.forEach((name, idx) => {
    if (idx < activeIdx) out[name] = "done";
    else if (idx === activeIdx) out[name] = phaseState;
    else out[name] = "pending";
  });
  return out;
}

export function LifecycleStepper({
  states,
  compact = false,
}: {
  states: LifecycleStates;
  compact?: boolean;
}) {
  return (
    <ol
      className={cn(
        "flex items-center",
        compact ? "gap-1" : "w-full gap-2",
      )}
      aria-label="Partition lifecycle"
    >
      {STEPS.map((step, idx) => {
        const state = states[step.name];
        const showSeparator = !compact && idx < STEPS.length - 1;
        return (
          <li
            key={step.name}
            className={cn(
              "flex items-center",
              compact ? "" : "flex-1 gap-2",
            )}
          >
            <Step
              label={step.label}
              icon={step.icon}
              state={state}
              compact={compact}
            />
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
  compact,
}: {
  label: string;
  icon: LucideIcon;
  state: LifecycleStateValue;
  compact: boolean;
}) {
  const StatusIcon = statusIconFor(state, Icon);
  const color = colorFor(state);
  const pulse = state === "running" ? "animate-pulse" : "";
  if (compact) {
    return (
      <StatusIcon
        className={cn("h-4 w-4 shrink-0", color, pulse)}
        aria-label={`${label}: ${state}`}
      />
    );
  }
  return (
    <div className="flex items-center gap-1.5 whitespace-nowrap">
      <StatusIcon className={cn("h-4 w-4 shrink-0", color, pulse)} aria-hidden />
      <span className={cn("text-xs md:text-sm", color)}>{label}</span>
    </div>
  );
}

function statusIconFor(state: LifecycleStateValue, fallback: LucideIcon): LucideIcon {
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

function colorFor(state: LifecycleStateValue): string {
  switch (state) {
    case "pending":
      return "text-muted-foreground/50";
    case "running":
      return "text-amber-500";
    case "awaiting_review":
      return "text-red-500";
    case "done":
      return "text-emerald-500";
    case "error":
      return "text-red-500";
  }
}
