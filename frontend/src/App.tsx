import { Link, Route, Routes } from "react-router-dom";
import CreateSession from "@/pages/CreateSession";
import Session from "@/pages/Session";
import SystemErrorBanner from "@/components/SystemErrorBanner";
import { Toaster } from "@/components/ui/sonner";

export default function App() {
  return (
    <div className="min-h-screen flex flex-col">
      <SystemErrorBanner />
      <header className="border-b">
        <div className="flex h-14 items-center px-4">
          <Link to="/" className="font-semibold">
            Eunomia
          </Link>
        </div>
      </header>
      <main className="flex-1">
        <Routes>
          <Route path="/" element={<CreateSession />} />
          <Route path="/sessions/:id" element={<Session />} />
        </Routes>
      </main>
      <Toaster richColors position="bottom-right" />
    </div>
  );
}
