import { X } from "lucide-react";
import { useSearchParams } from "react-router-dom";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { SESSION_NOT_FOUND_PARAM } from "@/lib/sessionNotFound";

export default function SessionNotFoundBanner() {
  const [searchParams, setSearchParams] = useSearchParams();

  if (!searchParams.has(SESSION_NOT_FOUND_PARAM)) return null;

  const dismiss = () => {
    setSearchParams(
      (prev) => {
        const updated = new URLSearchParams(prev);
        updated.delete(SESSION_NOT_FOUND_PARAM);
        return updated;
      },
      { replace: true },
    );
  };

  return (
    <Alert variant="destructive" className="rounded-none border-x-0 border-t-0 pr-12">
      <AlertTitle>Session not found</AlertTitle>
      <AlertDescription>
        The Session you requested could not be found. It may have been deleted or the link may be
        outdated.
      </AlertDescription>
      <button
        type="button"
        onClick={dismiss}
        className="absolute right-4 top-4 rounded-sm p-1 opacity-80 hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-destructive"
        aria-label="Dismiss session not found"
      >
        <X className="h-4 w-4" />
      </button>
    </Alert>
  );
}
