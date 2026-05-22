import { NavLink, useLocation } from "react-router-dom";

import { useSettingsDrill } from "@/components/SettingsDrillContext";
import { useIsDesktop } from "@/lib/useIsDesktop";
import { cn } from "@/lib/utils";

function subnavLinkClass(isActive: boolean) {
  return cn(
    "inline-flex items-center justify-center whitespace-nowrap text-sm font-medium relative -mb-px rounded-none border-b-2 border-transparent px-3 py-2.5 sm:px-1 sm:pb-2 sm:pt-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
    isActive
      ? "border-attention text-foreground"
      : "text-muted-foreground hover:text-foreground",
  );
}

function isSessionRoute(pathname: string) {
  return pathname === "/" || pathname.startsWith("/sessions/");
}

function useCodePath() {
  const location = useLocation();
  if (location.pathname === "/settings") {
    return (location.state as { codePath?: string } | null)?.codePath ?? "/";
  }
  if (isSessionRoute(location.pathname)) {
    return location.pathname;
  }
  return "/";
}

export default function AppSubNav() {
  const location = useLocation();
  const codePath = useCodePath();
  const isDesktop = useIsDesktop();
  const { resetDrill, isDrilledIn } = useSettingsDrill();

  const settingsCodePath = isSessionRoute(location.pathname)
    ? location.pathname
    : (location.state as { codePath?: string } | null)?.codePath ?? "/";

  const handleSettingsClick = (event: React.MouseEvent) => {
    if (!isDesktop && location.pathname === "/settings" && isDrilledIn) {
      event.preventDefault();
      resetDrill();
    }
  };

  return (
    <nav
      aria-label="App sections"
      className="flex h-10 items-center gap-4 bg-header px-3 sm:px-4"
    >
      <NavLink
        to={codePath}
        end={codePath === "/"}
        className={() =>
          subnavLinkClass(isSessionRoute(location.pathname))
        }
      >
        Session
      </NavLink>
      <NavLink
        to="/settings"
        state={{ codePath: settingsCodePath }}
        onClick={handleSettingsClick}
        className={() =>
          subnavLinkClass(location.pathname === "/settings")
        }
      >
        Settings
      </NavLink>
    </nav>
  );
}
