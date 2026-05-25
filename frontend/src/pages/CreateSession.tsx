/* SPDX-License-Identifier: Apache-2.0 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import { PlusIcon, TrashIcon } from "@primer/octicons-react";

import { RepoKindIcon } from "@/components/RepoKindIcon";

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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import MobileShareCard from "@/components/MobileShareCard";
import { api, type ResolvedPullRequest, type Session } from "@/lib/api";
import { createSessionFromResolved } from "@/lib/createSessionFromPullRequest";
import { formatError } from "@/lib/errors";
import { isGithubPullRequestUrl } from "@/lib/remoteRepoHost";
import { useAbortableEffect } from "@/lib/useAbortableEffect";

const branchSchema = z.object({
  remoteUrl: z.string().min(1, "repository is required"),
  sourceRef: z.string().min(1, "source branch is required"),
  baseRef: z.string().min(1, "base branch is required"),
});

const prSchema = z.object({
  pullRequestUrl: z.string(),
});

type BranchFormValues = z.infer<typeof branchSchema>;
type PrFormValues = z.infer<typeof prSchema>;
type CreateMode = "pr" | "branch";
type SubmitPhase = "idle" | "fetching" | "creating";
type ValidationStatus = "idle" | "pending" | "valid" | "invalid";

type TabValidation = {
  status: ValidationStatus;
  error: string | null;
};

const PR_RESOLVE_DEBOUNCE_MS = 400;
const BRANCH_VALIDATE_DEBOUNCE_MS = 1000;

const emptyValidation = (): TabValidation => ({ status: "idle", error: null });

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
  const groups = useMemo(() => {
    if (!sessions?.length) return [];
    const map = new Map<string, Session[]>();
    for (const s of sessions) {
      const list = map.get(s.normalizedRemote) ?? [];
      list.push(s);
      map.set(s.normalizedRemote, list);
    }
    return Array.from(map.entries());
  }, [sessions]);

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
      <div className="flex flex-col items-center justify-center rounded-lg border border-dashed py-16">
        <p className="text-sm text-muted-foreground">No sessions yet</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {groups.map(([key, rows]) => {
        const sample = rows[0];
        const headerTitle = sample.isLocal ? sample.literalRemote : undefined;
        return (
          <div key={key} className="divide-y rounded-md border">
            <div
              className="flex items-center gap-2 px-3 py-2 text-sm"
              aria-label={`Repository: ${sample.repoName}`}
            >
              <RepoKindIcon
                isLocal={sample.isLocal}
                remoteUrl={sample.literalRemote}
                className="h-4 w-4 shrink-0 text-muted-foreground"
              />
              {sample.repoOwner && (
                <>
                  <span className="truncate text-link">{sample.repoOwner}</span>
                  <span className="text-muted-foreground">/</span>
                </>
              )}
              <span className="truncate font-medium" title={headerTitle}>
                {sample.repoName}
              </span>
            </div>
            {rows.map((s) => (
              <div key={s.id} className="px-3">
                <SessionRow
                  session={s}
                  onContinue={() => onContinue(s.id)}
                  onDeleted={() => onDeleted(s.id)}
                />
              </div>
            ))}
          </div>
        );
      })}
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
  const [mode, setMode] = useState<CreateMode>("pr");
  const [submitting, setSubmitting] = useState(false);
  const [phase, setPhase] = useState<SubmitPhase>("idle");
  const [prValidation, setPrValidation] = useState<TabValidation>(emptyValidation);
  const [branchValidation, setBranchValidation] = useState<TabValidation>(emptyValidation);
  const [resolvedPr, setResolvedPr] = useState<ResolvedPullRequest | null>(null);
  const prGenRef = useRef(0);
  const branchGenRef = useRef(0);
  const prTimerRef = useRef<number | null>(null);
  const branchTimerRef = useRef<number | null>(null);

  const branchForm = useForm<BranchFormValues>({
    resolver: zodResolver(branchSchema),
    defaultValues: { remoteUrl: "", sourceRef: "", baseRef: "origin/main" },
  });

  const prForm = useForm<PrFormValues>({
    resolver: zodResolver(prSchema),
    defaultValues: { pullRequestUrl: "" },
  });

  const pullRequestUrl = prForm.watch("pullRequestUrl");
  const branchRemoteUrl = branchForm.watch("remoteUrl");
  const branchSourceRef = branchForm.watch("sourceRef");
  const branchBaseRef = branchForm.watch("baseRef");

  useAbortableEffect(async (signal) => {
    try {
      const hints = await api.getRepoHints();
      if (signal.aborted) return;
      if (hints.suggestedRemoteUrl && !branchForm.getValues("remoteUrl")) {
        branchForm.setValue("remoteUrl", hints.suggestedRemoteUrl);
      }
      if (hints.suggestedSourceRef && !branchForm.getValues("sourceRef")) {
        branchForm.setValue("sourceRef", hints.suggestedSourceRef);
      }
      if (hints.suggestedBaseRef && branchForm.getValues("baseRef") === "origin/main") {
        branchForm.setValue("baseRef", hints.suggestedBaseRef);
      }
    } catch {
      // Non-fatal; user can fill the field manually.
    }
  }, [branchForm]);

  useEffect(() => {
    if (prTimerRef.current) {
      window.clearTimeout(prTimerRef.current);
      prTimerRef.current = null;
    }

    const trimmed = pullRequestUrl.trim();

    if (trimmed === "") {
      setPrValidation(emptyValidation());
      setResolvedPr(null);
      return;
    }

    if (!isGithubPullRequestUrl(trimmed)) {
      setPrValidation({ status: "invalid", error: null });
      setResolvedPr(null);
      return;
    }

    setResolvedPr(null);
    setPrValidation({ status: "idle", error: null });

    const gen = ++prGenRef.current;
    prTimerRef.current = window.setTimeout(() => {
      void (async () => {
        setPrValidation({ status: "pending", error: null });
        try {
          const resolved = await api.resolvePullRequest(trimmed);
          if (gen !== prGenRef.current) return;
          setResolvedPr(resolved);
          setPrValidation({ status: "valid", error: null });
        } catch (e) {
          if (gen !== prGenRef.current) return;
          setPrValidation({
            status: "invalid",
            error: formatError(e, "Failed to resolve pull request"),
          });
        }
      })();
    }, PR_RESOLVE_DEBOUNCE_MS);

    return () => {
      if (prTimerRef.current) {
        window.clearTimeout(prTimerRef.current);
        prTimerRef.current = null;
      }
    };
  }, [pullRequestUrl]);

  useEffect(() => {
    if (branchTimerRef.current) {
      window.clearTimeout(branchTimerRef.current);
      branchTimerRef.current = null;
    }

    const remoteUrl = branchRemoteUrl.trim();
    const sourceRef = branchSourceRef.trim();
    const baseRef = branchBaseRef.trim();

    if (!remoteUrl || !sourceRef || !baseRef) {
      setBranchValidation(emptyValidation());
      return;
    }

    setBranchValidation({ status: "idle", error: null });

    const gen = ++branchGenRef.current;
    branchTimerRef.current = window.setTimeout(() => {
      void (async () => {
        setBranchValidation({ status: "pending", error: null });
        try {
          await api.validateSession(remoteUrl, baseRef, sourceRef);
          if (gen !== branchGenRef.current) return;
          setBranchValidation({ status: "valid", error: null });
        } catch (e) {
          if (gen !== branchGenRef.current) return;
          setBranchValidation({
            status: "invalid",
            error: formatError(e, "Validation failed"),
          });
        }
      })();
    }, BRANCH_VALIDATE_DEBOUNCE_MS);

    return () => {
      if (branchTimerRef.current) {
        window.clearTimeout(branchTimerRef.current);
        branchTimerRef.current = null;
      }
    };
  }, [branchRemoteUrl, branchSourceRef, branchBaseRef]);

  const createFromRefs = async (remoteUrl: string, baseRef: string, sourceRef: string) => {
    setPhase("fetching");
    const session = await createSessionFromResolved({ remoteUrl, baseRef, sourceRef }, (p) =>
      setPhase(p),
    );
    onCreated(session.id);
  };

  const onBranchSubmit = async (values: BranchFormValues) => {
    if (branchValidation.status !== "valid") return;
    setSubmitting(true);
    try {
      await createFromRefs(values.remoteUrl, values.baseRef, values.sourceRef);
    } catch (e) {
      toast.error(formatError(e, "Failed to create session"));
    } finally {
      setSubmitting(false);
      setPhase("idle");
    }
  };

  const onPrSubmit = async () => {
    if (prValidation.status !== "valid" || !resolvedPr) return;
    setSubmitting(true);
    try {
      await createFromRefs(resolvedPr.remoteUrl, resolvedPr.baseRef, resolvedPr.sourceRef);
    } catch (e) {
      toast.error(formatError(e, "Failed to create session"));
    } finally {
      setSubmitting(false);
      setPhase("idle");
    }
  };

  const submitPhaseLabel =
    phase === "fetching" ? "Fetching…" : phase === "creating" ? "Creating…" : "Create session";

  const prButtonLabel = submitting
    ? submitPhaseLabel
    : prValidation.status === "pending"
      ? "Resolving PR…"
      : "Create session";

  const branchButtonLabel = submitting
    ? submitPhaseLabel
    : branchValidation.status === "pending"
      ? "Validating…"
      : "Create session";

  const showBranchHint =
    pullRequestUrl.trim() !== "" && !isGithubPullRequestUrl(pullRequestUrl);

  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Create session</DialogTitle>
        <DialogDescription>
          Paste a GitHub pull request link, or enter a repository and branch refs manually.
        </DialogDescription>
      </DialogHeader>
      <Tabs
        value={mode}
        onValueChange={(v) => setMode(v as CreateMode)}
        className="w-full"
      >
        <TabsList variant="underline" className="w-full">
          <TabsTrigger variant="underline" value="pr">
            Pull Request
          </TabsTrigger>
          <TabsTrigger variant="underline" value="branch">
            Branch
          </TabsTrigger>
        </TabsList>
        <TabsContent value="pr" className="mt-4">
          <Form {...prForm}>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void onPrSubmit();
              }}
              className="space-y-4"
            >
              <FormField
                control={prForm.control}
                name="pullRequestUrl"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Pull request</FormLabel>
                    <FormControl>
                      <Input
                        placeholder="https://github.com/org/repo/pull/123"
                        {...field}
                      />
                    </FormControl>
                    {showBranchHint && (
                      <p className="text-sm text-destructive">
                        For local repos or git hosts other than GitHub, use the Branch tab.
                      </p>
                    )}
                    {prValidation.error && (
                      <p className="text-sm text-destructive">{prValidation.error}</p>
                    )}
                    <FormMessage />
                  </FormItem>
                )}
              />
              <Button
                type="submit"
                disabled={submitting || prValidation.status !== "valid"}
                className="w-full"
              >
                {mode === "pr" ? prButtonLabel : "Create session"}
              </Button>
            </form>
          </Form>
        </TabsContent>
        <TabsContent value="branch" className="mt-4">
          <Form {...branchForm}>
            <form onSubmit={branchForm.handleSubmit(onBranchSubmit)} className="space-y-4">
              <FormField
                control={branchForm.control}
                name="remoteUrl"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>repository</FormLabel>
                    <FormControl>
                      <Input
                        placeholder="/path/to/repo or https://github.com/org/repo.git"
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={branchForm.control}
                name="sourceRef"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>source branch</FormLabel>
                    <FormControl>
                      <Input placeholder="feature-branch" className="font-mono" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={branchForm.control}
                name="baseRef"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>base branch</FormLabel>
                    <FormControl>
                      <Input placeholder="origin/main" className="font-mono" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              {branchValidation.error && (
                <p className="text-sm text-destructive">{branchValidation.error}</p>
              )}
              <Button
                type="submit"
                disabled={submitting || branchValidation.status !== "valid"}
                className="w-full"
              >
                {mode === "branch" ? branchButtonLabel : "Create session"}
              </Button>
            </form>
          </Form>
        </TabsContent>
      </Tabs>
    </DialogContent>
  );
}
