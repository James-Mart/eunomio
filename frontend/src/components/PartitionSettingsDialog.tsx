import { useEffect, useState } from "react";
import {
  Code2,
  Construction,
  ListChecks,
  Telescope,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";

import {
  api,
  type CursorModel,
  type HumanInTheLoopSettings,
  type PartitionSettings,
} from "@/lib/api";
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

type Category = "coordinator" | "surveyor" | "planner" | "constructor";

type CategoryMeta = { label: string; icon: LucideIcon };

const CATEGORIES: Record<Category, CategoryMeta> = {
  coordinator: { label: "Coordinator", icon: Workflow },
  surveyor: { label: "Surveyor", icon: Telescope },
  planner: { label: "Planner", icon: ListChecks },
  constructor: { label: "Constructor", icon: Code2 },
};

const ORDER: Category[] = ["coordinator", "surveyor", "planner", "constructor"];

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sessionId: string;
};

type ModelsState =
  | { kind: "loading" }
  | { kind: "success"; models: CursorModel[] };

export default function PartitionSettingsDialog({ open, onOpenChange, sessionId }: Props) {
  const [active, setActive] = useState<Category>("surveyor");
  const [settings, setSettings] = useState<PartitionSettings | null>(null);
  const [models, setModels] = useState<ModelsState>({ kind: "loading" });

  useEffect(() => {
    if (!open) return;
    setSettings(null);
    setModels({ kind: "loading" });
    let cancelled = false;
    void Promise.all([
      api.getPartitionSettings(sessionId).catch(() => null),
      api.listCursorModels().catch(() => null),
    ]).then(([s, m]) => {
      if (cancelled) return;
      if (s) setSettings(s);
      if (m) setModels({ kind: "success", models: m.models });
    });
    return () => {
      cancelled = true;
    };
  }, [open, sessionId]);

  const onModelChange = async (next: string) => {
    if (!settings) return;
    const previous = settings.surveyor.model;
    setSettings({ ...settings, surveyor: { model: next } });
    try {
      const updated = await api.updatePartitionSettings(sessionId, {
        surveyor: { model: next },
      });
      setSettings(updated);
    } catch (e) {
      setSettings({ ...settings, surveyor: { model: previous } });
      toast.error(e instanceof Error ? e.message : "Failed to save model");
    }
  };

  const onHitlChange = async (next: HumanInTheLoopSettings) => {
    if (!settings) return;
    const previous = settings.coordinator.humanInTheLoop;
    setSettings({
      ...settings,
      coordinator: { humanInTheLoop: next },
    });
    try {
      const updated = await api.updatePartitionSettings(sessionId, {
        coordinator: { humanInTheLoop: next },
      });
      setSettings(updated);
    } catch (e) {
      setSettings({
        ...settings,
        coordinator: { humanInTheLoop: previous },
      });
      toast.error(e instanceof Error ? e.message : "Failed to save settings");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[640px] p-0">
        <DialogTitle className="sr-only">Partition settings</DialogTitle>
        <div className="flex h-[420px]">
          <div className="flex-1 p-6 overflow-auto">
            <h3 className="text-lg font-medium mb-4">{CATEGORIES[active].label}</h3>
            {active === "surveyor" ? (
              <SurveyorPanel
                settings={settings}
                models={models}
                onModelChange={onModelChange}
              />
            ) : active === "coordinator" ? (
              <CoordinatorPanel settings={settings} onHitlChange={onHitlChange} />
            ) : (
              <PlaceholderPanel />
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

function SurveyorPanel({
  settings,
  models,
  onModelChange,
}: {
  settings: PartitionSettings | null;
  models: ModelsState;
  onModelChange: (next: string) => void;
}) {
  const selected = settings?.surveyor.model ?? "composer-2";
  const loading = models.kind === "loading";
  const items =
    models.kind === "success"
      ? models.models.some((m) => m.id === selected)
        ? models.models
        : [{ id: selected }, ...models.models]
      : [];

  return (
    <div className="space-y-1.5">
      <Label htmlFor="surveyor-model">Model</Label>
      <Select
        value={selected}
        onValueChange={onModelChange}
        disabled={loading || !settings}
      >
        <SelectTrigger id="surveyor-model">
          <SelectValue placeholder={loading ? "Loading models…" : selected} />
        </SelectTrigger>
        <SelectContent>
          {items.map((m) => (
            <SelectItem key={m.id} value={m.id}>
              {m.id}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function CoordinatorPanel({
  settings,
  onHitlChange,
}: {
  settings: PartitionSettings | null;
  onHitlChange: (next: HumanInTheLoopSettings) => void;
}) {
  const hitl = settings?.coordinator.humanInTheLoop ?? {
    afterSurvey: false,
    afterPlanning: false,
  };
  const disabled = !settings;

  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <h4 className="text-sm font-medium">Human-in-the-loop</h4>
        <div className="space-y-3">
          <HitlRow
            id="hitl-after-survey"
            label="Pause after survey"
            description="Wait for me to review the survey before planning."
            checked={hitl.afterSurvey}
            disabled={disabled}
            onChange={(checked) =>
              onHitlChange({ ...hitl, afterSurvey: checked })
            }
          />
          <HitlRow
            id="hitl-after-planning"
            label="Pause after planning"
            description="Wait for me to review the plan before constructing."
            checked={hitl.afterPlanning}
            disabled={disabled}
            onChange={(checked) =>
              onHitlChange({ ...hitl, afterPlanning: checked })
            }
          />
        </div>
      </section>
    </div>
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

function PlaceholderPanel() {
  return (
    <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
      <Construction className="mr-2 h-4 w-4" aria-hidden="true" />
      <span>No settings available.</span>
    </div>
  );
}
