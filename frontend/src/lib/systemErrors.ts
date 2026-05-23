/* SPDX-License-Identifier: Apache-2.0 */

import { useSyncExternalStore } from "react";

export type SystemError = { code: string; message: string };

let store = new Map<string, string>();
const listeners = new Set<() => void>();

function snapshot(): SystemError[] {
  return Array.from(store, ([code, message]) => ({ code, message }));
}

let cached = snapshot();

function emit() {
  cached = snapshot();
  for (const l of listeners) l();
}

export function registerSystemError(code: string, message: string): void {
  if (store.get(code) === message) return;
  store.set(code, message);
  emit();
}

export function clearSystemError(code: string): void {
  if (!store.has(code)) return;
  store.delete(code);
  emit();
}

export function useSystemErrors(): SystemError[] {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => cached,
    () => cached,
  );
}
