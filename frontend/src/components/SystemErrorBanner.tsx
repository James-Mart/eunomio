import { XIcon } from "@primer/octicons-react";
import { clearSystemError, useSystemErrors } from "@/lib/systemErrors";

export default function SystemErrorBanner() {
  const errors = useSystemErrors();
  if (errors.length === 0) return null;
  return (
    <div className="sticky top-0 z-50 flex flex-col">
      {errors.map((err) => (
        <div
          key={err.code}
          role="alert"
          className="flex items-center justify-between gap-3 bg-destructive px-4 py-2 text-sm text-destructive-foreground"
        >
          <span className="min-w-0 truncate">{err.message}</span>
          <button
            type="button"
            onClick={() => clearSystemError(err.code)}
            className="rounded-sm p-1 opacity-80 hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-destructive-foreground"
            aria-label={`Dismiss ${err.code}`}
          >
            <XIcon className="h-4 w-4" />
          </button>
        </div>
      ))}
    </div>
  );
}
