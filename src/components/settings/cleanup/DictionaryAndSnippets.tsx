import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Trash2, X } from "lucide-react";
import type { Replacement, Snippet } from "@/bindings";
import { Button } from "../../ui/Button";
import { Input } from "../../ui/Input";
import { SettingContainer } from "../../ui/SettingContainer";
import { DebouncedInput, DebouncedTextarea } from "./DebouncedInput";
import { useCleanupSettings } from "./useCleanupSettings";

const deleteButton =
  "p-1.5 rounded-md border border-mid-gray/30 hover:bg-logo-primary/20 hover:border-logo-primary cursor-pointer transition-colors shrink-0";

const DictionarySection: React.FC = () => {
  const { t } = useTranslation();
  const { cleanup, save } = useCleanupSettings();
  const [term, setTerm] = useState("");
  const dictionary = cleanup.dictionary ?? [];

  const add = () => {
    const value = term.trim();
    if (!value) return;
    if (dictionary.some((d) => d.toLowerCase() === value.toLowerCase())) {
      toast.error(t("settings.cleanup.dictionary.duplicate", { term: value }));
      return;
    }
    void save({ dictionary: [...dictionary, value] });
    setTerm("");
  };

  return (
    <SettingContainer
      title={t("settings.cleanup.dictionary.title")}
      description={t("settings.cleanup.dictionary.description")}
      descriptionMode="inline"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-3">
        <div className="flex gap-2">
          <Input
            type="text"
            variant="compact"
            className="flex-1 min-w-0"
            aria-label={t("settings.cleanup.dictionary.title")}
            placeholder={t("settings.cleanup.dictionary.placeholder")}
            value={term}
            onChange={(e) => setTerm(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") add();
            }}
          />
          <Button
            variant="primary"
            size="md"
            disabled={!term.trim()}
            onClick={add}
          >
            {t("settings.cleanup.dictionary.add")}
          </Button>
        </div>
        {dictionary.length === 0 ? (
          <p className="text-sm text-mid-gray">
            {t("settings.cleanup.dictionary.empty")}
          </p>
        ) : (
          <div className="flex flex-wrap gap-2">
            {dictionary.map((entry, index) => (
              <span
                key={`${entry}-${index}`}
                className="inline-flex items-center gap-1 ps-3 pe-1.5 py-1 rounded-full text-xs font-medium bg-mid-gray/20"
              >
                {entry}
                <button
                  type="button"
                  className="p-0.5 rounded-full hover:bg-logo-primary/30 cursor-pointer"
                  aria-label={t("settings.cleanup.dictionary.remove", {
                    term: entry,
                  })}
                  onClick={() =>
                    void save((current) => ({
                      dictionary: (current.dictionary ?? []).filter(
                        (_, i) => i !== index,
                      ),
                    }))
                  }
                >
                  <X size={12} />
                </button>
              </span>
            ))}
          </div>
        )}
      </div>
    </SettingContainer>
  );
};

const ReplacementsSection: React.FC = () => {
  const { t } = useTranslation();
  const { cleanup, save } = useCleanupSettings();
  const replacements = cleanup.replacements ?? [];

  const update = (index: number, patch: Partial<Replacement>) =>
    void save((current) => ({
      replacements: (current.replacements ?? []).map((r, i) =>
        i === index ? { ...r, ...patch } : r,
      ),
    }));

  return (
    <SettingContainer
      title={t("settings.cleanup.replacements.title")}
      description={t("settings.cleanup.replacements.description")}
      descriptionMode="inline"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-3">
        {replacements.length === 0 && (
          <p className="text-sm text-mid-gray">
            {t("settings.cleanup.replacements.empty")}
          </p>
        )}
        {replacements.map((rule, index) => (
          <div key={index} className="flex items-center gap-2">
            <DebouncedInput
              type="text"
              variant="compact"
              className="flex-1 min-w-0"
              aria-label={t("settings.cleanup.replacements.heard")}
              placeholder={t("settings.cleanup.replacements.heard")}
              value={rule.heard}
              onCommit={(heard) => update(index, { heard })}
            />
            <span aria-hidden="true" className="text-mid-gray">
              →
            </span>
            <DebouncedInput
              type="text"
              variant="compact"
              className="flex-1 min-w-0"
              aria-label={t("settings.cleanup.replacements.write")}
              placeholder={t("settings.cleanup.replacements.write")}
              value={rule.write}
              onCommit={(write) => update(index, { write })}
            />
            <button
              type="button"
              className={deleteButton}
              aria-label={t("settings.cleanup.replacements.delete")}
              title={t("settings.cleanup.replacements.delete")}
              onClick={() =>
                void save((current) => ({
                  replacements: (current.replacements ?? []).filter(
                    (_, i) => i !== index,
                  ),
                }))
              }
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}
        <Button
          variant="primary"
          size="md"
          onClick={() =>
            void save((current) => ({
              replacements: [
                ...(current.replacements ?? []),
                { heard: "", write: "" },
              ],
            }))
          }
        >
          {t("settings.cleanup.replacements.add")}
        </Button>
      </div>
    </SettingContainer>
  );
};

const SnippetsSection: React.FC = () => {
  const { t } = useTranslation();
  const { cleanup, save } = useCleanupSettings();
  const snippets = cleanup.snippets ?? [];

  const update = (index: number, patch: Partial<Snippet>) =>
    void save((current) => ({
      snippets: (current.snippets ?? []).map((s, i) =>
        i === index ? { ...s, ...patch } : s,
      ),
    }));

  return (
    <SettingContainer
      title={t("settings.cleanup.snippets.title")}
      description={t("settings.cleanup.snippets.description")}
      descriptionMode="inline"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-3">
        {snippets.length === 0 && (
          <p className="text-sm text-mid-gray">
            {t("settings.cleanup.snippets.empty")}
          </p>
        )}
        {snippets.map((snippet, index) => (
          <div
            key={index}
            className="flex items-start gap-2 p-2 rounded-md border border-mid-gray/20"
          >
            <div className="flex-1 min-w-0 space-y-2">
              <DebouncedInput
                type="text"
                variant="compact"
                className="w-full"
                aria-label={t("settings.cleanup.snippets.trigger")}
                placeholder={t("settings.cleanup.snippets.trigger")}
                value={snippet.trigger}
                onCommit={(trigger) => update(index, { trigger })}
              />
              <DebouncedTextarea
                variant="compact"
                className="w-full"
                aria-label={t("settings.cleanup.snippets.expansion")}
                placeholder={t("settings.cleanup.snippets.expansion")}
                value={snippet.expansion}
                onCommit={(expansion) => update(index, { expansion })}
              />
            </div>
            <button
              type="button"
              className={deleteButton}
              aria-label={t("settings.cleanup.snippets.delete")}
              title={t("settings.cleanup.snippets.delete")}
              onClick={() =>
                void save((current) => ({
                  snippets: (current.snippets ?? []).filter(
                    (_, i) => i !== index,
                  ),
                }))
              }
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}
        <Button
          variant="primary"
          size="md"
          onClick={() =>
            void save((current) => ({
              snippets: [
                ...(current.snippets ?? []),
                { trigger: "", expansion: "" },
              ],
            }))
          }
        >
          {t("settings.cleanup.snippets.add")}
        </Button>
      </div>
    </SettingContainer>
  );
};

export const DictionaryAndSnippets: React.FC = () => (
  <>
    <DictionarySection />
    <ReplacementsSection />
    <SnippetsSection />
  </>
);
