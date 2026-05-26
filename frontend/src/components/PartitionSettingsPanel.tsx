/* SPDX-License-Identifier: Apache-2.0 */

import { useEffect, useState } from "react";
import {
  ArrowLeftIcon,
  ChevronRightIcon,
  CodeIcon,
  CommandPaletteIcon,
  FileIcon,
  KeyIcon,
  ProjectRoadmapIcon,
  SearchIcon,
  TasklistIcon,
  type IconProps,
} from "@primer/octicons-react";
import { toast } from "sonner";

import {
  useSettingsDrill,
  type SettingsCategory,
} from "@/components/SettingsDrillContext";
import { useHotkeys } from "@/components/HotkeysProvider";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
import { TIMELINE_HOTKEY_BINDINGS } from "@/lib/hotkeys";
import { useAbortableEffect } from "@/lib/useAbortableEffect";
import { useIsDesktop } from "@/lib/useIsDesktop";
import { cn } from "@/lib/utils";

type SubagentCategory = "surveyor" | "planner" | "constructor" | "shaver";

type IconComponent = React.ComponentType<IconProps>;

type CategoryMeta = { label: string; icon: IconComponent };

const CATEGORIES: Record<SettingsCategory, CategoryMeta> = {
  account: { label: "Account", icon: KeyIcon },
  general: { label: "General", icon: FileIcon },
  hotkeys: { label: "Hotkeys", icon: CommandPaletteIcon },
  coordinator: { label: "Coordinator", icon: ProjectRoadmapIcon },
  surveyor: { label: "Surveyor", icon: SearchIcon },
  planner: { label: "Planner", icon: TasklistIcon },
  constructor: { label: "Constructor", icon: CodeIcon },
  shaver: { label: "Timeline", icon: TasklistIcon },
};

const TOP_ORDER: SettingsCategory[] = ["general", "hotkeys", "coordinator", "account"];
const SUBAGENT_ORDER: SubagentCategory[] = [
  "surveyor",
  "planner",
  "constructor",
  "shaver",
];

type ModelsState =
  | { kind: "loading" }
  | { kind: "success"; models: CursorModel[] };

