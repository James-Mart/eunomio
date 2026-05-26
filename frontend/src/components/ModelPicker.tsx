/* SPDX-License-Identifier: Apache-2.0 */

import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { CursorModel, ModelSelection } from "@/lib/api";
import {
  catalogModelsForPicker,
  defaultSelectionForModel,
  findCatalogModel,
  findMatchingVariant,
  variantKey,
  variantLabel,
} from "@/lib/modelSelection";

type ModelPickerProps = {
  idPrefix: string;
  value: ModelSelection;
  models: CursorModel[];
  disabled: boolean;
  loading: boolean;
  onChange: (next: ModelSelection) => void;
};

export default function ModelPicker({
  idPrefix,
  value,
  models,
  disabled,
  loading,
  onChange,
}: ModelPickerProps) {
  const items = catalogModelsForPicker(models, value);
  const catalog = findCatalogModel(models, value.id);
  const variants = catalog?.variants ?? [];
  const parameters = catalog?.parameters ?? [];
  const showVariants = variants.length > 0;
  const showParameterControls = !showVariants && parameters.length > 0;

  const onModelIdChange = (nextId: string) => {
    const nextCatalog = findCatalogModel(models, nextId);
    if (!nextCatalog) {
      onChange({ id: nextId });
      return;
    }
    onChange(defaultSelectionForModel(nextCatalog));
  };

  const onVariantChange = (key: string) => {
    const variant = variants.find((v) => variantKey(v) === key);
    if (!variant || !catalog) return;
    onChange({ id: catalog.id, params: variant.params.map((p) => ({ ...p })) });
  };

  const onParamChange = (paramId: string, paramValue: string) => {
    const existing = value.params ?? [];
    const rest = existing.filter((p) => p.id !== paramId);
    onChange({
      id: value.id,
      params: [...rest, { id: paramId, value: paramValue }],
    });
  };

  const selectedVariantKey = catalog
    ? (findMatchingVariant(catalog, value) ?? variants[0])
    : undefined;

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <Label htmlFor={`${idPrefix}-model`}>Model</Label>
        <Select
          value={value.id}
          onValueChange={onModelIdChange}
          disabled={disabled || loading}
        >
          <SelectTrigger id={`${idPrefix}-model`}>
            <SelectValue placeholder={loading ? "Loading models…" : value.id} />
          </SelectTrigger>
          <SelectContent>
            {items.map((m) => (
              <SelectItem key={m.id} value={m.id}>
                {m.displayName ?? m.id}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {showVariants && catalog && (
        <div className="space-y-1.5">
          <Label htmlFor={`${idPrefix}-variant`}>Variant</Label>
          <Select
            value={
              selectedVariantKey ? variantKey(selectedVariantKey) : variantKey(variants[0])
            }
            onValueChange={onVariantChange}
            disabled={disabled || loading}
          >
            <SelectTrigger id={`${idPrefix}-variant`}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {variants.map((v) => (
                <SelectItem key={variantKey(v)} value={variantKey(v)}>
                  {variantLabel(v, catalog)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}

      {showParameterControls &&
        parameters.map((def) => {
          const current = (value.params ?? []).find((p) => p.id === def.id)?.value;
          const selected = current ?? def.values[0]?.value ?? "";
          return (
            <div key={def.id} className="space-y-1.5">
              <Label htmlFor={`${idPrefix}-param-${def.id}`}>
                {def.displayName ?? def.id}
              </Label>
              <Select
                value={selected}
                onValueChange={(v) => onParamChange(def.id, v)}
                disabled={disabled || loading}
              >
                <SelectTrigger id={`${idPrefix}-param-${def.id}`}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {def.values.map((opt) => (
                    <SelectItem key={opt.value} value={opt.value}>
                      {opt.displayName ?? opt.value}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          );
        })}
    </div>
  );
}
