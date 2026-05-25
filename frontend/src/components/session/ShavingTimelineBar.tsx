/* SPDX-License-Identifier: Apache-2.0 */

import type { ShavingTrack } from "@/lib/api";

type Props = {
  track: ShavingTrack;
  stepIndex: number;
  onStepIndexChange: (index: number) => void;
};

export function ShavingTimelineBar({
  track,
  stepIndex,
  onStepIndexChange,
}: Props) {
  const max = Math.max(0, track.steps.length);
  const activeLabel =
    stepIndex < track.steps.length
      ? carriedLabel(track, stepIndex)
      : "";
  return (
    <div className="shrink-0 border-t bg-background px-4 py-2">
      <div className="h-4 truncate text-center text-xs text-foreground">
        {activeLabel}
      </div>
      <input
        className="mt-2 h-2 w-full accent-primary"
        type="range"
        min={0}
        max={max}
        step={1}
        value={stepIndex}
        onChange={(event) => onStepIndexChange(Number(event.target.value))}
      />
    </div>
  );
}

function carriedLabel(track: ShavingTrack, stepIndex: number): string {
  for (let i = stepIndex; i >= 0; i -= 1) {
    const label = track.steps[i]?.label?.trim();
    if (label) return label;
  }
  return "";
}
