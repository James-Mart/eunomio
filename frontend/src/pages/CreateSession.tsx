import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Skeleton } from "@/components/ui/skeleton";
import PublicUrlCard from "@/components/PublicUrlCard";
import { api, type Session } from "@/lib/api";

const schema = z.object({
  baseRef: z.string().min(1, "baseRef is required"),
  sourceRef: z.string().min(1, "sourceRef is required"),
});

type FormValues = z.infer<typeof schema>;

export default function CreateSession() {
  const navigate = useNavigate();
  const [sessions, setSessions] = useState<Session[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .listSessions()
      .then((rows) => {
        if (!cancelled) setSessions(rows);
      })
      .catch((e) => {
        if (!cancelled) setListError(e instanceof Error ? e.message : "Failed to load sessions");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="container max-w-2xl py-10">
      <div className="mb-6">
        <PublicUrlCard />
      </div>

      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold tracking-tight">Sessions</h1>
        <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
          <DialogTrigger asChild>
            <Button size="sm">
              <Plus className="h-4 w-4 mr-1" />
              New
            </Button>
          </DialogTrigger>
          <CreateSessionDialogContent
            onCreated={(id) => {
              setDialogOpen(false);
              navigate(`/sessions/${id}`);
            }}
          />
        </Dialog>
      </div>

      <SessionsList sessions={sessions} error={listError} onContinue={(id) => navigate(`/sessions/${id}`)} />
    </div>
  );
}

function SessionsList({
  sessions,
  error,
  onContinue,
}: {
  sessions: Session[] | null;
  error: string | null;
  onContinue: (id: string) => void;
}) {
  if (error) {
    return <p className="text-sm text-destructive">{error}</p>;
  }
  if (sessions === null) {
    return (
      <div className="divide-y">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="flex items-center gap-3 py-3">
            <div className="flex-1 space-y-2">
              <Skeleton className="h-4 w-1/3" />
              <Skeleton className="h-3 w-1/2" />
            </div>
            <Skeleton className="h-8 w-20" />
          </div>
        ))}
      </div>
    );
  }
  if (sessions.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        No sessions yet — click + New to create one.
      </p>
    );
  }
  return (
    <div className="divide-y">
      {sessions.map((s) => (
        <SessionRow key={s.id} session={s} onContinue={() => onContinue(s.id)} />
      ))}
    </div>
  );
}

const TIMESTAMP_FORMAT = new Intl.DateTimeFormat(undefined, {
  dateStyle: "short",
  timeStyle: "short",
});

function SessionRow({ session, onContinue }: { session: Session; onContinue: () => void }) {
  const created = TIMESTAMP_FORMAT.format(new Date(session.createdAt * 1000));
  return (
    <div className="flex items-center gap-3 py-3">
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium truncate">{session.sourceRef}</div>
        <div className="text-xs text-muted-foreground">
          from <span className="break-all">{session.baseRef}</span> · created {created}
        </div>
      </div>
      <Button size="sm" onClick={onContinue} className="shrink-0">
        Continue
      </Button>
    </div>
  );
}

function CreateSessionDialogContent({ onCreated }: { onCreated: (sessionId: string) => void }) {
  const [submitting, setSubmitting] = useState(false);
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { baseRef: "main", sourceRef: "" },
  });

  const onSubmit = async (values: FormValues) => {
    setSubmitting(true);
    try {
      const session = await api.createSession(values.baseRef, values.sourceRef);
      onCreated(session.id);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to create session");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Create session</DialogTitle>
        <DialogDescription>
          Pick a base ref (the merge target) and a source ref (the work to review). Both must
          exist in the repo this server was started in.
        </DialogDescription>
      </DialogHeader>
      <Form {...form}>
        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <FormField
            control={form.control}
            name="baseRef"
            render={({ field }) => (
              <FormItem>
                <FormLabel>baseRef</FormLabel>
                <FormControl>
                  <Input placeholder="main" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="sourceRef"
            render={({ field }) => (
              <FormItem>
                <FormLabel>sourceRef</FormLabel>
                <FormControl>
                  <Input placeholder="feature-branch" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <Button type="submit" disabled={submitting} className="w-full">
            {submitting ? "Creating…" : "Create session"}
          </Button>
        </form>
      </Form>
    </DialogContent>
  );
}
