import type { AppPreferences } from "@/types/backend";

export const defaultAppPreferences: AppPreferences = {
  terminal: {
    windowsTerminalPath: null,
    codexCliPath: null,
    commandTemplates: [],
  },
  ide: {
    vscodePath: null,
    jetbrainsPath: null,
    customEditors: [],
  },
  workspace: {
    selectedWorkspaceIds: [],
  },
  startup: {
    launchAtLogin: false,
    startSilently: false,
  },
  tray: {
    closeToTray: true,
    showRecentWorkspaces: true,
  },
  diagnostics: {
    showAdapterSources: true,
    showExecutablePaths: true,
  },
};
