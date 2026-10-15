import { useCallback } from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { commands, type CleanupSettings } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";

export const CLEANUP_DEFAULT_MODEL = "anthropic/claude-haiku-5.5";
export const TIMEOUT_RANGE = { min: 200, max: 30000 } as const;
export const THRESHOLD_RANGE = { min: 0, max: 100 } as const;

/**
 * Reads the cleanup settings from the shared settings store and exposes a
 * `save` that merges a patch into the latest value, updates the store
 * optimistically, persists the full object through `setCleanupSettings` and
 * re-syncs from the backend (which also reverts the UI if the save failed).
 */
export const useCleanupSettings = () => {
  const { t } = useTranslation();
  const cleanup = useSettingsStore((s) => s.settings?.cleanup);

  const save = useCallback(
    async (
      patch:
        | Partial<CleanupSettings>
        | ((current: CleanupSettings) => Partial<CleanupSettings>),
    ) => {
      const state = useSettingsStore.getState();
      if (!state.settings) return;
      const current = state.settings.cleanup ?? {};
      const next: CleanupSettings = {
        ...current,
        ...(typeof patch === "function" ? patch(current) : patch),
      };
      state.setSettings({ ...state.settings, cleanup: next });
      try {
        const result = await commands.setCleanupSettings(next);
        if (result.status === "error") {
          toast.error(t("settings.cleanup.saveError", { error: result.error }));
        }
      } catch (error) {
        toast.error(t("settings.cleanup.saveError", { error: String(error) }));
      }
      await useSettingsStore.getState().refreshSettings();
    },
    [t],
  );

  return { cleanup: cleanup ?? ({} as CleanupSettings), save };
};
