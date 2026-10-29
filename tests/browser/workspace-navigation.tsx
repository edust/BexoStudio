// Browser-only fixture. All IPC stays in memory; no real projects or system commands.
import React from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { ConfigProvider, theme } from "antd";
import { Toaster } from "sonner";
import { AppShell } from "../../src/layouts/app-shell";
import HomePage from "../../src/pages/home-page";
import { SectionSidebar } from "../../src/components/shell/section-sidebar";
import { defaultAppPreferences } from "../../src/lib/app-preferences";
import { sidebarContentByRoute } from "../../src/lib/navigation";
import { useShellStore } from "../../src/stores/shell-store";
import "../../src/styles/globals.css";

const fixture = window as any;
const full = new URLSearchParams(location.search).has("full");
if (full && !localStorage.getItem("bexo.resourceBrowser.width")) localStorage.setItem("bexo.resourceBrowser.width", "520");
const dark = new URLSearchParams(location.search).has("dark");
document.documentElement.dataset.theme = dark ? "dark" : "light";
useShellStore.setState({ themeMode: dark ? "dark" : "light" });
const prefs = structuredClone(defaultAppPreferences);
let workspaces = Array.from({ length: 1000 }, (_, index) => ({
  id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
  name: index === 0 ? "StarExpoHub" : index === 1 ? "go-rfid-gate-checkin" : `项目 ${String(index).padStart(4, "0")}`,
  description: index === 0 ? "会展大后台程序" : index === 1 ? "RFID门签到程序" : `工作项目备注 ${index}`,
  icon: null, color: null, sortOrder: index, isDefault: index === 0, isArchived: index >= 980,
  createdAt: "2026-10-19", updatedAt: "2026-10-19",
  projects: [{ id: `p-${index}`, workspaceId: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
    name: "project", path: `D:\\Desktop\\a\\StarExpoHub\\长路径\\项目-${index}`, platform: "windows", terminalType: "windows_terminal",
    ideType: "vscode", codexProfileId: null, openTerminal: true, openIde: true, autoResumeCodex: false,
    sortOrder: 0, createdAt: "2026-10-19", updatedAt: "2026-10-19", launchTasks: full ? Array.from({length:4}, (_, taskIndex) => ({ id: `task-${index}-${taskIndex}`, projectId: `p-${index}`, name: ["StarExpoHub", "api-service", "api-service-web", "CodexNew"][taskIndex], taskType: "terminal_command", enabled: true, command: "go run ./cmd/server", args: [], workingDir: "D:/Desktop/a/StarExpoHub/go-star-expo-hub-api-service", timeoutMs: 30000, continueOnFailure: false, retryPolicy: {}, sortOrder: taskIndex })) : [] }],
}));
if (full) {
  workspaces[1].projects[0].path = "D:/Desktop/a/StarExpoHub/go-star-expo-hub-checkin/go-rfid-gate-checkin";
  workspaces[2].description = "";
  workspaces[3].description = "这是一段较长的项目备注，用于确认窄窗口省略时不会覆盖操作按钮";
}
prefs.workspace.pinnedWorkspaceIds = workspaces.slice(0, 2).map((item) => item.id);
fixture.commands = [];
fixture.isTauri = true;
fixture.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
fixture.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  transformCallback: () => 1,
  unregisterCallback: () => {},
  invoke: async (command: string, args: any = {}) => {
    fixture.commands.push({ command, args });
    if (command.startsWith("plugin:")) return 1;
    let data: any;
    switch (command) {
      case "list_workspace_resource_children": data = ["src", "docs", "scripts"].map(name => ({name, path: `D:/Desktop/a/StarExpoHub/${name}`, kind: "directory", isHidden: false})); break;
      case "allow_workspace_resource_scope": data = "D:/Desktop/a/StarExpoHub"; break;
      case "get_workspace_resource_git_statuses": data = {workspaceRootPath: "D:/Desktop/a/StarExpoHub", gitAvailable: false, statuses: []}; break;
      case "list_workspaces": data = workspaces; break;
      case "list_recent_restore_targets": data = []; break;
      case "get_app_preferences": data = prefs; break;
      case "update_app_preferences":
        for (const key of Object.keys(args.input)) Object.assign((prefs as any)[key], args.input[key]);
        data = prefs; break;
      case "get_restore_capabilities": data = { vscode: { available: true }, jetbrains: { available: false } }; break;
      case "set_workspaces_archived":
        if (fixture.failArchive) return { ok: false, error: { code: "DB_WRITE_FAILED", message: "测试：归档保存失败" } };
        workspaces = workspaces.map((item) => args.input.workspaceIds.includes(item.id) ? { ...item, isArchived: args.input.archived } : item);
        data = { workspaces, warning: null }; break;
      case "update_workspace_description":
        workspaces = workspaces.map((item) => item.id === args.input.workspaceId ? { ...item, description: args.input.description } : item);
        data = workspaces.find((item) => item.id === args.input.workspaceId); break;
      case "reorder_workspaces":
        workspaces = args.input.workspaceIds.map((id: string) => workspaces.find((item) => item.id === id));
        data = workspaces; break;
      default: throw new Error(`Fixture does not implement ${command}`);
    }
    return { ok: true, data: structuredClone(data) };
  },
};

function App() {
  const selected = useShellStore((state) => state.selectedHomeWorkspaceId);
  if (full) return <><Routes><Route element={<AppShell />}><Route path="/" element={<HomePage />} /></Route></Routes><output id="selected" hidden>{selected}</output><Toaster /></>;
  return <div style={{ height: "100vh", display: "flex", padding: 16, gap: 24 }}>
    <div style={{ width: 340, height: "100%" }}><SectionSidebar content={sidebarContentByRoute.home!} /></div>
    <main style={{ padding: 24 }}><h1>工作区导航交互验收</h1><p>1000 个模拟项目，20 个已归档。</p><output id="selected">{selected}</output></main>
    <Toaster />
  </div>;
}
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
    <ConfigProvider theme={{ algorithm: dark ? theme.darkAlgorithm : theme.defaultAlgorithm }}>
      <MemoryRouter><App /></MemoryRouter>
    </ConfigProvider>
  </QueryClientProvider>,
);
