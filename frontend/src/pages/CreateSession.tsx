import { useCallback, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import { PlusIcon, TrashIcon } from "@primer/octicons-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
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
import MobileShareCard from "@/components/MobileShareCard";
import { api, type Session } from "@/lib/api";
import { formatError } from "@/lib/errors";
import { useAbortableEffect } from "@/lib/useAbortableEffect";

const schema = z.object({
  sourceRef: z.string().min(1, "source is required"),
  baseRef: z.string().min(1, "base is required"),
});

type FormValues = z.infer<typeof schema>;

export default function CreateSession() {
  const navigate = useNavigate();
  const [sessions, setSessions] = useState<Session[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  const handleDeleted = useCallback((id: string) => {
    setSessions((prev) => (prev ? prev.filter((s) => s.id !== id) : prev));
  }, []);

  useAbortableEffect(async (signal) => {
    try {
      const rows = await api.listSessions();
      if (!signal.aborted) setSessions(rows);
    } catch (e) {
      if (!signal.aborted) setListError(formatError(e, "Failed to load sessions"));
    }
  }, []);

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="container max-w-2xl py-10">
      <div className="mb-6">
        <MobileShareCard />
      </div>

      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold tracking-tight">Sessions</h1>
        <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
          <DialogTrigger asChild>
            <Button size="sm" variant="outline">
              <PlusIcon className="h-4 w-4 mr-1" />
              New
            </Button>
          </DialogTrigger>
          {dialogOpen && (
            <CreateSessionDialogContent
              onCreated={(id) => {
                setDialogOpen(false);
                navigate(`/sessions/${id}`);
              }}
            />
          )}
        </Dialog>
      </div>

      <SessionsList
        sessions={sessions}
        error={listError}
        onContinue={(id) => navigate(`/sessions/${id}`)}
        onDeleted={handleDeleted}
      />
      </div>
    </div>
  );
}

function SessionsList({
  sessions,
  error,
  onContinue,
  onDeleted,
}: {
  sessions: Session[] | null;
  error: string | null;
  onContinue: (id: string) => void;
  onDeleted: (id: string) => void;
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
        <SessionRow
          key={s.id}
          session={s}
          onContinue={() => onContinue(s.id)}
          onDeleted={() => onDeleted(s.id)}
        />
      ))}
    </div>
  );
}

const TIMESTAMP_FORMAT = new Intl.DateTimeFormat(undefined, {
  dateStyle: "short",
  timeStyle: "short",
});

function SessionRow({
  session,
  onContinue,
  onDeleted,
}: {
  session: Session;
  onContinue: () => void;
  onDeleted: () => void;
}) {
  const created = TIMESTAMP_FORMAT.format(new Date(session.createdAt * 1000));
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await api.deleteSession(session.id);
      setConfirmOpen(false);
      onDeleted();
    } catch (e) {
      toast.error(formatError(e, "Failed to delete session"));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="flex items-center gap-3 py-3">
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium truncate">{session.sourceRef}</div>
        <div className="text-xs text-muted-foreground">
          from <span className="font-mono break-all">{session.baseRef}</span> · created {created}
        </div>
      </div>
      <Button size="sm" variant="outline" onClick={onContinue} className="shrink-0">
        Continue
      </Button>
      <Dialog open={confirmOpen} onOpenChange={(open) => !deleting && setConfirmOpen(open)}>
        <DialogTrigger asChild>
          <Button
            size="icon"
            variant="ghost"
            className="shrink-0 h-9 w-9 text-muted-foreground hover:text-destructive"
            aria-label="Delete session"
          >
            <TrashIcon className="h-4 w-4" />
          </Button>
        </DialogTrigger>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete session?</DialogTitle>
            <DialogDescription>
              This will permanently delete the session for{" "}
              <span className="font-medium">{session.sourceRef}</span> (from {session.baseRef}),
              including its graph and any in-progress partitions. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline" disabled={deleting}>
                Cancel
              </Button>
            </DialogClose>
            <Button variant="destructive" onClick={handleDelete} disabled={deleting}>
              {deleting ? "Deleting…" : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function CreateSessionDialogContent({ onCreated }: { onCreated: (sessionId: string) => void }) {
  const [submitting, setSubmitting] = useState(false);
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { sourceRef: "", baseRef: "origin/main" },
  });

  useAbortableEffect(async (signal) => {
    try {
      const info = await api.getRepoInfo();
      if (signal.aborted) return;
      if (info.currentBranch && !form.getValues("sourceRef")) {
        form.setValue("sourceRef", info.currentBranch);
      }
    } catch {
      // Non-fatal; user can fill the field manually.
    }
  }, [form]);

  const onSubmit = async (values: FormValues) => {
    setSubmitting(true);
    try {
      const session = await api.createSession(values.baseRef, values.sourceRef);
      onCreated(session.id);
    } catch (e) {
      toast.error(formatError(e, "Failed to create session"));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Create session</DialogTitle>
        <DialogDescription>
          Pick a source ref (the work to review) and a base ref (the merge target). Both must
          exist in the repo this server was started in.
        </DialogDescription>
      </DialogHeader>
      <Form {...form}>
        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <FormField
            control={form.control}
            name="sourceRef"
            render={({ field }) => (
              <FormItem>
                <FormLabel>source</FormLabel>
                <FormControl>
                  <Input placeholder="feature-branch" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="baseRef"
            render={({ field }) => (
              <FormItem>
                <FormLabel>base</FormLabel>
                <FormControl>
                  <Input placeholder="origin/main" {...field} />
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
