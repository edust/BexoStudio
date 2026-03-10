import { create } from "zustand";

type ShellState = {
  sidebarQuery: string;
  selectedHomeWorkspaceId: string | null;
  setSidebarQuery: (value: string) => void;
  resetSidebarQuery: () => void;
  setSelectedHomeWorkspaceId: (value: string | null) => void;
};

export const useShellStore = create<ShellState>((set) => ({
  sidebarQuery: "",
  selectedHomeWorkspaceId: null,
  setSidebarQuery: (value) => set({ sidebarQuery: value }),
  resetSidebarQuery: () => set({ sidebarQuery: "" }),
  setSelectedHomeWorkspaceId: (value) => set({ selectedHomeWorkspaceId: value }),
}));
