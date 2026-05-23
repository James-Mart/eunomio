/* SPDX-License-Identifier: Apache-2.0 */

import { lazy, Suspense } from "react";
import { Route, Routes } from "react-router-dom";

import AppHeader from "@/components/AppHeader";
import { AuthProvider } from "@/components/AuthProvider";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { SettingsDrillProvider } from "@/components/SettingsDrillContext";
import SessionNotFoundBanner from "@/components/SessionNotFoundBanner";
import SystemErrorBanner from "@/components/SystemErrorBanner";
import { Skeleton } from "@/components/ui/skeleton";
import { Toaster } from "@/components/ui/sonner";

const CreateSession = lazy(() => import("@/pages/CreateSession"));
const Session = lazy(() => import("@/pages/Session"));
const PartitionSettingsPage = lazy(
  () => import("@/pages/PartitionSettingsPage"),
);

export default function App() {
  return (
    <AuthProvider>
      <AuthenticatedShell />
    </AuthProvider>
  );
}

function AuthenticatedShell() {
  return (
    <SettingsDrillProvider>
      <div className="flex h-dvh min-h-0 flex-col overflow-hidden">
        <SystemErrorBanner />
        <AppHeader />
        <SessionNotFoundBanner />
        <main className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <ErrorBoundary>
            <Suspense fallback={<RouteFallback />}>
              <Routes>
                <Route path="/" element={<CreateSession />} />
                <Route path="/sessions/:id" element={<Session />} />
                <Route path="/settings" element={<PartitionSettingsPage />} />
              </Routes>
            </Suspense>
          </ErrorBoundary>
        </main>
        <Toaster position="bottom-right" />
      </div>
    </SettingsDrillProvider>
  );
}

function RouteFallback() {
  return (
    <div className="container max-w-2xl space-y-3 py-10">
      <Skeleton className="h-8 w-1/3" />
      <Skeleton className="h-4 w-2/3" />
      <Skeleton className="h-4 w-1/2" />
    </div>
  );
}
