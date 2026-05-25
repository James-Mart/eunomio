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
  const max = Math.max(0, track.steps.length - 1);
  return (
    <div className="flex h-14 shrink-0 items-center gap-3 border-t bg-background px-4">
      <span className="w-24 text-xs tabular-nums text-muted-foreground">
        Step {stepIndex + 1} of {track.steps.length}
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
  );
}
