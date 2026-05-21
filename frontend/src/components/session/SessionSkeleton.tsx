import { Skeleton } from "@/components/ui/skeleton";

export function SessionSkeleton() {
  return (
    <>
      <div className="hidden md:grid grid-cols-[7fr_3fr] h-[calc(100vh-3.5rem)] gap-2 p-2">
        <Skeleton className="h-full" />
        <div className="grid grid-rows-2 gap-2">
          <Skeleton className="h-full" />
          <Skeleton className="h-full" />
        </div>
      </div>
      <div className="md:hidden flex h-[calc(100dvh-3.5rem)] flex-col gap-2 p-2">
        <Skeleton className="flex-1" />
        <Skeleton className="h-16 shrink-0" />
      </div>
    </>
  );
}
