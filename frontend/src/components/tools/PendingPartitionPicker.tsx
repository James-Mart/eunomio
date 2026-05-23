/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, useState } from "react";
import { StopIcon } from "@primer/octicons-react";

import type { Partition } from "@/lib/api";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type Props = {
  options: { partition: Partition; label: string }[];
  onSelect: (partition: Partition) => void;
};

function isConstructAwaitingReview(p: Partition): boolean {
  return p.phase === "construct" && p.phaseState === "awaiting_review";
}

export default function PendingPartitionPicker({ options, onSelect }: Props) {
  const [selectedId, setSelectedId] = useState(() =>
    options[0] ? String(options[0].partition.id) : "",
  );

  useEffect(() => {
    setSelectedId(options[0] ? String(options[0].partition.id) : "");
  }, [options]);

  if (options.length === 0) return null;

  const selected = options.find((o) => String(o.partition.id) === selectedId);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Select value={selectedId} onValueChange={setSelectedId}>
          <SelectTrigger className="h-8 min-w-[12rem] flex-1">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {options.map((o) => (
              <SelectItem key={o.partition.id} value={String(o.partition.id)}>
                {o.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button
          variant="outline"
          disabled={!selected}
          onClick={() => selected && onSelect(selected.partition)}
        >
          View pending partition
        </Button>
      </div>
      {selected && isConstructAwaitingReview(selected.partition) ? (
        <Alert>
          <StopIcon className="h-4 w-4" />
          <AlertTitle>Candidate ready for review</AlertTitle>
          <AlertDescription>
            Inspect the candidate via the candidate view in the graph pane, then
            Accept, re-run the Constructor with feedback, re-run the Planner for
            a different slice, or Abandon.
          </AlertDescription>
        </Alert>
      ) : null}
    </div>
  );
}
