import type { AppPreferences } from "@/types/backend";

export const defaultAppPreferences: AppPreferences = {
  terminal: {
    windowsTerminalPath: null,
    codexCliPath: null,
    commandShell: "powershell7",
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
    promptQuickPasteSlots: [1, 2, 3, 4, 5].map((slot) => ({
      slot,
      enabled: false,
      promptId: null,
      shortcut: `Ctrl+Alt+Shift+${slot}`,
    })),
  },
  tray: {
    closeToTray: true,
    showRecentWorkspaces: true,
  },
  diagnostics: {
    showAdapterSources: true,
    showExecutablePaths: true,
  },
  codexHistory: {
    messageFontFamily: "",
    messageFontSize: 12,
  },
  codexAuth: {
    quotaRefreshIntervalSeconds: 60,
    proxy: {
      mode: "system",
      manualProxyUrl: "",
    },
  },
};
