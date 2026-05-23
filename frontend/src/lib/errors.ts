/* SPDX-License-Identifier: Apache-2.0 */

import { ApiError } from "@/lib/api";

export function formatError(e: unknown, fallback: string): string {
  if (e instanceof ApiError) return e.message;
  if (e instanceof Error) return e.message;
  return fallback;
}
