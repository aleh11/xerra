import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";

export const CleanupApiKey: React.FC = () => {
  const { t } = useTranslation();
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setHasKey(await commands.hasCleanupApiKey());
    } catch (error) {
      console.error("Failed to read cleanup key status:", error);
      setHasKey(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const write = async (key: string) => {
    setBusy(true);
    try {
      const result = await commands.setCleanupApiKey(key);
      if (result.status === "error") {
        toast.error(
          t("settings.cleanup.apiKey.error", { error: result.error }),
        );
      } else {
        setDraft("");
      }
    } catch (error) {
      toast.error(t("settings.cleanup.apiKey.error", { error: String(error) }));
    } finally {
      setBusy(false);
      await refresh();
    }
  };

  return (
    <SettingContainer
      title={t("settings.cleanup.apiKey.title")}
      description={t("settings.cleanup.apiKey.description")}
      descriptionMode="inline"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-2">
        <div className="flex gap-2">
          <Input
            type="password"
            autoComplete="off"
            variant="compact"
            className="flex-1 min-w-0"
            aria-label={t("settings.cleanup.apiKey.title")}
            placeholder={t("settings.cleanup.apiKey.placeholder")}
            value={draft}
            disabled={busy}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && draft.trim()) void write(draft.trim());
            }}
          />
          <Button
            variant="primary"
            size="md"
            disabled={busy || !draft.trim()}
            onClick={() => void write(draft.trim())}
          >
            {t("settings.cleanup.apiKey.save")}
          </Button>
          <Button
            variant="secondary"
            size="md"
            disabled={busy || !hasKey}
            onClick={() => void write("")}
          >
            {t("settings.cleanup.apiKey.clear")}
          </Button>
        </div>
        {hasKey !== null && (
          <p
            className={`text-xs ${hasKey ? "text-green-500" : "text-mid-gray"}`}
          >
            {hasKey
              ? t("settings.cleanup.apiKey.stored")
              : t("settings.cleanup.apiKey.none")}
          </p>
        )}
      </div>
    </SettingContainer>
  );
};
