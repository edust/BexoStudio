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
  hotkey: {
    screenshotCapture: "Ctrl+Alt+A",
    voiceInputToggle: null,
    voiceInputHold: null,
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
