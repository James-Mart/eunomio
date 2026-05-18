import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { Star } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { type GraphNode } from "@/lib/api";
import { cn } from "@/lib/utils";

export type NodeCardData = {
  node: GraphNode;
  sessionId?: string;
  onChange?: () => void;
};

export default function NodeCard({ data, selected }: NodeProps) {
  const { node } = data as NodeCardData;
  const [favorite, setFavorite] = useState(node.isFavorite);

  return (
    <>
      <Handle type="target" position={Position.Left} />
      <Card
        className={cn(
          "w-[220px] shadow-md",
          selected && "ring-2 ring-primary ring-offset-2 ring-offset-background",
        )}
      >
        <CardContent className="flex items-center justify-between gap-2 p-3">
          <span className="flex-1 truncate text-sm font-semibold">{node.title}</span>
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              setFavorite((v) => !v);
            }}
            aria-label={favorite ? "Unfavorite" : "Favorite"}
            className="text-muted-foreground hover:text-yellow-500"
          >
            <Star
              className={cn("h-4 w-4", favorite && "fill-yellow-500 text-yellow-500")}
            />
          </button>
        </CardContent>
      </Card>
      <Handle type="source" position={Position.Right} />
    </>
  );
}
