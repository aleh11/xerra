import React, { useState } from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { ArrowDown, ArrowUp, Trash2 } from "lucide-react";
import { commands, type AppStyleRule } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { Dropdown } from "../../ui/Dropdown";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import { DebouncedInput } from "./DebouncedInput";
import { useCleanupSettings } from "./useCleanupSettings";
import {
  fieldOptions,
  isCleanupStyle,
  isMatchField,
  styleOptions,
} from "./options";

const iconButton =
  "p-1.5 rounded-md border border-mid-gray/30 hover:bg-logo-primary/20 hover:border-logo-primary disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:border-mid-gray/30 cursor-pointer transition-colors";

export const AppStyleRules: React.FC = () => {
  const { t } = useTranslation();
  const { cleanup, save } = useCleanupSettings();
  const rules = cleanup.app_style_rules ?? [];
  const styles = styleOptions(t);
  const fields = fieldOptions(t);

  const [resetting, setResetting] = useState(false);
  const reset = async () => {
    setResetting(true);
    try {
      const result = await commands.resetCleanupStyleRules();
      if (result.status === "error") {
        toast.error(
          t("settings.cleanup.appStyles.resetError", { error: result.error }),
        );
      }
    } catch (error) {
      toast.error(
        t("settings.cleanup.appStyles.resetError", { error: String(error) }),
      );
    } finally {
      await useSettingsStore.getState().refreshSettings();
      setResetting(false);
    }
  };
  const setRules = (next: AppStyleRule[]) => save({ app_style_rules: next });
  const updateRule = (index: number, patch: Partial<AppStyleRule>) =>
    save((current) => ({
      app_style_rules: (current.app_style_rules ?? []).map((r, i) =>
        i === index ? { ...r, ...patch } : r,
      ),
    }));
  const move = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= rules.length) return;
    const next = [...rules];
    [next[index], next[target]] = [next[target], next[index]];
    setRules(next);
  };

  return (
    <>
      <SettingContainer
        title={t("settings.cleanup.defaultStyle.title")}
        description={t("settings.cleanup.defaultStyle.description")}
        descriptionMode="tooltip"
        layout="horizontal"
        grouped={true}
      >
        <Dropdown
          options={styles}
          selectedValue={cleanup.default_style ?? "Neutral"}
          onSelect={(v) => {
            if (isCleanupStyle(v)) save({ default_style: v });
          }}
        />
      </SettingContainer>
      <SettingContainer
        title={t("settings.cleanup.appStyles.title")}
        description={t("settings.cleanup.appStyles.description")}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3">
          <p className="text-xs text-mid-gray">
            {t("settings.cleanup.appStyles.firstMatchNote")}
          </p>
          {rules.length === 0 && (
            <p className="text-sm text-mid-gray">
              {t("settings.cleanup.appStyles.empty")}
            </p>
          )}
          {rules.map((rule, index) => (
            <div
              key={index}
              className="flex flex-wrap items-center gap-2 p-2 rounded-md border border-mid-gray/20"
            >
              <span className="text-xs text-mid-gray w-5 text-center">
                {index + 1}
              </span>
              <DebouncedInput
                type="text"
                variant="compact"
                className="flex-1 min-w-[140px]"
                aria-label={t("settings.cleanup.appStyles.pattern")}
                placeholder={t("settings.cleanup.appStyles.patternPlaceholder")}
                value={rule.pattern}
                onCommit={(pattern) => updateRule(index, { pattern })}
              />
              <Dropdown
                className="w-40"
                options={fields}
                selectedValue={rule.field}
                onSelect={(v) => {
                  if (isMatchField(v)) updateRule(index, { field: v });
                }}
              />
              <Dropdown
                className="w-36"
                options={styles}
                selectedValue={rule.style}
                onSelect={(v) => {
                  if (isCleanupStyle(v)) updateRule(index, { style: v });
                }}
              />
              <label className="flex items-center gap-1.5 text-xs cursor-pointer">
                <input
                  type="checkbox"
                  checked={rule.exact ?? false}
                  onChange={(e) =>
                    updateRule(index, { exact: e.target.checked })
                  }
                />
                {t("settings.cleanup.appStyles.exact")}
              </label>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  className={iconButton}
                  aria-label={t("settings.cleanup.appStyles.moveUp")}
                  title={t("settings.cleanup.appStyles.moveUp")}
                  disabled={index === 0}
                  onClick={() => move(index, -1)}
                >
                  <ArrowUp size={14} />
                </button>
                <button
                  type="button"
                  className={iconButton}
                  aria-label={t("settings.cleanup.appStyles.moveDown")}
                  title={t("settings.cleanup.appStyles.moveDown")}
                  disabled={index === rules.length - 1}
                  onClick={() => move(index, 1)}
                >
                  <ArrowDown size={14} />
                </button>
                <button
                  type="button"
                  className={iconButton}
                  aria-label={t("settings.cleanup.appStyles.delete")}
                  title={t("settings.cleanup.appStyles.delete")}
                  onClick={() => setRules(rules.filter((_, i) => i !== index))}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </div>
          ))}
          <div className="flex gap-2">
            <Button
              variant="primary"
              size="md"
              onClick={() =>
                setRules([
                  ...rules,
                  {
                    pattern: "",
                    field: "AppName",
                    style: "Neutral",
                    exact: false,
                  },
                ])
              }
            >
              {t("settings.cleanup.appStyles.add")}
            </Button>
            <Button
              variant="secondary"
              size="md"
              disabled={resetting}
              onClick={() => void reset()}
            >
              {t("settings.cleanup.appStyles.reset")}
            </Button>
          </div>
        </div>
      </SettingContainer>
    </>
  );
};
