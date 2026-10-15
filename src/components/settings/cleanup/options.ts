import type { TFunction } from "i18next";
import type { CleanupStyle, MatchField } from "@/bindings";

export const CLEANUP_STYLES: CleanupStyle[] = [
  "Neutral",
  "Casual",
  "Formal",
  "Code",
];
export const MATCH_FIELDS: MatchField[] = [
  "AppName",
  "BundleId",
  "WindowTitle",
];
export const REASONING_EFFORTS = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
] as const;

export const styleOptions = (t: TFunction) =>
  CLEANUP_STYLES.map((s) => ({
    value: s,
    label: t(`settings.cleanup.styles.${s}`),
  }));

export const fieldOptions = (t: TFunction) =>
  MATCH_FIELDS.map((f) => ({
    value: f,
    label: t(`settings.cleanup.appStyles.fields.${f}`),
  }));

export const isCleanupStyle = (v: string): v is CleanupStyle =>
  (CLEANUP_STYLES as string[]).includes(v);
export const isMatchField = (v: string): v is MatchField =>
  (MATCH_FIELDS as string[]).includes(v);
