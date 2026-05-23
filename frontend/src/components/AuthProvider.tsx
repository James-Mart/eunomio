/* SPDX-License-Identifier: Apache-2.0 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import LaunchPullRequestBootstrap from "@/components/LaunchPullRequestBootstrap";
import Login from "@/pages/Login";
import { Skeleton } from "@/components/ui/skeleton";
import { api, ApiError, type Principal } from "@/lib/api";

type AuthContextValue = {
  principal: Principal;
  logout: () => Promise<void>;
  refresh: () => Promise<void>;
  pendingLaunchPullRequestUrl: string | null;
  clearPendingLaunchPullRequest: () => void;
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
  const pendingLaunchPullRequestRef = useRef<string | null>(null);
  const [pendingLaunchPullRequestUrl, setPendingLaunchPullRequestUrl] = useState<string | null>(
    null,
  );
  const [loginPullRequestUrl, setLoginPullRequestUrl] = useState<string | null>(null);

  const clearPendingLaunchPullRequest = useCallback(() => {
    pendingLaunchPullRequestRef.current = null;
    setPendingLaunchPullRequestUrl(null);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const launch = await api.consumeLaunchPullRequest();
      if (launch.pullRequestUrl) {
        pendingLaunchPullRequestRef.current = launch.pullRequestUrl;
      }
    } catch {
      // Non-fatal if launch intent is unavailable.
    }

    try {
      const principal = await api.getMe();
      setLoginPullRequestUrl(null);
      setPendingLaunchPullRequestUrl(pendingLaunchPullRequestRef.current);
      setState({ status: "authenticated", principal });
    } catch (e) {
      setPendingLaunchPullRequestUrl(null);
      setLoginPullRequestUrl(pendingLaunchPullRequestRef.current);
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
      clearPendingLaunchPullRequest();
      setLoginPullRequestUrl(null);
      setState({ status: "unauthenticated" });
    }
  }, [clearPendingLaunchPullRequest]);

  const value = useMemo(
    () =>
      state.status === "authenticated"
        ? {
            principal: state.principal,
            logout,
            refresh,
            pendingLaunchPullRequestUrl,
            clearPendingLaunchPullRequest,
          }
        : null,
    [state, logout, refresh, pendingLaunchPullRequestUrl, clearPendingLaunchPullRequest],
  );

  if (state.status === "loading") {
    return (
      <div className="flex h-dvh items-center justify-center bg-background">
        <Skeleton className="h-8 w-48" />
      </div>
    );
  }

  if (state.status === "unauthenticated") {
    return <Login onSuccess={refresh} pendingPullRequestUrl={loginPullRequestUrl} />;
  }

  return (
    <AuthContext.Provider value={value!}>
      <LaunchPullRequestBootstrap />
      {children}
    </AuthContext.Provider>
  );
}
