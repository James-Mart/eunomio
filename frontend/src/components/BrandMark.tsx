/* SPDX-License-Identifier: Apache-2.0 */

import { cn } from "@/lib/utils";

type Props = {
  className?: string;
};

const MARK_X = 16;
const MARK_Y = 13.5;

export function BrandMark({ className }: Props) {
  return (
    <svg
      viewBox="0 0 32 32"
      className={cn("brand-mark size-[1.25em] shrink-0", className)}
      aria-hidden
    >
      <circle
        cx="16"
        cy="16"
        r="13"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      />
      <text
        x={MARK_X}
        y={MARK_Y}
        textAnchor="middle"
        dominantBaseline="central"
        fontSize="24"
        fill="currentColor"
        style={{
          fontFamily: "var(--brand-mark-font)",
          fontWeight: "var(--brand-mark-weight)",
        }}
      >
        ε
      </text>
    </svg>
  );
}
