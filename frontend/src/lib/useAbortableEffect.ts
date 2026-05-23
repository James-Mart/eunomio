/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, type DependencyList } from "react";

export function useAbortableEffect(
  effect: (signal: AbortSignal) => Promise<void>,
  deps: DependencyList,
): void {
  useEffect(() => {
    const controller = new AbortController();
    void effect(controller.signal);
    return () => controller.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}

export function isAbortError(e: unknown): boolean {
  return (
    e instanceof DOMException && e.name === "AbortError"
  );
}
