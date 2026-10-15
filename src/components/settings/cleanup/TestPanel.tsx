import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Check, Copy } from "lucide-react";
import {
  commands,
  type CleanupSource,
  type CleanupStyle,
  type CleanupTestResult,
} from "@/bindings";
import { Dropdown } from "../../ui/Dropdown";
import { Button } from "../../ui/Button";
import { Textarea } from "../../ui/Textarea";
import Badge from "../../ui/Badge";
import { SettingContainer } from "../../ui/SettingContainer";
import { copyToClipboard } from "../history/clipboard";
import { DebouncedInput } from "./DebouncedInput";
import { isCleanupStyle, styleOptions } from "./options";

const AUTO = "auto";

const sourceVariant: Record<
  CleanupSource,
  "primary" | "success" | "secondary"
> = {
  Llm: "success",
  Regex: "primary",
  Unchanged: "secondary",
  Snippet: "primary",
};

export const CleanupTestPanel: React.FC = () => {
  const { t } = useTranslation();
  const [raw, setRaw] = useState("");
  const [style, setStyle] = useState<string>(AUTO);
  const [appName, setAppName] = useState("");
  const [windowTitle, setWindowTitle] = useState("");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CleanupTestResult | null>(null);
  const [copied, setCopied] = useState(false);

  const options = [
    { value: AUTO, label: t("settings.cleanup.test.styleAuto") },
    ...styleOptions(t),
  ];

  const run = async () => {
    setRunning(true);
    setCopied(false);
    try {
      const chosen: CleanupStyle | null = isCleanupStyle(style) ? style : null;
      setResult(
        await commands.testCleanup(
          raw,
          chosen,
          appName.trim() || null,
          windowTitle.trim() || null,
        ),
      );
    } catch (error) {
      toast.error(t("settings.cleanup.test.failed", { error: String(error) }));
    } finally {
      setRunning(false);
    }
  };

  const copy = async () => {
    if (!result) return;
    if (await copyToClipboard(result.output)) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } else {
      toast.error(t("settings.cleanup.test.copyError"));
    }
  };

  return (
    <SettingContainer
      title={t("settings.cleanup.test.title")}
      description={t("settings.cleanup.test.description")}
      descriptionMode="inline"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-3">
        <Textarea
          variant="compact"
          className="w-full"
          aria-label={t("settings.cleanup.test.rawLabel")}
          placeholder={t("settings.cleanup.test.rawPlaceholder")}
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
        />
        <div className="flex flex-wrap gap-2 items-center">
          <Dropdown
            className="w-44"
            options={options}
            selectedValue={style}
            onSelect={setStyle}
          />
          <DebouncedInput
            type="text"
            variant="compact"
            className="flex-1 min-w-[140px]"
            aria-label={t("settings.cleanup.test.appName")}
            placeholder={t("settings.cleanup.test.appName")}
            value={appName}
            onCommit={setAppName}
          />
          <DebouncedInput
            type="text"
            variant="compact"
            className="flex-1 min-w-[140px]"
            aria-label={t("settings.cleanup.test.windowTitle")}
            placeholder={t("settings.cleanup.test.windowTitle")}
            value={windowTitle}
            onCommit={setWindowTitle}
          />
        </div>
        <Button
          variant="primary"
          size="md"
          disabled={running || !raw.trim()}
          onClick={() => void run()}
        >
          {running
            ? t("settings.cleanup.test.running")
            : t("settings.cleanup.test.run")}
        </Button>

        {result && (
          <div className="space-y-2 pt-2 border-t border-mid-gray/20">
            <div className="flex flex-wrap items-center gap-2 text-xs">
              <Badge variant={sourceVariant[result.source] ?? "secondary"}>
                {t(`settings.cleanup.test.sources.${result.source}`)}
              </Badge>
              <span className="text-mid-gray">
                {t("settings.cleanup.test.style", {
                  style: t(`settings.cleanup.styles.${result.style}`),
                })}
              </span>
              <span className="text-mid-gray">
                {t("settings.cleanup.test.latency", {
                  ms: result.latency_ms,
                })}
              </span>
            </div>
            {result.error && (
              <p className="text-sm text-red-500 break-words">
                {t("settings.cleanup.test.error", { error: result.error })}
              </p>
            )}
            <div className="relative">
              <pre className="whitespace-pre-wrap break-words text-sm font-sans p-3 pe-10 rounded-md bg-mid-gray/10 border border-mid-gray/20 select-text">
                {result.output}
              </pre>
              <button
                type="button"
                className="absolute top-2 end-2 p-1 rounded-md hover:bg-logo-primary/20 cursor-pointer"
                aria-label={t("settings.cleanup.test.copy")}
                title={t("settings.cleanup.test.copy")}
                onClick={() => void copy()}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
            </div>
          </div>
        )}
      </div>
    </SettingContainer>
  );
};
