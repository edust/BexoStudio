import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";

import { defaultAppPreferences } from "@/lib/app-preferences";
import { appPreferencesQueryKey, getAppPreferences } from "@/queries/preferences";
import type { CodexHistoryViewPreferences } from "@/types/backend";

const MESSAGE_FONT_SIZE_MIN = 10;
const MESSAGE_FONT_SIZE_MAX = 24;

export function useCodexHistoryViewPreferences(enabled: boolean) {
  const preferencesQuery = useQuery({
    queryKey: appPreferencesQueryKey,
    queryFn: getAppPreferences,
    enabled,
    staleTime: 30_000,
  });

  return useMemo(
    () => normalizeCodexHistoryViewPreferences(preferencesQuery.data?.codexHistory),
    [preferencesQuery.data?.codexHistory],
  );
}

export function normalizeCodexHistoryViewPreferences(
  preferences?: Partial<CodexHistoryViewPreferences> | null,
): CodexHistoryViewPreferences {
  const defaults = defaultAppPreferences.codexHistory;
  return {
    messageFontFamily: preferences?.messageFontFamily?.trim() ?? defaults.messageFontFamily,
    messageFontSize: normalizeMessageFontSize(preferences?.messageFontSize),
  };
}

function normalizeMessageFontSize(value?: number | null) {
  if (!Number.isFinite(value)) {
    return defaultAppPreferences.codexHistory.messageFontSize;
  }

  const rounded = Math.round(value as number);
  if (rounded < MESSAGE_FONT_SIZE_MIN || rounded > MESSAGE_FONT_SIZE_MAX) {
    return defaultAppPreferences.codexHistory.messageFontSize;
  }

  return rounded;
}
