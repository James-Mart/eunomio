import { Link } from "react-router-dom";

import AppSubNav from "@/components/AppSubNav";
import { BrandMark } from "@/components/BrandMark";
import RepoBreadcrumb from "@/components/RepoBreadcrumb";
import SessionHeaderContext from "@/components/SessionHeaderContext";

export default function AppHeader() {
  return (
    <header className="shrink-0 border-b border-border bg-header">
      <div className="flex h-12 items-center gap-3 px-4">
        <Link to="/" aria-label="Eunomia home" className="shrink-0">
          <BrandMark className="text-xl" />
        </Link>
        <div className="h-6 w-px shrink-0 bg-border" aria-hidden="true" />
        <RepoBreadcrumb />
        <div className="min-w-0 flex-1" />
        <SessionHeaderContext />
        <div className="w-2 shrink-0" aria-hidden="true" />
      </div>
      <AppSubNav />
    </header>
  );
}
