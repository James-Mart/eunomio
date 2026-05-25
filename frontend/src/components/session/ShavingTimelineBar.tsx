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
    <div className="h-16 shrink-0 border-t bg-background px-4 py-2">
      <div className="h-4 truncate text-xs text-muted-foreground">
        {activeLabel}
      </div>
      <div className="flex items-center gap-3">
        <span className="w-28 text-xs tabular-nums text-muted-foreground">
          Step {stepIndex + 1} / {track.steps.length + 1}
        </span>
        <input
          className="h-2 flex-1 accent-primary"
          type="range"
          min={0}
          max={max}
          step={1}
          value={stepIndex}
          onChange={(event) => onStepIndexChange(Number(event.target.value))}
        />
      </div>
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
