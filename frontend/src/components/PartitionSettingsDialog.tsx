import { useState } from "react";
import {
  Code2,
  ListChecks,
  ScrollText,
  Telescope,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";

import {
  api,
  type CursorModel,
  type GeneralSettings,
  type HumanInTheLoopSettings,
  type IterationLimit,
  type PartitionSettings,
  type PartitionSettingsPatch,
} from "@/lib/api";
import { formatError } from "@/lib/errors";
import { useAbortableEffect } from "@/lib/useAbortableEffect";
import { cn } from "@/lib/utils";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type Category = "general" | "coordinator" | "surveyor" | "planner" | "constructor";
type SubagentCategory = "surveyor" | "planner" | "constructor";

type CategoryMeta = { label: string; icon: LucideIcon };

const CATEGORIES: Record<Category, CategoryMeta> = {
  general: { label: "General", icon: ScrollText },
  coordinator: { label: "Coordinator", icon: Workflow },
  surveyor: { label: "Surveyor", icon: Telescope },
  planner: { label: "Planner", icon: ListChecks },
  constructor: { label: "Constructor", icon: Code2 },
};

const ORDER: Category[] = [
  "general",
  "coordinator",
  "surveyor",
  "planner",
  "constructor",
];

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

type ModelsState =
  | { kind: "loading" }
  | { kind: "success"; models: CursorModel[] };

export default function PartitionSettingsDialog({ open, onOpenChange }: Props) {
  const [active, setActive] = useState<Category>("general");
  const [settings, setSettings] = useState<PartitionSettings | null>(null);
  const [models, setModels] = useState<ModelsState>({ kind: "loading" });

  useAbortableEffect(async (signal) => {
    if (!open) return;
    setSettings(null);
    setModels({ kind: "loading" });
    const [s, m] = await Promise.all([
      api.getPartitionSettings().catch(() => null),
      api.listCursorModels().catch(() => null),
    ]);
    if (signal.aborted) return;
    if (s) setSettings(s);
    if (m) setModels({ kind: "success", models: m.models });
  }, [open]);

  const applyOptimistic = async <K extends keyof PartitionSettings>(
    key: K,
    next: PartitionSettings[K],
    errorMessage: string,
  ) => {
    if (!settings) return;
    const previous = settings[key];
    setSettings({ ...settings, [key]: next });
    try {
      const fresh = await api.updatePartitionSettings(
        { [key]: next } as unknown as PartitionSettingsPatch,
      );
      setSettings(fresh);
    } catch (e) {
      setSettings({ ...settings, [key]: previous });
      toast.error(formatError(e, errorMessage));
    }
  };

  const onCoordinatorModelChange = (next: string) =>
    settings &&
    applyOptimistic(
      "coordinator",
      { ...settings.coordinator, model: next },
      "Failed to save model",
    );

  const onHitlChange = (next: HumanInTheLoopSettings) =>
    settings &&
    applyOptimistic(
      "coordinator",
      { ...settings.coordinator, humanInTheLoop: next },
      "Failed to save settings",
    );

  const onMaxIterationsChange = (next: IterationLimit) =>
    settings &&
    applyOptimistic(
      "coordinator",
      { ...settings.coordinator, maxIterations: next },
      "Failed to save settings",
    );

  const updateRoleModel = (role: SubagentCategory, next: string) =>
    settings &&
    applyOptimistic(
      role,
      { ...settings[role], model: next },
      "Failed to save model",
    );

  const updateRoleOverride = (role: SubagentCategory, next: boolean) =>
    settings &&
    applyOptimistic(
      role,
      { ...settings[role], overrideModel: next },
      "Failed to save override",
    );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[640px] p-0">
        <DialogTitle className="sr-only">Partition settings</DialogTitle>
        <div className="flex h-[460px]">
          <div className="flex-1 p-6 overflow-auto">
            <h3 className="text-lg font-medium mb-4">{CATEGORIES[active].label}</h3>
            {active === "general" ? (
              <GeneralPanel
                settings={settings}
                onChange={(next) =>
                  settings &&
                  applyOptimistic("general", next, "Failed to save settings")
                }
              />
            ) : active === "coordinator" ? (
              <CoordinatorPanel
                settings={settings}
                models={models}
                onModelChange={onCoordinatorModelChange}
                onHitlChange={onHitlChange}
                onMaxIterationsChange={onMaxIterationsChange}
              />
            ) : (
              <SubagentPanel
                role={active}
                settings={settings}
                models={models}
                onModelChange={(v) => updateRoleModel(active, v)}
                onOverrideChange={(v) => updateRoleOverride(active, v)}
              />
            )}
          </div>
          <nav
            className="w-14 md:w-44 border-l flex flex-col gap-1 pt-12 pb-3"
            aria-label="Settings categories"
          >
            {ORDER.map((cat) => {
              const meta = CATEGORIES[cat];
              const Icon = meta.icon;
              const isActive = cat === active;
              return (
                <button
                  key={cat}
                  type="button"
                  onClick={() => setActive(cat)}
                  aria-label={meta.label}
                  className={cn(
                    "mx-2 flex items-center gap-2 rounded-md px-2 py-2 text-sm text-left",
                    isActive ? "bg-muted" : "hover:bg-muted/50",
                  )}
                >
                  <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
                  <span className="hidden md:inline">{meta.label}</span>
                </button>
              );
            })}
          </nav>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function GeneralPanel({
  settings,
  onChange,
}: {
  settings: PartitionSettings | null;
  onChange: (next: GeneralSettings) => void;
}) {
  const current = settings?.general ?? { transcriptsEnabled: false };
  const disabled = !settings;
  return (
    <div className="space-y-4">
      <div className="flex items-start gap-3">
        <Checkbox
          id="general-transcripts"
          checked={current.transcriptsEnabled}
          disabled={disabled}
          onChange={(e) =>
            onChange({ ...current, transcriptsEnabled: e.target.checked })
          }
          className="mt-0.5"
        />
        <div className="space-y-0.5">
          <Label htmlFor="general-transcripts" className="font-normal">
            Enable transcripts
          </Label>
          <p className="text-xs text-muted-foreground">
            Subagent output is always captured on the server. This toggle
            controls whether the Transcript section appears on each Partition
            and whether live output streams while a Run is in progress.
          </p>
        </div>
      </div>
    </div>
  );
}

const ITERATION_OPTIONS: { value: string; label: string }[] = [
  { value: "1", label: "1" },
  { value: "2", label: "2" },
  { value: "3", label: "3" },
  { value: "5", label: "5" },
  { value: "10", label: "10" },
  { value: "auto", label: "Auto" },
];

function iterationLimitToOption(limit: IterationLimit | undefined): string {
  if (!limit) return "1";
  if (limit.kind === "auto") return "auto";
  return String(limit.count);
}

function optionToIterationLimit(value: string): IterationLimit {
  if (value === "auto") return { kind: "auto" };
  const n = Number(value);
  return { kind: "count", count: Number.isFinite(n) && n > 0 ? n : 1 };
}

function CoordinatorPanel({
  settings,
  models,
  onModelChange,
  onHitlChange,
  onMaxIterationsChange,
}: {
  settings: PartitionSettings | null;
  models: ModelsState;
  onModelChange: (next: string) => void;
  onHitlChange: (next: HumanInTheLoopSettings) => void;
  onMaxIterationsChange: (next: IterationLimit) => void;
}) {
  const hitl =
    settings?.coordinator.humanInTheLoop ?? {
      afterSurvey: true,
      afterPlanning: true,
      afterConstruct: true,
      afterIndivisible: true,
    };
  const disabled = !settings;
  const selected = settings?.coordinator.model ?? "composer-2";
  const iterations = settings?.coordinator.maxIterations;
  const iterationsOption = iterationLimitToOption(iterations);
  return (
    <div className="space-y-6">
      <section className="space-y-1.5">
        <Label htmlFor="coordinator-model">Default model</Label>
        <ModelSelect
          id="coordinator-model"
          value={selected}
          models={models}
          disabled={disabled}
          onChange={onModelChange}
        />
        <p className="text-xs text-muted-foreground">
          Used for any subagent that doesn't override the model on its own tab.
        </p>
      </section>

      <section className="space-y-3">
        <h4 className="text-sm font-medium">Human-in-the-loop</h4>
        <div className="space-y-3">
          <HitlRow
            id="hitl-after-survey"
            label="Pause after survey"
            description="Wait for me to review the survey before planning."
            checked={hitl.afterSurvey}
            disabled={disabled}
            onChange={(checked) => onHitlChange({ ...hitl, afterSurvey: checked })}
          />
          <HitlRow
            id="hitl-after-planning"
            label="Pause after planning"
            description="Wait for me to review the plan before constructing."
            checked={hitl.afterPlanning}
            disabled={disabled}
            onChange={(checked) => onHitlChange({ ...hitl, afterPlanning: checked })}
          />
          <HitlRow
            id="hitl-after-construct"
            label="Pause after construct"
            description="Wait for me to accept the candidate slice before merging."
            checked={hitl.afterConstruct}
            disabled={disabled}
            onChange={(checked) => onHitlChange({ ...hitl, afterConstruct: checked })}
          />
          <HitlRow
            id="hitl-after-indivisible"
            label="Pause after indivisible verdict"
            description="Wait for me to confirm an indivisible plan before auto-Abandoning."
            checked={hitl.afterIndivisible}
            disabled={disabled}
            onChange={(checked) =>
              onHitlChange({ ...hitl, afterIndivisible: checked })
            }
          />
        </div>
      </section>

      <section className="space-y-1.5">
        <Label htmlFor="coordinator-max-iterations">Max iterations</Label>
        <Select
          value={iterationsOption}
          onValueChange={(v) => onMaxIterationsChange(optionToIterationLimit(v))}
          disabled={disabled}
        >
          <SelectTrigger id="coordinator-max-iterations">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {ITERATION_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>
                {o.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <p className="text-xs text-muted-foreground">
          After Accepting a Partition, automatically Begin two new Partitions
          (one on the new Slice, one on the renamed target). Each iteration
          roughly doubles the work in flight. Stops when the Planner verdicts a
          branch as indivisible, the depth budget runs out, or a Constructor
          blocks.
        </p>
      </section>
    </div>
  );
}

function SubagentPanel({
  role,
  settings,
  models,
  onModelChange,
  onOverrideChange,
}: {
  role: SubagentCategory;
  settings: PartitionSettings | null;
  models: ModelsState;
  onModelChange: (next: string) => void;
  onOverrideChange: (next: boolean) => void;
}) {
  const cur = settings?.[role];
  const enabled = cur?.overrideModel ?? false;
  return (
    <div className="space-y-4">
      <div className="flex items-start gap-3">
        <Checkbox
          id={`${role}-override`}
          checked={enabled}
          disabled={!settings}
          onChange={(e) => onOverrideChange(e.target.checked)}
          className="mt-0.5"
        />
        <div className="space-y-0.5">
          <Label htmlFor={`${role}-override`} className="font-normal">
            Override default model
          </Label>
          <p className="text-xs text-muted-foreground">
            Use a different model for the {role} role than the Coordinator default.
          </p>
        </div>
      </div>
      <div className="space-y-1.5">
        <Label htmlFor={`${role}-model`}>Model</Label>
        <ModelSelect
          id={`${role}-model`}
          value={cur?.model ?? "composer-2"}
          models={models}
          disabled={!settings || !enabled}
          onChange={onModelChange}
        />
      </div>
    </div>
  );
}

function ModelSelect({
  id,
  value,
  models,
  disabled,
  onChange,
}: {
  id: string;
  value: string;
  models: ModelsState;
  disabled: boolean;
  onChange: (next: string) => void;
}) {
  const loading = models.kind === "loading";
  const items =
    models.kind === "success"
      ? models.models.some((m) => m.id === value)
        ? models.models
        : [{ id: value }, ...models.models]
      : [];
  return (
    <Select
      value={value}
      onValueChange={onChange}
      disabled={disabled || loading}
    >
      <SelectTrigger id={id}>
        <SelectValue placeholder={loading ? "Loading models…" : value} />
      </SelectTrigger>
      <SelectContent>
        {items.map((m) => (
          <SelectItem key={m.id} value={m.id}>
            {m.id}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function HitlRow({
  id,
  label,
  description,
  checked,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  description: string;
  checked: boolean;
  disabled: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-start gap-3">
      <Checkbox
        id={id}
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5"
      />
      <div className="space-y-0.5">
        <Label htmlFor={id} className="font-normal">
          {label}
        </Label>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
    </div>
  );
}
