import {
  getAppPreferences,
  getCodexHomeDirectory,
  updateAppPreferences,
} from "@/lib/command-client";

export const appPreferencesQueryKey = ["appPreferences"] as const;
export const codexHomeDirectoryQueryKey = ["codexHomeDirectory"] as const;

export { getAppPreferences, getCodexHomeDirectory, updateAppPreferences };
