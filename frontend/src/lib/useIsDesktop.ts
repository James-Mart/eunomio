/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, useState } from "react";

const DESKTOP_QUERY = "(min-width: 768px)";

export function useIsDesktop(): boolean {
  const [matches, setMatches] = useState(() =>
    typeof window !== "undefined"
      ? window.matchMedia(DESKTOP_QUERY).matches
      : true,
  );
  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia(DESKTOP_QUERY);
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);
  return matches;
}

export function isDesktopViewport(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia(DESKTOP_QUERY).matches
  );
}
