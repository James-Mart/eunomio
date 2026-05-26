/* SPDX-License-Identifier: Apache-2.0 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { toast } from "sonner";

import {
  handleTimelineHotkey,
  type TimelineControls,
} from "@/lib/hotkeys";
import { api, type PartitionSettingsPatch } from "@/lib/api";
import { formatError } from "@/lib/errors";

type HotkeysContextValue = {
  enabled: boolean;
  setEnabled: (next: boolean) => void;
  registerTimeline: (id: symbol, controls: TimelineControls | null) => void;
};

const HotkeysContext = createContext<HotkeysContextValue | null>(null);

export function useHotkeys(): HotkeysContextValue {
  const ctx = useContext(HotkeysContext);
  if (!ctx) throw new Error("useHotkeys must be used inside HotkeysProvider");
  return ctx;
}

export function HotkeysProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabledState] = useState(true);
  const timelineRegistrationRef = useRef<{
    id: symbol;
    controls: TimelineControls;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;
    void api.getPartitionSettings().then((settings) => {
      if (!cancelled) setEnabledState(settings.hotkeys.enabled);
    }).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const registerTimeline = useCallback(
    (id: symbol, controls: TimelineControls | null) => {
      if (controls === null) {
        if (timelineRegistrationRef.current?.id === id) {
          timelineRegistrationRef.current = null;
        }
        return;
      }
      timelineRegistrationRef.current = { id, controls };
    },
    [],
  );

  const setEnabled = useCallback((next: boolean) => {
    setEnabledState((previous) => {
      void api
        .updatePartitionSettings(
          { hotkeys: { enabled: next } } as PartitionSettingsPatch,
        )
        .then((settings) => setEnabledState(settings.hotkeys.enabled))
        .catch((error) => {
          setEnabledState(previous);
          toast.error(formatError(error, "Failed to save hotkey settings"));
        });
      return next;
    });
  }, []);

  useEffect(() => {
    if (!enabled) return;
    const onKeyDown = (event: KeyboardEvent) => {
      handleTimelineHotkey(
        event,
        timelineRegistrationRef.current?.controls ?? null,
      );
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [enabled]);

  return (
    <HotkeysContext.Provider value={{ enabled, setEnabled, registerTimeline }}>
      {children}
    </HotkeysContext.Provider>
  );
}
