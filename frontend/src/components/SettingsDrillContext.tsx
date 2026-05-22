import {
  createContext,
  useCallback,
  useContext,
  useState,
  type ReactNode,
} from "react";

export type SettingsCategory =
  | "account"
  | "general"
  | "coordinator"
  | "surveyor"
  | "planner"
  | "constructor";

type SettingsDrillContextValue = {
  activeCategory: SettingsCategory | null;
  setActiveCategory: (category: SettingsCategory | null) => void;
  resetDrill: () => void;
  isDrilledIn: boolean;
};

const noop = () => {};

const defaultValue: SettingsDrillContextValue = {
  activeCategory: null,
  setActiveCategory: noop,
  resetDrill: noop,
  isDrilledIn: false,
};

const SettingsDrillContext =
  createContext<SettingsDrillContextValue>(defaultValue);

export function SettingsDrillProvider({ children }: { children: ReactNode }) {
  const [activeCategory, setActiveCategory] =
    useState<SettingsCategory | null>(null);
  const resetDrill = useCallback(() => setActiveCategory(null), []);

  return (
    <SettingsDrillContext.Provider
      value={{
        activeCategory,
        setActiveCategory,
        resetDrill,
        isDrilledIn: activeCategory !== null,
      }}
    >
      {children}
    </SettingsDrillContext.Provider>
  );
}

export function useSettingsDrill() {
  return useContext(SettingsDrillContext);
}
