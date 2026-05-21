import { lazy, Suspense } from "react";
import { Link, Route, Routes } from "react-router-dom";

import { BrandMark } from "@/components/BrandMark";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import SessionNotFoundBanner from "@/components/SessionNotFoundBanner";
import SystemErrorBanner from "@/components/SystemErrorBanner";
import { Skeleton } from "@/components/ui/skeleton";
import { Toaster } from "@/components/ui/sonner";

const CreateSession = lazy(() => import("@/pages/CreateSession"));
const Session = lazy(() => import("@/pages/Session"));

export default function App() {
  return (
    <div className="min-h-screen flex flex-col">
      <SystemErrorBanner />
      <header className="border-b">
        <div className="flex h-14 items-center px-4">
          <Link to="/" className="flex items-center gap-2 font-semibold">
            <BrandMark className="text-2xl" />
            Eunomia
          </Link>
        </div>
      </header>
      <SessionNotFoundBanner />
      <main className="flex-1">
        <ErrorBoundary>
          <Suspense fallback={<RouteFallback />}>
            <Routes>
              <Route path="/" element={<CreateSession />} />
              <Route path="/sessions/:id" element={<Session />} />
            </Routes>
          </Suspense>
        </ErrorBoundary>
      </main>
      <Toaster richColors position="bottom-right" />
    </div>
  );
}

function RouteFallback() {
  return (
    <div className="container max-w-2xl py-10 space-y-3">
      <Skeleton className="h-8 w-1/3" />
      <Skeleton className="h-4 w-2/3" />
      <Skeleton className="h-4 w-1/2" />
    </div>
  );
}