export default function PartitionSettingsPanel() {
  const isDesktop = useIsDesktop();
  const { activeCategory, setActiveCategory, resetDrill } = useSettingsDrill();
  const [desktopActive, setDesktopActive] =
    useState<SettingsCategory>("general");
  const [settings, setSettings] = useState<PartitionSettings | null>(null);
  const [models, setModels] = useState<ModelsState>({ kind: "loading" });

  useEffect(() => {
    resetDrill();
  }, [resetDrill]);

  useAbortableEffect(async (signal) => {
    setSettings(null);
    setModels({ kind: "loading" });

    void (async () => {
      try {
        const s = await api.getPartitionSettings();
        if (!signal.aborted) setSettings(s);
      } catch {
        if (!signal.aborted) setSettings(null);
      }
    })();

    void (async () => {
      try {
        const m = await api.listCursorModels();
        if (!signal.aborted) setModels({ kind: "success", models: m.models });
      } catch {
        if (!signal.aborted) setModels({ kind: "success", models: [] });
      }
    })();
  }, []);

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

  const onSurveyorEnabledChange = (next: boolean) =>
    settings &&
    applyOptimistic(
      "coordinator",
      { ...settings.coordinator, surveyorEnabled: next },
      "Failed to save settings",
    );

  const onTimelineEnabledChange = (next: boolean) =>
    settings &&
    applyOptimistic(
      "coordinator",
      { ...settings.coordinator, timelineEnabled: next },
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

  const renderPanel = (category: SettingsCategory) => {
    if (category === "account") {
      return <AccountPanel models={models} />;
    }
    if (category === "general") {
      return (
        <GeneralPanel
          settings={settings}
          onChange={(next) =>
            settings &&
            applyOptimistic("general", next, "Failed to save settings")
          }
        />
      );
    }
    if (category === "hotkeys") {
      return <HotkeysPanel />;
    }
    if (category === "coordinator") {
      return (
        <CoordinatorPanel
          settings={settings}
          models={models}
          onModelChange={onCoordinatorModelChange}
          onSurveyorEnabledChange={onSurveyorEnabledChange}
          onTimelineEnabledChange={onTimelineEnabledChange}
          onHitlChange={onHitlChange}
          onMaxIterationsChange={onMaxIterationsChange}
        />
      );
    }
    return (
      <SubagentPanel
        role={category}
        settings={settings}
        models={models}
        onModelChange={(v) => updateRoleModel(category, v)}
        onOverrideChange={(v) => updateRoleOverride(category, v)}
      />
    );
  };

  if (!isDesktop && activeCategory === null) {
    return (
      <MobileCategoryIndex onSelect={setActiveCategory} />
    );
  }

  if (!isDesktop && activeCategory !== null) {
    return (
      <MobileCategoryDetail
        category={activeCategory}
        onBack={() => setActiveCategory(null)}
      >
        {renderPanel(activeCategory)}
      </MobileCategoryDetail>
    );
  }

  return (
    <div className="flex min-h-0 flex-1">
      <CategorySidebar active={desktopActive} onSelect={setDesktopActive} />
      <main className="min-w-0 flex-1 overflow-auto">
        <div className="mx-auto max-w-6xl px-4 py-8">
          <h1 className="mb-6 border-b border-border pb-4 text-2xl font-semibold">
            {CATEGORIES[desktopActive].label}
          </h1>
          {renderPanel(desktopActive)}
        </div>
      </main>
    </div>
  );
}

function CategorySidebar({
  active,
  onSelect,
}: {
  active: SettingsCategory;
  onSelect: (category: SettingsCategory) => void;
}) {
  return (
    <nav
      aria-label="Settings categories"
      className="w-56 shrink-0 border-r border-border py-4"
    >
      {TOP_ORDER.map((cat) => (
        <SidebarItem
          key={cat}
          category={cat}
          active={active}
          onSelect={onSelect}
        />
      ))}
      <div className="px-3 pb-1 pt-4 text-xs font-semibold text-muted-foreground">
        Subagents
      </div>
      {SUBAGENT_ORDER.map((cat) => (
        <SidebarItem
          key={cat}
          category={cat}
          active={active}
          onSelect={onSelect}
        />
      ))}
    </nav>
  );
}

function SidebarItem({
  category,
  active,
  onSelect,
}: {
  category: SettingsCategory;
  active: SettingsCategory;
  onSelect: (category: SettingsCategory) => void;
}) {
  const meta = CATEGORIES[category];
  const Icon = meta.icon;
  const isActive = category === active;

  return (
    <button
      type="button"
      onClick={() => onSelect(category)}
      className={cn(
        "flex w-full items-center gap-2 border-l-2 px-3 py-2 text-left text-sm",
        isActive
          ? "border-attention bg-muted text-foreground"
          : "border-transparent text-muted-foreground hover:bg-muted/50 hover:text-foreground",
      )}
    >
      <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
      {meta.label}
    </button>
  );
}

function MobileCategoryIndex({
  onSelect,
}: {
  onSelect: (category: SettingsCategory) => void;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="px-4 py-6">
        <h1 className="mb-2 text-2xl font-semibold">Settings</h1>
        <div className="mt-4">
          {TOP_ORDER.map((cat) => (
            <MobileCategoryRow
              key={cat}
              category={cat}
              onSelect={onSelect}
            />
          ))}
          <div className="px-0 pb-1 pt-4 text-xs font-semibold text-muted-foreground">
            Subagents
          </div>
          {SUBAGENT_ORDER.map((cat) => (
            <MobileCategoryRow
              key={cat}
              category={cat}
              onSelect={onSelect}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function MobileCategoryRow({
  category,
  onSelect,
}: {
  category: SettingsCategory;
  onSelect: (category: SettingsCategory) => void;
}) {
  const meta = CATEGORIES[category];
  const Icon = meta.icon;

  return (
    <button
      type="button"
      onClick={() => onSelect(category)}
      className="flex w-full items-center gap-3 border-b border-border py-3 text-left"
    >
      <Icon className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span className="flex-1 text-sm">{meta.label}</span>
      <ChevronRightIcon
        className="h-4 w-4 shrink-0 text-muted-foreground"
        aria-hidden="true"
      />
    </button>
  );
}

function MobileCategoryDetail({
  category,
  onBack,
  children,
}: {
  category: SettingsCategory;
  onBack: () => void;
  children: React.ReactNode;
}) {
  const meta = CATEGORIES[category];

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="sticky top-0 z-10 flex shrink-0 items-center gap-3 border-b border-border bg-background px-4 py-3">
        <button
          type="button"
          onClick={onBack}
          aria-label="Back to settings"
          className="inline-flex items-center text-link"
        >
          <ArrowLeftIcon className="h-4 w-4" aria-hidden="true" />
        </button>
        <span className="text-sm font-medium text-foreground">{meta.label}</span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-4 py-6">{children}</div>
    </div>
  );
}

function AccountPanel({ models }: { models: ModelsState }) {
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const keyConfigured = models.kind === "success";

  const save = async () => {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      toast.error("Cursor API key is required");
      return;
    }
    setSaving(true);
    try {
      await api.patchCredentials({ cursorApiKey: trimmed });
      setApiKey("");
      toast.success("Cursor API key updated");
    } catch (e) {
      toast.error(formatError(e, "Failed to update API key"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-6">
      <section className="space-y-1.5">
        <h4 className="text-sm font-medium">Cursor API key</h4>
        <p className="text-xs text-muted-foreground">
          {keyConfigured
            ? "A Cursor API key is configured for your account."
            : "No Cursor API key is configured yet. Add one to enable model selection and agent runs."}
        </p>
      </section>

      <section className="space-y-1.5">
        <Label htmlFor="account-cursor-api-key">Update API key</Label>
        <Input
          id="account-cursor-api-key"
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          autoComplete="new-password"
          placeholder="Enter a new Cursor API key"
        />
        <Button
          type="button"
          variant="outline"
          disabled={saving || !apiKey.trim()}
          onClick={() => void save()}
        >
          {saving ? "Saving…" : "Save API key"}
        </Button>
      </section>
    </div>
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
  onSurveyorEnabledChange,
  onTimelineEnabledChange,
  onHitlChange,
  onMaxIterationsChange,
}: {
  settings: PartitionSettings | null;
  models: ModelsState;
  onModelChange: (next: string) => void;
  onSurveyorEnabledChange: (next: boolean) => void;
  onTimelineEnabledChange: (next: boolean) => void;
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
  const surveyorEnabled = settings?.coordinator.surveyorEnabled ?? true;
  const timelineEnabled = settings?.coordinator.timelineEnabled ?? true;
  const selected = settings?.coordinator.model ?? "composer-2.5";
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
        <div className="flex items-start gap-3">
          <Checkbox
            id="coordinator-surveyor-enabled"
            checked={surveyorEnabled}
            disabled={disabled}
            onChange={(e) => onSurveyorEnabledChange(e.target.checked)}
            className="mt-0.5"
          />
          <div className="space-y-0.5">
            <Label htmlFor="coordinator-surveyor-enabled" className="font-normal">
              Enable surveyor
            </Label>
            <p className="text-xs text-muted-foreground">
              Run a separate Survey phase before Plan. When off, the planner
              surveys the diff inline.
            </p>
          </div>
        </div>
        <div className="flex items-start gap-3">
          <Checkbox
            id="coordinator-timeline-enabled"
            checked={timelineEnabled}
            disabled={disabled}
            onChange={(e) => onTimelineEnabledChange(e.target.checked)}
            className="mt-0.5"
          />
          <div className="space-y-0.5">
            <Label htmlFor="coordinator-timeline-enabled" className="font-normal">
              Timeline
            </Label>
            <p className="text-xs text-muted-foreground">
              Generate a visually inspectable history of changes tied to each
              finished Edge.
            </p>
          </div>
        </div>
      </section>

      <section className="space-y-3">
        <h4 className="text-sm font-medium">Human-in-the-loop</h4>
        <div className="space-y-3">
          <HitlRow
            id="hitl-after-survey"
            label="Pause after survey"
            description="Wait for me to review the survey before planning."
            checked={hitl.afterSurvey}
            disabled={disabled || !surveyorEnabled}
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
            description="Wait for me to confirm an indivisible plan before finishing."
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
  const roleLabel = CATEGORIES[role].label;
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
            Use a different model for {roleLabel} than the Coordinator default.
          </p>
        </div>
      </div>
      <div className="space-y-1.5">
        <Label htmlFor={`${role}-model`}>Model</Label>
        <ModelSelect
          id={`${role}-model`}
          value={cur?.model ?? "composer-2.5"}
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

function HotkeysPanel() {
  const { enabled, setEnabled } = useHotkeys();

  return (
    <div className="space-y-6">
      <div className="flex items-start gap-3">
        <Checkbox
          id="hotkeys-enabled"
          checked={enabled}
          onChange={(e) => setEnabled(e.target.checked)}
          className="mt-0.5"
        />
        <div className="space-y-0.5">
          <Label htmlFor="hotkeys-enabled" className="font-normal">
            Enable keyboard shortcuts
          </Label>
          <p className="text-xs text-muted-foreground">
            Timeline shortcuts apply only when a timeline is visible in the
            diff pane. This is separate from the coordinator Timeline setting.
          </p>
        </div>
      </div>

      <section className="space-y-3">
        <h4 className="text-sm font-medium">Timeline</h4>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b text-left text-xs text-muted-foreground">
              <th className="pb-2 pr-4 font-medium">Action</th>
              <th className="pb-2 font-medium">Shortcut</th>
            </tr>
          </thead>
          <tbody>
            {TIMELINE_HOTKEY_BINDINGS.map((binding) => (
              <tr key={binding.id} className="border-b border-border/50">
                <td className="py-2 pr-4">{binding.label}</td>
                <td className="py-2 font-mono text-xs">{binding.keys}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
