/* SPDX-License-Identifier: Apache-2.0 */

import { Fragment } from "react";
import type { IconProps } from "@primer/octicons-react";
import {
  AlertIcon,
  CheckCircleIcon,
  CodeIcon,
  PauseIcon,
  TasklistIcon,
} from "@primer/octicons-react";

import { cn } from "@/lib/utils";
import type { PhaseName, PhaseState } from "@/lib/sessionEvents";

type IconComponent = React.ComponentType<IconProps>;

const STEPS: { name: PhaseName; label: string; icon: IconComponent }[] = [
  { name: "plan", label: "Plan", icon: TasklistIcon },
  { name: "construct", label: "Construct", icon: CodeIcon },
];

export type LifecycleStateValue = PhaseState | "pending" | "done";
export type LifecycleStates = Record<PhaseName, LifecycleStateValue>;

const PHASE_ORDER: readonly PhaseName[] = ["plan", "construct"];

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
        "flex w-full list-none items-center p-0",
        compact && "gap-1",
      )}
      aria-label="Partition lifecycle"
    >
      {STEPS.map((step, idx) => {
        const state = states[step.name];
        const showSeparator = !compact && idx < STEPS.length - 1;
        return (
          <Fragment key={step.name}>
            <li className="flex shrink-0 items-center">
              <Step
                label={step.label}
                icon={step.icon}
                state={state}
                compact={compact}
              />
            </li>
            {showSeparator ? (
              <li
                className="mx-2 h-px min-w-2 flex-1 bg-border"
                aria-hidden
              />
            ) : null}
          </Fragment>
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
  icon: IconComponent;
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

function statusIconFor(state: LifecycleStateValue, fallback: IconComponent): IconComponent {
  switch (state) {
    case "awaiting_review":
      return PauseIcon;
    case "done":
      return CheckCircleIcon;
    case "error":
      return AlertIcon;
    default:
      return fallback;
  }
}

function colorFor(state: LifecycleStateValue): string {
  switch (state) {
    case "pending":
      return "text-muted-foreground/50";
    case "running":
      return "text-attention";
    case "awaiting_review":
      return "text-attention";
    case "done":
      return "text-success";
    case "error":
      return "text-danger";
  }
}
