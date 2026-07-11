import {
  getAppPreferences,
  getCodexHomeDirectory,
  getHotkeyHealth,
  retryHotkeyRegistration,
  updateAppPreferences,
} from "@/lib/command-client";

export const appPreferencesQueryKey = ["appPreferences"] as const;
export const codexHomeDirectoryQueryKey = ["codexHomeDirectory"] as const;
export const hotkeyHealthQueryKey = ["hotkeyHealth"] as const;

export {
  getAppPreferences,
  getCodexHomeDirectory,
  getHotkeyHealth,
  retryHotkeyRegistration,
  updateAppPreferences,
};
