/* SPDX-License-Identifier: Apache-2.0 */

import { useCallback } from "react";
import { useSearchParams } from "react-router-dom";

export type ActiveTab = "graph" | "diff" | "tools";

function parseActiveTab(raw: string | null): ActiveTab {
  return raw === "diff" || raw === "tools" ? raw : "graph";
}

export function useSessionActiveTab(): {
  activeTab: ActiveTab;
  setActiveTab: (next: ActiveTab) => void;
} {
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab = parseActiveTab(searchParams.get("tab"));

  const setActiveTab = useCallback(
    (next: ActiveTab) => {
      setSearchParams(
        (prev) => {
          const updated = new URLSearchParams(prev);
          if (next === "graph") updated.delete("tab");
          else updated.set("tab", next);
          return updated;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );

  return { activeTab, setActiveTab };
}
