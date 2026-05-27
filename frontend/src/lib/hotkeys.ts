/* SPDX-License-Identifier: Apache-2.0 */

export type TimelineHotkeyAction =
  | "prevStep"
  | "nextStep"
  | "jumpStart"
  | "jumpEnd";

export type TimelineHotkeyBinding = {
  id: TimelineHotkeyAction;
  keys: string;
  label: string;
};

export type TimelineControls = {
  stepIndex: number;
  maxStepIndex: number;
  setStepIndex: (index: number) => void;
};

export const TIMELINE_HOTKEY_BINDINGS: TimelineHotkeyBinding[] = [
  { id: "prevStep", keys: "[", label: "Previous timeline step" },
  { id: "nextStep", keys: "]", label: "Next timeline step" },
  { id: "jumpStart", keys: "Shift+[", label: "Jump to timeline start" },
  { id: "jumpEnd", keys: "Shift+]", label: "Jump to timeline end" },
];

export function shouldIgnoreHotkeyTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  // Diff "viewed" toggles are mouse-first; bracket keys should still scrub timeline.
  if (target.closest("[data-edge-viewed-control]")) return false;
  const el = target.closest(
    "input:not([type='range']), textarea, select, button, a, [contenteditable='true'], [role='tree']",
  );
  return el !== null;
}

export function resolveTimelineHotkeyAction(
  event: Pick<KeyboardEvent, "key" | "code" | "shiftKey" | "metaKey" | "ctrlKey" | "altKey">,
): TimelineHotkeyAction | null {
  if (event.metaKey || event.ctrlKey || event.altKey) return null;
  const bracketLeft =
    event.code === "BracketLeft" || event.key === "[" || event.key === "{";
  const bracketRight =
    event.code === "BracketRight" || event.key === "]" || event.key === "}";
  if (bracketLeft) {
    return event.shiftKey ? "jumpStart" : "prevStep";
  }
  if (bracketRight) {
    return event.shiftKey ? "jumpEnd" : "nextStep";
  }
  return null;
}

export function applyTimelineHotkeyAction(
  action: TimelineHotkeyAction,
  controls: TimelineControls,
): void {
  const { stepIndex, maxStepIndex, setStepIndex } = controls;
  switch (action) {
    case "prevStep":
      setStepIndex(Math.max(0, stepIndex - 1));
      return;
    case "nextStep":
      setStepIndex(Math.min(maxStepIndex, stepIndex + 1));
      return;
    case "jumpStart":
      setStepIndex(0);
      return;
    case "jumpEnd":
      setStepIndex(maxStepIndex);
      return;
  }
}

export function handleTimelineHotkey(
  event: KeyboardEvent,
  controls: TimelineControls | null,
): boolean {
  if (!controls || shouldIgnoreHotkeyTarget(event.target)) return false;
  const action = resolveTimelineHotkeyAction(event);
  if (!action) return false;
  event.preventDefault();
  applyTimelineHotkeyAction(action, controls);
  return true;
}
