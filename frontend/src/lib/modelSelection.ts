import type {
  CursorModel,
  ModelParamDef,
  ModelParamValue,
  ModelSelection,
  ModelVariant,
} from "@/lib/api";

export const DEFAULT_MODEL_SELECTION: ModelSelection = {
  id: "composer-2.5",
  params: [{ id: "fast", value: "true" }],
};

export function paramsEqual(a: ModelParamValue[], b: ModelParamValue[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((p, i) => p.id === b[i]?.id && p.value === b[i]?.value);
}

export function findCatalogModel(
  models: CursorModel[],
  id: string,
): CursorModel | undefined {
  return models.find((m) => m.id === id);
}

export function catalogModelsForPicker(
  models: CursorModel[],
  value: ModelSelection,
): CursorModel[] {
  if (models.some((m) => m.id === value.id)) return models;
  return [{ id: value.id }, ...models];
}

function withDefaultSuffix(label: string, isDefault: boolean | undefined): string {
  return isDefault ? `${label} (default)` : label;
}

function findParamDef(
  catalog: CursorModel | undefined,
  paramId: string,
): ModelParamDef | undefined {
  return catalog?.parameters?.find((d) => d.id === paramId);
}

/** Human-readable value for one parameter, using catalog option labels when present. */
export function paramValueLabel(
  paramId: string,
  value: string,
  catalog: CursorModel | undefined,
): string {
  if (paramId === "fast") {
    return value === "true" ? "Fast" : "Standard";
  }
  const def = findParamDef(catalog, paramId);
  const opt = def?.values.find((v) => v.value === value);
  if (opt?.displayName) return opt.displayName;
  return value;
}

function orderedVariantParams(
  params: ModelParamValue[],
  catalog: CursorModel | undefined,
): ModelParamValue[] {
  const order = catalog?.parameters?.map((d) => d.id) ?? [];
  if (order.length === 0) return params;
  const index = new Map(order.map((id, i) => [id, i]));
  return [...params].sort(
    (a, b) => (index.get(a.id) ?? order.length) - (index.get(b.id) ?? order.length),
  );
}

function formatParamPart(
  param: ModelParamValue,
  catalog: CursorModel | undefined,
  multiParam: boolean,
): string {
  const valueLabel = paramValueLabel(param.id, param.value, catalog);
  if (!multiParam) return valueLabel;
  if (param.id === "fast") return valueLabel;
  const name = findParamDef(catalog, param.id)?.displayName ?? param.id;
  return `${name}: ${valueLabel}`;
}

export function variantLabel(
  variant: ModelVariant,
  catalog: CursorModel | undefined,
): string {
  if (variant.displayName && variant.displayName !== catalog?.displayName) {
    return withDefaultSuffix(variant.displayName, variant.isDefault);
  }

  const ordered = orderedVariantParams(variant.params, catalog);
  if (ordered.length === 0) {
    return withDefaultSuffix("Default", variant.isDefault);
  }

  const multiParam = ordered.length > 1;
  const base = ordered.map((p) => formatParamPart(p, catalog, multiParam)).join(" · ");
  return withDefaultSuffix(base, variant.isDefault);
}

export function defaultSelectionForModel(catalog: CursorModel): ModelSelection {
  const variants = catalog.variants ?? [];
  if (variants.length > 0) {
    const pick = variants.find((v) => v.isDefault) ?? variants[0];
    return { id: catalog.id, params: pick.params.map((p) => ({ ...p })) };
  }
  const parameters = catalog.parameters ?? [];
  if (parameters.length > 0) {
    const params = parameters.map((def) => ({
      id: def.id,
      value: def.values[0]?.value ?? "",
    }));
    return { id: catalog.id, params };
  }
  return { id: catalog.id };
}

export function variantKey(variant: ModelVariant): string {
  return JSON.stringify(variant.params);
}

export function selectionMatchesVariant(
  value: ModelSelection,
  variant: ModelVariant,
): boolean {
  return paramsEqual(value.params ?? [], variant.params);
}

export function findMatchingVariant(
  catalog: CursorModel,
  value: ModelSelection,
): ModelVariant | undefined {
  const variants = catalog.variants ?? [];
  return variants.find((v) => selectionMatchesVariant(value, v));
}
