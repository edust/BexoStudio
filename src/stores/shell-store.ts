import { create } from "zustand";

export type ThemeMode = "light" | "dark";

const THEME_MODE_STORAGE_KEY = "bexo.theme.mode";

function resolveInitialThemeMode(): ThemeMode {
  if (typeof window === "undefined") {
    return "light";
  }

  const storedMode = window.localStorage.getItem(THEME_MODE_STORAGE_KEY);
  if (storedMode === "light" || storedMode === "dark") {
    return storedMode;
  }

  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function persistThemeMode(mode: ThemeMode) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(THEME_MODE_STORAGE_KEY, mode);
}

type ShellState = {
  sidebarQuery: string;
  selectedHomeWorkspaceId: string | null;
  themeMode: ThemeMode;
  setSidebarQuery: (value: string) => void;
  resetSidebarQuery: () => void;
  setSelectedHomeWorkspaceId: (value: string | null) => void;
  setThemeMode: (value: ThemeMode) => void;
  toggleThemeMode: () => void;
};

export const useShellStore = create<ShellState>((set, get) => ({
  sidebarQuery: "",
  selectedHomeWorkspaceId: null,
  themeMode: resolveInitialThemeMode(),
  setSidebarQuery: (value) => set({ sidebarQuery: value }),
  resetSidebarQuery: () => set({ sidebarQuery: "" }),
  setSelectedHomeWorkspaceId: (value) => set({ selectedHomeWorkspaceId: value }),
  setThemeMode: (value) => {
    const normalizedMode: ThemeMode = value === "dark" ? "dark" : "light";
    persistThemeMode(normalizedMode);
    set({ themeMode: normalizedMode });
  },
  toggleThemeMode: () => {
    const nextMode: ThemeMode = get().themeMode === "dark" ? "light" : "dark";
    persistThemeMode(nextMode);
    set({ themeMode: nextMode });
  },
}));
