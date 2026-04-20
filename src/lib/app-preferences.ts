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
    pinnedWorkspaceIds: [],
  },
  startup: {
    launchAtLogin: false,
    startSilently: false,
  },
  hotkey: {
    screenshotCapture: "Ctrl+Shift+X",
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
