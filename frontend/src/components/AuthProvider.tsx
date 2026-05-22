import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import Login from "@/pages/Login";
import { Skeleton } from "@/components/ui/skeleton";
import { api, ApiError, type Principal } from "@/lib/api";

type AuthContextValue = {
  principal: Principal;
  logout: () => Promise<void>;
  refresh: () => Promise<void>;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used inside AuthProvider");
  return ctx;
}

type AuthState =
  | { status: "loading" }
  | { status: "unauthenticated" }
  | { status: "authenticated"; principal: Principal };

export function AuthProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AuthState>({ status: "loading" });

  const refresh = useCallback(async () => {
    try {
      const principal = await api.getMe();
      setState({ status: "authenticated", principal });
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) {
        setState({ status: "unauthenticated" });
        return;
      }
      setState({ status: "unauthenticated" });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const logout = useCallback(async () => {
    try {
      await api.logout();
    } finally {
      setState({ status: "unauthenticated" });
    }
  }, []);

  const value = useMemo(
    () =>
      state.status === "authenticated"
        ? { principal: state.principal, logout, refresh }
        : null,
    [state, logout, refresh],
  );

  if (state.status === "loading") {
    return (
      <div className="flex h-dvh items-center justify-center bg-background">
        <Skeleton className="h-8 w-48" />
      </div>
    );
  }

  if (state.status === "unauthenticated") {
    return <Login onSuccess={refresh} />;
  }

  return (
    <AuthContext.Provider value={value!}>{children}</AuthContext.Provider>
  );
}
