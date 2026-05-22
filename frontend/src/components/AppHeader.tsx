import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { PersonIcon, SignOutIcon } from "@primer/octicons-react";

import { useAuth } from "@/components/AuthProvider";
import AppSubNav from "@/components/AppSubNav";
import { BrandMark } from "@/components/BrandMark";
import RepoBreadcrumb from "@/components/RepoBreadcrumb";
import SessionHeaderContext from "@/components/SessionHeaderContext";
import { cn } from "@/lib/utils";

export default function AppHeader() {
  return (
    <header className="shrink-0 border-b border-border bg-header">
      <div className="relative flex h-12 items-center gap-3 px-4">
        <Link to="/" aria-label="Eunomio home" className="shrink-0">
          <BrandMark className="text-xl" />
        </Link>
        <div className="h-6 w-px shrink-0 bg-border" aria-hidden="true" />
        <RepoBreadcrumb />
        <div className="pointer-events-none absolute inset-x-0 flex justify-center px-4">
          <SessionHeaderContext />
        </div>
        <div className="min-w-0 flex-1" />
        <UserMenu />
      </div>
      <AppSubNav />
    </header>
  );
}

function UserMenu() {
  const { principal, logout } = useAuth();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open]);

  const onLogout = async () => {
    setOpen(false);
    await logout();
  };

  return (
    <div ref={rootRef} className="relative shrink-0">
      <button
        type="button"
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen((prev) => !prev)}
        className={cn(
          "inline-flex h-8 items-center gap-1.5 rounded-md border border-border px-2 text-sm text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground",
          open && "bg-muted/50 text-foreground",
        )}
      >
        <PersonIcon className="h-4 w-4 shrink-0" aria-hidden="true" />
        <span className="max-w-[10rem] truncate">{principal.username}</span>
      </button>
      {open ? (
        <div
          role="menu"
          className="absolute right-0 top-full z-50 mt-1 min-w-[10rem] overflow-hidden rounded-md border border-border bg-popover py-1 shadow-md"
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => void onLogout()}
            className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-muted-foreground hover:bg-muted/50 hover:text-foreground"
          >
            <SignOutIcon className="h-4 w-4 shrink-0" aria-hidden="true" />
            Sign out
          </button>
        </div>
      ) : null}
    </div>
  );
}
