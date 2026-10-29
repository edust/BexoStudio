// Browser-only fixture for visual review of every main-window route.
// All IPC stays in memory: no real projects, credentials or system commands are touched.
// Usage: /tests/browser/app-ui.html?route=/prompts&dark
import { createRoot } from "react-dom/client";
import { createMemoryRouter, Navigate, RouterProvider } from "react-router-dom";

import { AppProviders } from "../../src/app/providers";
import { AppShell } from "../../src/layouts/app-shell";
import OssPage from "../../src/features/oss/oss-page";
import CodexAuthPage from "../../src/pages/codex-auth-page";
import HistoryPage from "../../src/pages/history-page";
import HomePage from "../../src/pages/home-page";
import LogsPage from "../../src/pages/logs-page";
import PromptsPage from "../../src/pages/prompts-page";
import SettingsPage from "../../src/pages/settings-page";
import { defaultAppPreferences } from "../../src/lib/app-preferences";
import { useShellStore } from "../../src/stores/shell-store";
import "../../src/styles/globals.css";

const params = new URLSearchParams(location.search);
const route = params.get("route") ?? "/";
const dark = params.has("dark");
localStorage.setItem("bexo.theme.mode", dark ? "dark" : "light");
useShellStore.setState({ themeMode: dark ? "dark" : "light" });

const now = "2026-10-19T09:30:00Z";
const prefs = structuredClone(defaultAppPreferences);
const workspaceId = (index: number) => `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`;
const names = ["StarExpoHub", "go-rfid-gate-checkin", "bexo-studio", "crm-admin-web"];
const notes = ["会展大后台程序", "RFID 门禁签到程序", "", "客户管理后台"];
let workspaces = Array.from({ length: 60 }, (_, index) => ({
  id: workspaceId(index),
  name: names[index] ?? `项目 ${String(index).padStart(4, "0")}`,
  description: notes[index] ?? (index % 3 === 0 ? "" : `工作项目备注 ${index}`),
  icon: null, color: null, sortOrder: index, isDefault: index === 0, isArchived: index >= 56,
  createdAt: now, updatedAt: now,
  projects: [{
    id: `p-${index}`, workspaceId: workspaceId(index), name: "project",
    path: index === 1 ? "D:\\Desktop\\a\\StarExpoHub\\go-star-expo-hub-checkin\\go-rfid-gate-checkin" : `D:\\Desktop\\a\\${names[index] ?? `project-${index}`}`,
    platform: "windows", terminalType: "windows_terminal", ideType: "vscode", codexProfileId: null,
    openTerminal: true, openIde: true, autoResumeCodex: false, sortOrder: 0, createdAt: now, updatedAt: now,
    launchTasks: index === 0 ? [
      ["前端开发服务器", "npm run dev", ""],
      ["API 服务", "go run ./cmd/server", "D:\\Desktop\\a\\StarExpoHub\\api-service"],
      ["管理后台", "pnpm --filter admin dev", ""],
      ["Codex", "codex --profile work", ""],
    ].map(([name, command, workingDir], taskIndex) => ({
      id: `task-${taskIndex}`, projectId: "p-0", name, taskType: "terminal_command", enabled: taskIndex !== 3,
      command, args: [], workingDir, timeoutMs: 30000, continueOnFailure: false, retryPolicy: {}, sortOrder: taskIndex,
    })) : [],
  }],
}));
prefs.workspace.pinnedWorkspaceIds = [workspaceId(0), workspaceId(1)];

const prompts = [
  ["生产级代码 Review", "请按生产标准审查以下改动：正确性、边界条件、错误处理、并发与资源释放……"],
  ["需求澄清", "先复述你对需求的理解，列出不明确的地方，再给出方案。"],
  ["提交说明", "根据 diff 生成简洁的提交说明，首行不超过 50 字。"],
  ["SQL 优化", "分析下面的 SQL 执行计划并给出索引建议。"],
].map(([title, content], index) => ({ id: `prompt-${index}`, title, content, sortOrder: index, createdAt: now, updatedAt: now }));

const quota = (profileId: string, five: number, seven: number) => ({
  profileId, success: true, credentialStatus: "ok", tiers: [
    { name: "five_hour", utilization: five, resetsAt: now }, { name: "seven_day", utilization: seven, resetsAt: now },
  ], queriedAt: now,
});
const authProfiles = [
  { id: "auth-1", name: "工作账号", description: "公司 Team 订阅", codexHome: "C:\\Users\\dev\\.codex", isActive: true, lastQuota: quota("auth-1", 32, 58), createdAt: now, updatedAt: now },
  { id: "auth-2", name: "个人账号", description: "", codexHome: "C:\\Users\\dev\\.codex-personal", isActive: false, lastQuota: quota("auth-2", 4, 12), createdAt: now, updatedAt: now },
  { id: "auth-3", name: "备用账号", description: "额度用完时切换", codexHome: "D:\\codex\\backup", isActive: false, lastQuota: null, createdAt: now, updatedAt: now },
];

