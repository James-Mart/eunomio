/* SPDX-License-Identifier: Apache-2.0 */

import type { PartitionStrategy } from "@/lib/api";

export const STRATEGY_OPTIONS: ReadonlyArray<{
  value: "auto" | PartitionStrategy;
  label: string;
}> = [
  { value: "auto", label: "Auto (let planner choose)" },
  { value: "synthetic", label: "Synthetic" },
  { value: "vertical", label: "Vertical" },
  { value: "horizontal", label: "Horizontal" },
];
