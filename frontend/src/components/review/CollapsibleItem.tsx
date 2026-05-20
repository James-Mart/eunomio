import { useState } from "react";
import { ChevronRight } from "lucide-react";

import { cn } from "@/lib/utils";

type Props = {
  title: string;
  description: string;
  defaultOpen?: boolean;
  leadingLabel?: string;
};

export default function CollapsibleItem({
  title,
  description,
  defaultOpen,
  leadingLabel,
}: Props) {
  const [open, setOpen] = useState(!!defaultOpen);
  return (
    <div className="rounded-md border bg-muted/30">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left"
      >
        <ChevronRight
          className={cn(
            "h-3.5 w-3.5 shrink-0 transition-transform",
            open && "rotate-90",
          )}
          aria-hidden="true"
        />
        <span className="min-w-0 flex-1">
          {leadingLabel && (
            <span className="block text-xs text-muted-foreground">
              {leadingLabel}
            </span>
          )}
          <span className="block text-sm font-medium">{title}</span>
        </span>
      </button>
      {open && (
        <div className="px-3 pb-2 pl-8 text-xs text-muted-foreground">
          {description}
        </div>
      )}
    </div>
  );
}
