import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight } from "lucide-react";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { CleanupApiKey } from "./ApiKeyField";
import { AppStyleRules } from "./AppStyleRules";
import { CleanupTestPanel } from "./TestPanel";
import { DictionaryAndSnippets } from "./DictionaryAndSnippets";
import { DebouncedInput, NumberField } from "./DebouncedInput";
import {
  CLEANUP_DEFAULT_MODEL,
  THRESHOLD_RANGE,
  TIMEOUT_RANGE,
  useCleanupSettings,
} from "./useCleanupSettings";
import { REASONING_EFFORTS } from "./options";

export const CleanupSettings: React.FC = () => {
  const { t } = useTranslation();
  const { cleanup, save } = useCleanupSettings();
  const [advancedOpen, setAdvancedOpen] = useState(false);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.cleanup.general.title")}>
        <ToggleSwitch
          checked={cleanup.enabled ?? false}
          onChange={(enabled) => void save({ enabled })}
          label={t("settings.cleanup.enable.label")}
          description={t("settings.cleanup.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        />
        <CleanupApiKey />
        <SettingContainer
          title={t("settings.cleanup.model.title")}
          description={t("settings.cleanup.model.description")}
          descriptionMode="inline"
          layout="stacked"
          grouped={true}
        >
          <DebouncedInput
            type="text"
            variant="compact"
            className="w-full"
            aria-label={t("settings.cleanup.model.title")}
            placeholder={CLEANUP_DEFAULT_MODEL}
            value={cleanup.model ?? ""}
            onCommit={(model) => void save({ model: model.trim() })}
          />
        </SettingContainer>
        <SettingContainer
          title={t("settings.cleanup.reasoningEffort.title")}
          description={t("settings.cleanup.reasoningEffort.description")}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <Dropdown
            options={REASONING_EFFORTS.map((value) => ({
              value,
              label: t(`settings.cleanup.reasoningEffort.options.${value}`),
            }))}
            selectedValue={cleanup.reasoning_effort ?? "low"}
            onSelect={(reasoning_effort) => void save({ reasoning_effort })}
          />
        </SettingContainer>
        <SettingContainer
          title={t("settings.cleanup.timeout.title")}
          description={t("settings.cleanup.timeout.description")}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <NumberField
            value={cleanup.timeout_ms ?? 2000}
            min={TIMEOUT_RANGE.min}
            max={TIMEOUT_RANGE.max}
            ariaLabel={t("settings.cleanup.timeout.title")}
            errorMessage={t("settings.cleanup.timeout.error", TIMEOUT_RANGE)}
            onCommit={(timeout_ms) => void save({ timeout_ms })}
          />
        </SettingContainer>
        <SettingContainer
          title={t("settings.cleanup.shortUtterance.title")}
          description={t("settings.cleanup.shortUtterance.description")}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <NumberField
            value={cleanup.short_utterance_words ?? 8}
            min={THRESHOLD_RANGE.min}
            max={THRESHOLD_RANGE.max}
            ariaLabel={t("settings.cleanup.shortUtterance.title")}
            errorMessage={t(
              "settings.cleanup.shortUtterance.error",
              THRESHOLD_RANGE,
            )}
            onCommit={(short_utterance_words) =>
              void save({ short_utterance_words })
            }
          />
        </SettingContainer>
        <div className="px-4 p-2">
          <button
            type="button"
            className="flex items-center gap-1 text-sm font-medium cursor-pointer"
            aria-expanded={advancedOpen}
            onClick={() => setAdvancedOpen(!advancedOpen)}
          >
            <ChevronRight
              size={14}
              className={`transition-transform rtl:rotate-180 ${advancedOpen ? "rotate-90 rtl:rotate-90" : ""}`}
            />
            {t("settings.cleanup.advanced.title")}
          </button>
          {advancedOpen && (
            <div className="pt-2 space-y-1">
              <h3 className="text-sm font-medium">
                {t("settings.cleanup.baseUrl.title")}
              </h3>
              <p className="text-sm">
                {t("settings.cleanup.baseUrl.description")}
              </p>
              <DebouncedInput
                type="text"
                variant="compact"
                className="w-full"
                aria-label={t("settings.cleanup.baseUrl.title")}
                placeholder="https://openrouter.ai/api/v1"
                value={cleanup.base_url ?? ""}
                onCommit={(base_url) =>
                  void save({ base_url: base_url.trim() })
                }
              />
            </div>
          )}
        </div>
      </SettingsGroup>

      <SettingsGroup title={t("settings.cleanup.styles.title")}>
        <AppStyleRules />
      </SettingsGroup>

      <SettingsGroup title={t("settings.cleanup.personal.title")}>
        <DictionaryAndSnippets />
      </SettingsGroup>

      <SettingsGroup title={t("settings.cleanup.test.groupTitle")}>
        <CleanupTestPanel />
      </SettingsGroup>
    </div>
  );
};
