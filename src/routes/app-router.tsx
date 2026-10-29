import { Suspense, lazy, type ReactNode } from "react";
import { Flex, Spin, Typography } from "antd";
import { createBrowserRouter, Navigate, RouterProvider } from "react-router-dom";

import { AppShell } from "@/layouts/app-shell";

const HomePage = lazy(() => import("@/pages/home-page"));
const HistoryPage = lazy(() => import("@/pages/history-page"));
const CodexAuthPage = lazy(() => import("@/pages/codex-auth-page"));
const PromptsPage = lazy(() => import("@/pages/prompts-page"));
const WorkspacesPage = lazy(() => import("@/pages/workspaces-page"));
const SnapshotsPage = lazy(() => import("@/pages/snapshots-page"));
const ProfilesPage = lazy(() => import("@/pages/profiles-page"));
const LogsPage = lazy(() => import("@/pages/logs-page"));
const SettingsPage = lazy(() => import("@/pages/settings-page"));
const OssPage = lazy(() => import("@/features/oss/oss-page"));

function RouterFallback() {
  return (
    <Flex
      align="center"
      className="min-h-[320px] rounded-[14px] border border-line bg-surface-2"
      justify="center"
      vertical
    >
      <Spin size="large" />
      <Typography.Text className="mt-3 text-[13px] font-medium text-fg">
        正在载入模块
      </Typography.Text>
      <Typography.Paragraph className="!mb-0 !mt-1 !text-[12px] !text-fg-2">
        Bexo Studio 正在准备当前页面壳。
      </Typography.Paragraph>
    </Flex>
  );
}

const withFallback = (element: ReactNode) => <Suspense fallback={<RouterFallback />}>{element}</Suspense>;

const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: withFallback(<HomePage />) },
      { path: "oss", element: withFallback(<OssPage />) },
      { path: "history", element: withFallback(<HistoryPage />) },
      { path: "codex-auth", element: withFallback(<CodexAuthPage />) },
      { path: "prompts", element: withFallback(<PromptsPage />) },
      { path: "workspaces", element: withFallback(<WorkspacesPage />) },
      { path: "snapshots", element: withFallback(<SnapshotsPage />) },
      { path: "profiles", element: withFallback(<ProfilesPage />) },
      { path: "logs", element: withFallback(<LogsPage />) },
      { path: "settings", element: withFallback(<SettingsPage />) },
      { path: "settings/:section", element: withFallback(<SettingsPage />) },
      { path: "*", element: <Navigate replace to="/" /> },
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
