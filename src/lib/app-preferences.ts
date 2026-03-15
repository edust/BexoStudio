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
    screenshotCapture: "Ctrl+Shift+X",
    screenshotTools: {
      select: "1",
      line: "2",
      rect: "3",
      ellipse: "4",
      arrow: "5",
    },
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