const sessions = Array.from({ length: 12 }, (_, index) => ({
  providerId: "codex", sessionId: `019a-${index}`, title: ["重构工作台布局", "修复 OSS 上传失败", "Prompt 导入导出", "截图工具栏定位"][index % 4],
  summary: "讨论界面精简方案并实现", projectDir: `D:\\Desktop\\a\\${names[index % 4]}`, createdAt: now, lastActiveAt: now,
  sourcePath: `C:\\Users\\dev\\.codex\\sessions\\2026\\10\\19\\rollout-${index}.jsonl`, fileSizeBytes: 182_000 + index * 1000,
}));
const messages = [
  { role: "user", content: "程序界面有点臃肿，帮我精简。", timestamp: now, itemType: "message", truncated: false },
  { role: "assistant", content: "已梳理重复信息与装饰元素，方案如下……", timestamp: now, itemType: "message", truncated: false },
];

const ossAccounts = [{ id: "oss-1", displayName: "生产账号", authMode: "access_key", accessKeyIdHint: "LTAI****9xQ", lastProbeStatus: "success", isDisabled: false, createdAt: now, updatedAt: now }];
const ossTargets = [{ id: "target-1", accountId: "oss-1", displayName: "静态资源", bucket: "bexo-assets", region: "oss-cn-hangzhou", endpoint: "oss-cn-hangzhou.aliyuncs.com", prefix: "", isDefault: true, sortOrder: 0, createdAt: now, updatedAt: now }];
const ossObjects = { objects: ["logo.png", "banner-2026.jpg", "manual.pdf"].map((key, index) => ({ key, kind: "object", size: 120_000 * (index + 1), lastModified: now })), commonPrefixes: ["images/", "docs/"], isTruncated: false };

const handlers: Record<string, (args: any) => unknown> = {
  list_workspaces: () => workspaces,
  list_recent_restore_targets: () => [],
  get_app_preferences: () => prefs,
  update_app_preferences: (args) => { for (const key of Object.keys(args.input)) Object.assign((prefs as any)[key], args.input[key]); return prefs; },
  get_restore_capabilities: () => ({ vscode: { available: true }, jetbrains: { available: false } }),
  list_workspace_resource_children: () => ["src", "docs", "scripts", "package.json", "README.md"].map((name) => ({ name, path: `D:/Desktop/a/StarExpoHub/${name}`, kind: name.includes(".") ? "file" : "directory", isHidden: false })),
  allow_workspace_resource_scope: () => "D:/Desktop/a/StarExpoHub",
  get_workspace_resource_git_statuses: () => ({ workspaceRootPath: "D:/Desktop/a/StarExpoHub", gitAvailable: true, statuses: [{ path: "D:/Desktop/a/StarExpoHub/README.md", status: "modified" }] }),
  set_workspaces_archived: (args) => { workspaces = workspaces.map((item) => args.input.workspaceIds.includes(item.id) ? { ...item, isArchived: args.input.archived } : item); return { workspaces, warning: null }; },
  reorder_workspaces: (args) => { workspaces = args.input.workspaceIds.map((id: string) => workspaces.find((item) => item.id === id)); return workspaces; },
  list_prompts: () => prompts,
  list_codex_auth_profiles: () => authProfiles,
  get_codex_auth_profile_detail: (args) => ({ ...authProfiles.find((item) => item.id === args.id), authJson: "{\n  \"OPENAI_API_KEY\": null,\n  \"tokens\": \"<hidden>\"\n}", configToml: "model = \"gpt-5\"\nmodel_reasoning_effort = \"high\"\n" }),
  get_codex_home_directory: () => ({ path: "C:\\Users\\dev\\.codex", source: "default", exists: true }),
  refresh_all_codex_auth_quotas: () => ({ refreshed: 0, failed: 0, total: 0, finishedAt: now }),
  list_all_codex_history_sessions: () => ({ codexRoots: ["C:\\Users\\dev\\.codex"], sessions, nextCursor: null, hasMore: false }),
  get_codex_history_messages: () => ({ messages, olderCursor: null, hasMore: false }),
  list_oss_accounts: () => ossAccounts,
  list_oss_targets: () => ossTargets,
  list_oss_objects: () => ossObjects,
  list_oss_transfer_tasks: () => [],
  get_hotkey_health: () => ({ status: "ready", initialized: true, registeredBindings: [{ action: "screenshot_capture", shortcut: "Ctrl+Shift+A" }], updatedAt: now }),
};

const fixture = window as any;
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
    const handler = handlers[command];
    if (!handler) return { ok: false, error: { code: "FIXTURE_MISSING", message: `Fixture does not implement ${command}` } };
    return { ok: true, data: structuredClone(handler(args)) };
  },
};
if (route === "/") useShellStore.setState({ selectedHomeWorkspaceId: workspaceId(0) });

const router = createMemoryRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "oss", element: <OssPage /> },
      { path: "history", element: <HistoryPage /> },
      { path: "codex-auth", element: <CodexAuthPage /> },
      { path: "prompts", element: <PromptsPage /> },
      { path: "logs", element: <LogsPage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "settings/:section", element: <SettingsPage /> },
      { path: "*", element: <Navigate replace to="/" /> },
    ],
  },
], { initialEntries: [route] });

createRoot(document.getElementById("root")!).render(
  <AppProviders>
    <RouterProvider router={router} />
  </AppProviders>,
);