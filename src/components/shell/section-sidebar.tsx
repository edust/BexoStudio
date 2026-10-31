import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CaretRightOutlined,
  CheckSquareOutlined,
  CloseOutlined,
  CodeOutlined,
  CopyOutlined,
  DownloadOutlined,
  EditOutlined,
  FolderOpenOutlined,
  HistoryOutlined,
  StarOutlined,
  StarFilled,
  MoreOutlined,
  PlusOutlined,
  SearchOutlined,
  UploadOutlined,
} from "@ant-design/icons";
import { getCurrentWindow, type DragDropEvent } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Button, Checkbox, Dropdown, Empty, Input, Modal, Tooltip, Typography, type MenuProps } from "antd";
import { motion } from "motion/react";
import { startTransition, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { toast } from "sonner";

import { copyTextToClipboard, getClipboardErrorMessage } from "@/lib/clipboard";
import { defaultAppPreferences } from "@/lib/app-preferences";
import { cn } from "@/lib/cn";
import { PaneHeader } from "@/components/shell/pane-header";
import {
  exportWorkspaceList,
  getErrorSummary,
  getRestoreCapabilities,
  hasDesktopRuntime,
  listRecentRestoreTargets,
  listWorkspaces,
  openCodexHistoryWindow,
  openWorkspaceInEditor,
  openWorkspaceTerminal,
  registerWorkspaceFolder,
  reorderWorkspaces,
  setWorkspacesArchived,
  removeWorkspaceRegistration,
  runWorkspaceTerminalCommands,
  updateWorkspaceDescription,
  upsertProject,
} from "@/lib/command-client";
import { sendDesktopNotification } from "@/lib/desktop-notification";
import { splitWorkspacePath } from "@/components/shell/workspace-sidebar-width";
import { WorkspaceNavigationList } from "@/features/workspaces/workspace-navigation-list";
import { moveWorkspaceWithinGroup, workspaceScope } from "@/features/workspaces/workspace-navigation-model";
import {
  countUnicodeCharacters,
  filterWorkspaceItems,
  MAX_WORKSPACE_DESCRIPTION_CHARS,
  validateWorkspaceDescription,
} from "@/features/workspaces/workspace-model";
import {
  appPreferencesQueryKey,
  getAppPreferences,
  updateAppPreferences,
} from "@/queries/preferences";
import { restoreCapabilitiesQueryKey } from "@/queries/restore-runs";
import { sidebarWorkspacesQueryKey, workspacesQueryKey } from "@/queries/workspaces";
import { recentRestoreTargetsQueryKey } from "@/queries/restore-runs";
import { useShellStore } from "@/stores/shell-store";
import { WorkspaceTransferDialog } from "@/features/workspaces/workspace-transfer-dialog";
import type { SectionSidebarContent, SidebarItem } from "@/types/navigation";
import type {
  AppPreferences,
  CustomEditorRecord,
  ProjectRecord,
  RecentRestoreTarget,
  RestoreCapabilities,
  WorkspaceEditorKey,
  WorkspaceRecord,
} from "@/types/backend";

type SectionSidebarProps = {
  content: SectionSidebarContent;
};

export function SectionSidebar({ content }: SectionSidebarProps) {
  const [query, setQuery] = useState("");
  const [selectedWorkspaceIds, setSelectedWorkspaceIds] = useState<string[]>([]);
  const [orderedWorkspaceItems, setOrderedWorkspaceItems] = useState<WorkspaceSidebarItem[]>([]);
  const [archivedView, setArchivedView] = useState(false);
  const [keyboardId, setKeyboardId] = useState<string | null>(null);
  const [noteEditor, setNoteEditor] = useState<{ workspace: WorkspaceRecord; draft: string; error: string | null } | null>(null);
  const searchRef = useRef<import("antd").InputRef>(null);
  const [workspaceDropPending, setWorkspaceDropPending] = useState(false);
  const [workspaceCreationPath, setWorkspaceCreationPath] = useState<string | null>(null);
  const [workspaceCreationDescription, setWorkspaceCreationDescription] = useState("");
  const [workspaceImportSourcePath, setWorkspaceImportSourcePath] = useState<string | null>(null);
  const location = useLocation();
  const selectedHomeWorkspaceId = useShellStore((state) => state.selectedHomeWorkspaceId);
  const setSelectedHomeWorkspaceId = useShellStore((state) => state.setSelectedHomeWorkspaceId);
  const queryClient = useQueryClient();
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const workspaceDropContainerRef = useRef<HTMLDivElement | null>(null);
  const persistSelectedWorkspaceIdsQueueRef = useRef<Promise<void>>(Promise.resolve());
  const persistPinnedWorkspaceIdsQueueRef = useRef<Promise<void>>(Promise.resolve());
  const workspaceDropPendingRef = useRef(false);
  const workspaceDropActiveRef = useRef(false);
  const workspaceDropLastInsideAtRef = useRef(0);
  const workspaceDropDeactivateTimeoutRef = useRef<number | null>(null);
  const recentWorkspaceDropRef = useRef<{
    signature: string;
    timestamp: number;
  } | null>(null);
  const selectedWorkspaceIdsRef = useRef<string[]>([]);
  const pinnedWorkspaceIdsRef = useRef<string[]>([]);
  const workspaceItemsRef = useRef<WorkspaceSidebarItem[]>([]);
  const latestOrderedWorkspaceItemsRef = useRef<WorkspaceSidebarItem[]>([]);

  const workspaceQuery = useQuery({
    queryKey: sidebarWorkspacesQueryKey,
    queryFn: listWorkspaces,
    enabled: content.dataSource === "workspaces" && desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const recentRestoreTargetsQuery = useQuery({
    queryKey: recentRestoreTargetsQueryKey,
    queryFn: listRecentRestoreTargets,
    enabled: content.dataSource === "workspaces" && desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const preferencesQuery = useQuery({
    queryKey: appPreferencesQueryKey,
    queryFn: getAppPreferences,
    enabled: content.dataSource === "workspaces" && desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const restoreCapabilitiesQuery = useQuery({
    queryKey: restoreCapabilitiesQueryKey,
    queryFn: getRestoreCapabilities,
    enabled: content.dataSource === "workspaces" && desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const updatePreferencesMutation = useMutation({
    mutationFn: updateAppPreferences,
    onSuccess: (preferences) => {
      queryClient.setQueryData(appPreferencesQueryKey, preferences);
    },
  });

  const registerWorkspaceMutation = useMutation({
    mutationFn: ({ path, description }: { path: string; description?: string | null }) =>
      registerWorkspaceFolder(path, description),
    onSuccess: async (workspace) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: sidebarWorkspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: workspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: recentRestoreTargetsQueryKey }),
      ]);
      toast.success(`已添加工作区：${workspace.name}`);
    },
  });

  const archiveMutation = useMutation({
    mutationFn: setWorkspacesArchived,
    onSuccess: async ({ workspaces, warning }, input) => {
      const nextItems = applyPinnedWorkspaceGrouping(workspaces.map((workspace) => buildWorkspaceSidebarItem(workspace, recentRestoreTargetByWorkspaceId.get(workspace.id))), pinnedWorkspaceIdsRef.current);
      setOrderedWorkspaceItems(nextItems);
      latestOrderedWorkspaceItemsRef.current = nextItems;
      if (warning) toast.warning(warning);
      queryClient.setQueryData(sidebarWorkspacesQueryKey, workspaces);
      queryClient.setQueryData(workspacesQueryKey, workspaces);
      const changed = new Set(input.workspaceIds);
      persistSelectedWorkspaceIds(selectedWorkspaceIdsRef.current.filter((id) => !changed.has(id)));
      if (input.archived && selectedHomeWorkspaceId && changed.has(selectedHomeWorkspaceId)) {
        setSelectedHomeWorkspaceId(workspaces.find((item) => !item.isArchived)?.id ?? null);
      }
      await queryClient.invalidateQueries({ queryKey: recentRestoreTargetsQueryKey });
      toast.success(`${input.archived ? "已归档" : "已恢复"} ${input.workspaceIds.length} 个项目`);
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });

  const exportWorkspaceListMutation = useMutation({ mutationFn: exportWorkspaceList });

  const updateWorkspaceDescriptionMutation = useMutation({
    mutationFn: updateWorkspaceDescription,
    onSuccess: async (workspace) => {
      setOrderedWorkspaceItems((current) => {
        const nextItems = current.map((item) =>
          item.workspace.id === workspace.id
            ? buildWorkspaceSidebarItem(workspace, item.recentRestoreTarget)
            : item,
        );
        latestOrderedWorkspaceItemsRef.current = nextItems;
        return nextItems;
      });
      queryClient.setQueryData<WorkspaceRecord[] | undefined>(sidebarWorkspacesQueryKey, (current) =>
        replaceWorkspaceRecord(current, workspace),
      );
      queryClient.setQueryData<WorkspaceRecord[] | undefined>(workspacesQueryKey, (current) =>
        replaceWorkspaceRecord(current, workspace),
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: sidebarWorkspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: workspacesQueryKey }),
      ]);
      toast.success("项目备注已保存");
    },
  });

  const removeWorkspaceMutation = useMutation({
    mutationFn: removeWorkspaceRegistration,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: sidebarWorkspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: workspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: recentRestoreTargetsQueryKey }),
      ]);
      toast.success("工作区已从 Bexo Studio 中移除");
    },
  });
  const openWorkspaceTerminalMutation = useMutation({
    mutationFn: ({ workspaceId }: { workspaceId: string; workspaceName: string }) =>
      openWorkspaceTerminal(workspaceId),
    onSuccess: (result, variables) => {
      toast.success(`已在终端中打开 ${variables.workspaceName}`, {
        description: result.workspacePath,
      });
    },
  });
  const openCodexHistoryWindowMutation = useMutation({
    mutationFn: ({ workspaceId }: { workspaceId: string; workspaceName: string }) =>
      openCodexHistoryWindow(workspaceId),
    onSuccess: (_result, variables) => {
      toast.success(`已打开 ${variables.workspaceName} 的 Codex 历史窗口`);
    },
  });
  const openWorkspaceInEditorMutation = useMutation({
    mutationFn: ({
      workspaceId,
      editorKey,
    }: {
      workspaceId: string;
      workspaceName: string;
      editorKey: WorkspaceEditorKey;
    }) => openWorkspaceInEditor(workspaceId, editorKey),
    onSuccess: (result, variables) => {
      toast.success(`已用 ${result.editorLabel} 打开 ${variables.workspaceName}`, {
        description: result.workspacePath,
      });
    },
  });
  const updateWorkspaceEditorMutation = useMutation({
    mutationFn: async ({
      workspace,
      project,
      editorKey,
    }: {
      workspace: WorkspaceRecord;
      project: ProjectRecord;
      editorKey: WorkspaceEditorKey;
    }) => {
      await upsertProject(buildProjectEditorUpdatePayload(project, editorKey));
      return { workspace, projectId: project.id, editorKey };
    },
    onSuccess: async ({ workspace, projectId, editorKey }) => {
      setOrderedWorkspaceItems((current) => {
        const nextItems = current.map((item) =>
          item.workspace.id === workspace.id
            ? { ...item, workspace: setWorkspaceProjectEditorKey(item.workspace, projectId, editorKey) }
            : item,
        );
        latestOrderedWorkspaceItemsRef.current = nextItems;
        return nextItems;
      });
      queryClient.setQueryData<WorkspaceRecord[] | undefined>(sidebarWorkspacesQueryKey, (current) =>
        current?.map((item) =>
          item.id === workspace.id ? setWorkspaceProjectEditorKey(item, projectId, editorKey) : item,
        ) ?? current,
      );
      queryClient.setQueryData<WorkspaceRecord[] | undefined>(workspacesQueryKey, (current) =>
        current?.map((item) =>
          item.id === workspace.id ? setWorkspaceProjectEditorKey(item, projectId, editorKey) : item,
        ) ?? current,
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: sidebarWorkspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: workspacesQueryKey }),
      ]);
    },
  });
  const runSelectedWorkspacesMutation = useMutation({
    mutationFn: async ({ workspaces }: { workspaces: WorkspaceSidebarItem[] }) => {
      const settledResults = await Promise.allSettled(
        workspaces.map(async (item) => ({
          workspace: item.workspace,
          result: await runWorkspaceTerminalCommands(item.workspace.id),
        })),
      );

      return settledResults.map((entry, index) => {
        const workspace = workspaces[index]?.workspace;
        if (!workspace) {
          throw new Error("工作区运行结果与请求队列不匹配");
        }

        if (entry.status === "fulfilled") {
          return {
            workspace,
            ok: true as const,
            result: entry.value.result,
          };
        }

        return {
          workspace,
          ok: false as const,
          error: getErrorSummary(entry.reason),
        };
      });
    },
    onSuccess: (results) => {
      const succeeded = results.filter((item) => item.ok);
      const failed = results.filter((item) => !item.ok);

      if (succeeded.length && !failed.length) {
        const staggerSeconds = Math.round(succeeded[0].result.staggerMs / 1000);
        toast.success(`已开始运行 ${succeeded.length} 个所选工作区`, {
          description: `每个工作区各开一个终端窗口，窗口内按顺序启动，间隔 ${staggerSeconds} 秒`,
        });
        void sendDesktopNotification({
          title: "Bexo Studio",
          body: `已开始运行 ${succeeded.length} 个所选工作区，每个工作区各开一个终端窗口`,
        });
        return;
      }

      if (succeeded.length) {
        const failedSummary = failed
          .map((item) => `${item.workspace.name}：${item.error.message}`)
          .join("；");
        toast.error(`已启动 ${succeeded.length} 个工作区，${failed.length} 个失败`, {
          description: failedSummary,
        });
        void sendDesktopNotification({
          title: "Bexo Studio",
          body: `已启动 ${succeeded.length} 个工作区，${failed.length} 个失败`,
        });
        return;
      }

      const failureMessage = failed
        .map((item) => `${item.workspace.name}：${item.error.message}`)
        .join("；");
      toast.error("运行所选工作区失败", {
        description: failureMessage,
      });
      void sendDesktopNotification({
        title: "Bexo Studio",
        body: `运行所选工作区失败：${failed.map((item) => item.workspace.name).join("、")}`,
      });
    },
    onError: (error) => {
      const summary = getErrorSummary(error);
      toast.error(summary.message);
      void sendDesktopNotification({
        title: "Bexo Studio",
        body: `运行所选工作区失败：${summary.message}`,
      });
    },
  });
  const reorderWorkspacesMutation = useMutation({
    mutationFn: async ({
      nextItems,
    }: {
      nextItems: WorkspaceSidebarItem[];
      successMessage?: string | null;
    }) => {
      return reorderWorkspaces({
        workspaceIds: nextItems.map((item) => item.workspace.id),
      });
    },
    onMutate: ({ nextItems }) => {
      setOrderedWorkspaceItems(nextItems);
      latestOrderedWorkspaceItemsRef.current = nextItems;
    },
    onSuccess: async (_, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: sidebarWorkspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: workspacesQueryKey }),
        queryClient.invalidateQueries({ queryKey: recentRestoreTargetsQueryKey }),
      ]);
      if (variables.successMessage !== null) {
        toast.success(variables.successMessage ?? "工作区顺序已更新");
      }
    },
    onError: (error) => {
      setOrderedWorkspaceItems(workspaceItemsRef.current);
      latestOrderedWorkspaceItemsRef.current = workspaceItemsRef.current;
      toast.error(getErrorSummary(error).message);
    },
  });

  useEffect(() => {
    setQuery("");
  }, [content.title]);

  useEffect(() => {
    selectedWorkspaceIdsRef.current = selectedWorkspaceIds;
  }, [selectedWorkspaceIds]);

  const persistedPinnedWorkspaceIds = useMemo(
    () =>
      normalizeWorkspacePinnedIds(
        (preferencesQuery.data ?? defaultAppPreferences).workspace.pinnedWorkspaceIds,
      ),
    [preferencesQuery.data],
  );

  useEffect(() => {
    pinnedWorkspaceIdsRef.current = persistedPinnedWorkspaceIds;
  }, [persistedPinnedWorkspaceIds]);

  const recentRestoreTargetByWorkspaceId = useMemo(() => {
    return new Map(
      (recentRestoreTargetsQuery.data ?? []).map((target) => [target.workspaceId, target] as const),
    );
  }, [recentRestoreTargetsQuery.data]);

  const workspaceItems = useMemo<WorkspaceSidebarItem[]>(() => {
    const baseItems = (workspaceQuery.data ?? []).map((workspace) =>
      buildWorkspaceSidebarItem(workspace, recentRestoreTargetByWorkspaceId.get(workspace.id)),
    );

    return applyPinnedWorkspaceGrouping(baseItems, persistedPinnedWorkspaceIds);
  }, [persistedPinnedWorkspaceIds, recentRestoreTargetByWorkspaceId, workspaceQuery.data]);

  useEffect(() => {
    workspaceItemsRef.current = workspaceItems;
    setOrderedWorkspaceItems((current) => {
      const nextOrderedItems = reconcileWorkspaceSidebarItems(current, workspaceItems);
      latestOrderedWorkspaceItemsRef.current = nextOrderedItems;
      return nextOrderedItems;
    });
  }, [workspaceItems]);

  const persistedSelectedWorkspaceIds = useMemo(
    () =>
      normalizeWorkspaceSelectionIds(
        (preferencesQuery.data ?? defaultAppPreferences).workspace.selectedWorkspaceIds,
      ),
    [preferencesQuery.data],
  );

  const persistSelectedWorkspaceIds = useCallback(
    (nextSelectedWorkspaceIds: string[]) => {
      const normalizedWorkspaceIds = normalizeWorkspaceSelectionIds(nextSelectedWorkspaceIds);
      setSelectedWorkspaceIds(normalizedWorkspaceIds);
      selectedWorkspaceIdsRef.current = normalizedWorkspaceIds;

      const optimisticPreferences = withWorkspaceSelectedIds(
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ?? defaultAppPreferences,
        normalizedWorkspaceIds,
      );
      queryClient.setQueryData(appPreferencesQueryKey, optimisticPreferences);

      persistSelectedWorkspaceIdsQueueRef.current = persistSelectedWorkspaceIdsQueueRef.current
        .catch(() => undefined)
        .then(async () => {
          const basePreferences =
            queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
            defaultAppPreferences;
          const nextPreferences = withWorkspaceSelectedIds(
            basePreferences,
            selectedWorkspaceIdsRef.current,
          );

          queryClient.setQueryData(appPreferencesQueryKey, nextPreferences);
          const updatedPreferences = await updatePreferencesMutation.mutateAsync({
            workspace: {
              selectedWorkspaceIds: selectedWorkspaceIdsRef.current,
            },
          });
          queryClient.setQueryData(appPreferencesQueryKey, updatedPreferences);
        })
        .catch((error) => {
          toast.error(getErrorSummary(error).message);
        });
    },
    [queryClient, updatePreferencesMutation],
  );

  const persistPinnedWorkspaceIds = useCallback(
    (nextPinnedWorkspaceIds: string[]) => {
      const normalizedWorkspaceIds = normalizeWorkspacePinnedIds(nextPinnedWorkspaceIds);
      pinnedWorkspaceIdsRef.current = normalizedWorkspaceIds;

      const optimisticPreferences = withWorkspacePinnedIds(
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ?? defaultAppPreferences,
        normalizedWorkspaceIds,
      );
      queryClient.setQueryData(appPreferencesQueryKey, optimisticPreferences);

      persistPinnedWorkspaceIdsQueueRef.current = persistPinnedWorkspaceIdsQueueRef.current
        .catch(() => undefined)
        .then(async () => {
          const basePreferences =
            queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
            defaultAppPreferences;
          const nextPreferences = withWorkspacePinnedIds(
            basePreferences,
            pinnedWorkspaceIdsRef.current,
          );

          queryClient.setQueryData(appPreferencesQueryKey, nextPreferences);
          const updatedPreferences = await updatePreferencesMutation.mutateAsync({
            workspace: {
              pinnedWorkspaceIds: pinnedWorkspaceIdsRef.current,
            },
          });
          queryClient.setQueryData(appPreferencesQueryKey, updatedPreferences);
        });

      return persistPinnedWorkspaceIdsQueueRef.current;
    },
    [queryClient, updatePreferencesMutation],
  );

  useEffect(() => {
    setSelectedWorkspaceIds((current) =>
      current.filter((workspaceId) =>
        orderedWorkspaceItems.some((item) => item.workspace.id === workspaceId),
      ),
    );
  }, [orderedWorkspaceItems]);

  useEffect(() => {
    if (content.dataSource !== "workspaces") {
      return;
    }

    setSelectedWorkspaceIds((current) =>
      areStringArraysEqual(current, persistedSelectedWorkspaceIds)
        ? current
        : persistedSelectedWorkspaceIds,
    );
    selectedWorkspaceIdsRef.current = persistedSelectedWorkspaceIds;
  }, [content.dataSource, persistedSelectedWorkspaceIds]);

  useEffect(() => {
    if (content.dataSource !== "workspaces" || !workspaceQuery.isSuccess) {
      return;
    }

    const validWorkspaceIds = new Set(
      (workspaceQuery.data ?? []).filter((item) => item.isArchived === archivedView).map((item) => item.id),
    );
    const filteredSelection = selectedWorkspaceIdsRef.current.filter((workspaceId) =>
      validWorkspaceIds.has(workspaceId),
    );

    if (areStringArraysEqual(filteredSelection, selectedWorkspaceIdsRef.current)) {
      return;
    }

    persistSelectedWorkspaceIds(filteredSelection);
  }, [
    content.dataSource,
    orderedWorkspaceItems,
    persistSelectedWorkspaceIds,
    workspaceQuery.isSuccess,
    workspaceQuery.data,
    archivedView,
  ]);

  useEffect(() => {
    if (content.dataSource !== "workspaces" || !workspaceQuery.isSuccess) {
      return;
    }

    const validWorkspaceIds = new Set((workspaceQuery.data ?? []).map((item) => item.id));
    const filteredPinnedIds = pinnedWorkspaceIdsRef.current.filter((workspaceId) =>
      validWorkspaceIds.has(workspaceId),
    );

    if (areStringArraysEqual(filteredPinnedIds, pinnedWorkspaceIdsRef.current)) {
      return;
    }

    void persistPinnedWorkspaceIds(filteredPinnedIds).catch((error) => {
      toast.error(getErrorSummary(error).message);
    });
  }, [
    content.dataSource,
    orderedWorkspaceItems,
    persistPinnedWorkspaceIds,
    workspaceQuery.isSuccess,
    workspaceQuery.data,
  ]);

  useEffect(() => {
    if (content.dataSource !== "workspaces") {
      return;
    }

    const activeItems = workspaceScope(orderedWorkspaceItems, false);
    if (!activeItems.length) {
      setSelectedHomeWorkspaceId(null);
      return;
    }

    if (!selectedHomeWorkspaceId) {
      setSelectedHomeWorkspaceId(activeItems[0].workspace.id);
      return;
    }

    const stillExists = activeItems.some(
      (item) => item.workspace.id === selectedHomeWorkspaceId,
    );
    if (!stillExists) {
      setSelectedHomeWorkspaceId(activeItems[0].workspace.id);
    }
  }, [
    content.dataSource,
    orderedWorkspaceItems,
    selectedHomeWorkspaceId,
    setSelectedHomeWorkspaceId,
  ]);

  const resolvedItems = useMemo<SidebarItem[]>(() => {
    if (content.dataSource !== "workspaces") {
      return content.items;
    }

    return orderedWorkspaceItems.map(({ workspace: _workspace, ...item }) => item);
  }, [content.dataSource, content.items, orderedWorkspaceItems]);

  const filteredItems = useMemo(() => {
    if (!resolvedItems.length) {
      return [];
    }

    return filterWorkspaceItems(resolvedItems, query);
  }, [query, resolvedItems]);

  const filteredWorkspaceItems = useMemo(() => {
    if (content.dataSource !== "workspaces") {
      return [];
    }

    if (!orderedWorkspaceItems.length) {
      return [];
    }

    return filterWorkspaceItems(workspaceScope(orderedWorkspaceItems, archivedView), query);
  }, [content.dataSource, orderedWorkspaceItems, query, archivedView]);

  const archivedMatchCount = useMemo(() => filterWorkspaceItems(workspaceScope(orderedWorkspaceItems, true), query).length, [orderedWorkspaceItems, query]);
  useEffect(() => { setKeyboardId(null); }, [query, archivedView]);
  useEffect(() => {
    if (content.dataSource !== "workspaces") return;
    const handleKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k" && !Array.from(document.querySelectorAll(".ant-modal-wrap")).some((element) => element.getClientRects().length > 0)) {
        event.preventDefault(); searchRef.current?.focus({ cursor: "all" });
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [content.dataSource]);

  function switchArchiveView(value: boolean) {
    setArchivedView(value);
    persistSelectedWorkspaceIds([]);
  }

  function archiveWorkspaces(ids: string[], archived: boolean) {
    if (!ids.length || archiveMutation.isPending || reorderWorkspacesMutation.isPending || runSelectedWorkspacesMutation.isPending) return;
    archiveMutation.mutate({ workspaceIds: ids, archived });
  }

  const visibleWorkspaceIds = useMemo(
    () => filteredWorkspaceItems.map((item) => item.workspace.id),
    [filteredWorkspaceItems],
  );

  const workspaceEditorOptions = useMemo(
    () =>
      buildWorkspaceEditorOptions(
        restoreCapabilitiesQuery.data,
        preferencesQuery.data ?? defaultAppPreferences,
      ),
    [preferencesQuery.data, restoreCapabilitiesQuery.data],
  );
  const workspaceDropEnabled =
    content.dataSource === "workspaces" && desktopRuntimeAvailable;

  const setWorkspaceDropActiveStable = useCallback((nextActive: boolean) => {
    workspaceDropActiveRef.current = nextActive;
  }, []);

  async function handleCreateWorkspace() {
    if (!desktopRuntimeAvailable || registerWorkspaceMutation.isPending) {
      return;
    }

    try {
      const selectedDirectory = await open({
        directory: true,
        multiple: false,
        recursive: true,
        title: "选择工作区文件夹",
      });

      if (!selectedDirectory || Array.isArray(selectedDirectory)) {
        return;
      }

      setWorkspaceCreationPath(selectedDirectory);
      setWorkspaceCreationDescription("");
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  function handleCancelCreateWorkspace() {
    if (registerWorkspaceMutation.isPending) {
      return;
    }
    setWorkspaceCreationPath(null);
    setWorkspaceCreationDescription("");
  }

  async function handleConfirmCreateWorkspace() {
    const path = workspaceCreationPath;
    if (!path || registerWorkspaceMutation.isPending) {
      return;
    }

    try {
      const workspace = await registerWorkspaceMutation.mutateAsync({
        path,
        description: workspaceCreationDescription,
      });
      setSelectedHomeWorkspaceId(workspace.id);
      handleCancelCreateWorkspace();
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  async function handleRemoveWorkspace(workspace: WorkspaceRecord) {
    if (removeWorkspaceMutation.isPending) {
      return;
    }

    try {
      await removeWorkspaceMutation.mutateAsync(workspace.id);
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  const handleDroppedWorkspacePaths = useCallback(
    async (paths: string[]) => {
      const droppedPaths = normalizeDroppedWorkspacePaths(paths);
      if (!droppedPaths.length) {
        return;
      }

      const dropSignature = buildDroppedWorkspaceSignature(droppedPaths);
      const now = Date.now();
      const recentDrop = recentWorkspaceDropRef.current;
      if (
        recentDrop &&
        recentDrop.signature === dropSignature &&
        now - recentDrop.timestamp < 1500
      ) {
        return;
      }

      if (registerWorkspaceMutation.isPending || workspaceDropPendingRef.current) {
        toast.error("正在创建工作区，请稍后再试");
        return;
      }

      recentWorkspaceDropRef.current = {
        signature: dropSignature,
        timestamp: now,
      };
      workspaceDropPendingRef.current = true;
      setWorkspaceDropPending(true);

      try {
        for (const path of droppedPaths) {
          try {
            await registerWorkspaceMutation.mutateAsync({ path, description: null });
          } catch (error) {
            const summary = getErrorSummary(error);
            if (summary.code === "WORKSPACE_PATH_ALREADY_REGISTERED") {
              toast.error("该文件夹已经注册为工作区", {
                description: summary.details?.path ?? path,
              });
              continue;
            }

            if (summary.code === "INVALID_WORKSPACE_PATH") {
              toast.error("只能拖入已存在的文件夹", {
                description: summary.details?.path ?? path,
              });
              continue;
            }

            toast.error(summary.message, {
              description: summary.details?.path ?? path,
            });
          }
        }
      } finally {
        workspaceDropPendingRef.current = false;
        setWorkspaceDropPending(false);
      }
    },
    [registerWorkspaceMutation],
  );

  useEffect(() => {
    if (!workspaceDropEnabled) {
      workspaceDropLastInsideAtRef.current = 0;
      setWorkspaceDropActiveStable(false);
      return;
    }

    let cancelled = false;
    let windowUnlisten: (() => void) | undefined;
    let scaleFactor = 1;

    const clearWorkspaceDropDeactivateTimeout = () => {
      if (workspaceDropDeactivateTimeoutRef.current !== null) {
        window.clearTimeout(workspaceDropDeactivateTimeoutRef.current);
        workspaceDropDeactivateTimeoutRef.current = null;
      }
    };

    const scheduleWorkspaceDropDeactivate = () => {
      clearWorkspaceDropDeactivateTimeout();
      workspaceDropDeactivateTimeoutRef.current = window.setTimeout(() => {
        workspaceDropDeactivateTimeoutRef.current = null;
        if (Date.now() - workspaceDropLastInsideAtRef.current >= 140) {
          setWorkspaceDropActiveStable(false);
        }
      }, 140);
    };

    const handleDragDropEvent = async (event: { payload: DragDropEvent }) => {
      if (cancelled) {
        return;
      }

      const payload = event.payload;
      if (payload.type === "leave") {
        scheduleWorkspaceDropDeactivate();
        return;
      }

      const insideDropZone =
        isPositionInsideElement(workspaceDropContainerRef.current, payload, scaleFactor);

      if (payload.type === "enter" || payload.type === "over") {
        if (insideDropZone) {
          workspaceDropLastInsideAtRef.current = Date.now();
          clearWorkspaceDropDeactivateTimeout();
          if (!workspaceDropActiveRef.current) {
            setWorkspaceDropActiveStable(true);
          }
        } else {
          scheduleWorkspaceDropDeactivate();
        }
        return;
      }

      const dropAccepted =
        insideDropZone ||
        workspaceDropActiveRef.current ||
        Date.now() - workspaceDropLastInsideAtRef.current <= 220;
      clearWorkspaceDropDeactivateTimeout();
      setWorkspaceDropActiveStable(false);
      if (!dropAccepted) {
        return;
      }

      await handleDroppedWorkspacePaths(payload.paths);
    };

    const bindDragDrop = async () => {
      try {
        scaleFactor = await getCurrentWindow().scaleFactor();
        windowUnlisten = await getCurrentWindow().onDragDropEvent(handleDragDropEvent);
      } catch {
        workspaceDropLastInsideAtRef.current = 0;
        setWorkspaceDropActiveStable(false);
      }
    };

    void bindDragDrop();

    return () => {
      cancelled = true;
      clearWorkspaceDropDeactivateTimeout();
      workspaceDropLastInsideAtRef.current = 0;
      setWorkspaceDropActiveStable(false);
      windowUnlisten?.();
    };
  }, [handleDroppedWorkspacePaths, setWorkspaceDropActiveStable, workspaceDropEnabled]);

  async function handleCopyWorkspacePath(workspace: WorkspaceRecord) {
    const workspacePath = resolveWorkspacePath(workspace);
    if (!workspacePath) {
      toast.error("当前工作区没有可复制的目录路径");
      return;
    }

    try {
      await copyTextToClipboard(workspacePath, "工作区路径为空");
      toast.success("工作区路径已复制", {
        description: workspacePath,
      });
    } catch (error) {
      toast.error(getClipboardErrorMessage(error, "复制工作区路径失败"));
    }
  }

  async function handleOpenCodexHistory(workspace: WorkspaceRecord) {
    const workspacePath = resolveWorkspacePath(workspace);
    if (!workspacePath) {
      toast.error("当前工作区没有可查看的目录路径");
      return;
    }

    if (!desktopRuntimeAvailable) {
      toast.error("当前页面需要在桌面应用中查看 Codex 历史");
      return;
    }

    try {
      await openCodexHistoryWindowMutation.mutateAsync({
        workspaceId: workspace.id,
        workspaceName: workspace.name,
      });
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  async function handleOpenWorkspaceTerminal(workspace: WorkspaceRecord) {
    try {
      await openWorkspaceTerminalMutation.mutateAsync({
        workspaceId: workspace.id,
        workspaceName: workspace.name,
      });
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  async function handleOpenWorkspaceDirectory(workspace: WorkspaceRecord) {
    const workspacePath = resolveWorkspacePath(workspace);
    if (!workspacePath) {
      toast.error("当前工作区没有可打开的目录路径");
      return;
    }

    if (!desktopRuntimeAvailable) {
      toast.error("当前页面需要在桌面应用中打开工作区目录");
      return;
    }

    try {
      await revealItemInDir(workspacePath);
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  async function handleOpenWorkspaceInEditor(
    workspace: WorkspaceRecord,
    editorKey = resolveWorkspaceEditorKey(workspace, workspaceEditorOptions),
  ) {
    const workspacePath = resolveWorkspacePath(workspace);
    if (!workspacePath) {
      toast.error("当前工作区没有可打开的目录路径");
      return;
    }

    if (!desktopRuntimeAvailable) {
      toast.error("当前页面需要在桌面应用中打开工作区编辑器");
      return;
    }

    try {
      await openWorkspaceInEditorMutation.mutateAsync({
        workspaceId: workspace.id,
        workspaceName: workspace.name,
        editorKey,
      });
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  async function handleWorkspaceEditorChange(
    workspace: WorkspaceRecord,
    editorKey: WorkspaceEditorKey,
  ) {
    const project = resolveWorkspacePrimaryProject(workspace);
    if (!project) {
      toast.error("当前工作区没有可保存编辑器类型的项目");
      return;
    }

    if (editorKey === resolveWorkspaceEditorKey(workspace, workspaceEditorOptions)) {
      return;
    }

    const targetOption = workspaceEditorOptions.find((option) => option.key === editorKey);
    if (!targetOption?.available) {
      toast.error(targetOption?.message ?? "当前编辑器不可用");
      return;
    }

    try {
      await updateWorkspaceEditorMutation.mutateAsync({
        workspace,
        project,
        editorKey,
      });
      toast.success(`${workspace.name} 已切换为 ${targetOption.label}`);
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  function moveWorkspace(source: string, target: string) {
    if (reorderWorkspacesMutation.isPending || archiveMutation.isPending || query.trim()) return;
    const current = latestOrderedWorkspaceItemsRef.current;
    const nextItems = moveWorkspaceWithinGroup(current, source, target, pinnedWorkspaceIdsRef.current);
    if (nextItems !== current) reorderWorkspacesMutation.mutate({ nextItems });
  }

  function moveWorkspaceStep(id: string, direction: number) {
    const pinned = new Set(pinnedWorkspaceIdsRef.current);
    const group = filteredWorkspaceItems.filter((item) => pinned.has(item.workspace.id) === pinned.has(id));
    const index = group.findIndex((item) => item.workspace.id === id);
    const target = group[index + direction];
    if (target) moveWorkspace(id, target.workspace.id);
  }

  async function handleWorkspacePinnedChange(workspace: WorkspaceRecord, nextPinned: boolean) {
    if (archiveMutation.isPending || reorderWorkspacesMutation.isPending || updatePreferencesMutation.isPending) {
      return;
    }

    const currentPinnedIds = pinnedWorkspaceIdsRef.current;
    const nextPinnedIds = nextPinned
      ? normalizeWorkspacePinnedIds([workspace.id, ...currentPinnedIds])
      : currentPinnedIds.filter((workspaceId) => workspaceId !== workspace.id);

    const currentItems = latestOrderedWorkspaceItemsRef.current;
    const movedToFrontItems = nextPinned
      ? moveWorkspaceItemToIndex(currentItems, workspace.id, 0)
      : currentItems;
    const nextItems = applyPinnedWorkspaceGrouping(movedToFrontItems, nextPinnedIds);

    try {
      await persistPinnedWorkspaceIds(nextPinnedIds);

      if (!isSameWorkspaceOrder(currentItems.map((item) => item.workspace.id), nextItems)) {
        await reorderWorkspacesMutation.mutateAsync({
          nextItems,
          successMessage: null,
        });
      } else {
        setOrderedWorkspaceItems(nextItems);
        latestOrderedWorkspaceItemsRef.current = nextItems;
      }

      toast.success(nextPinned ? `已置顶：${workspace.name}` : `已取消置顶：${workspace.name}`);
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }

  function handleWorkspaceSelectionChange(workspaceId: string, checked: boolean) {
    const current = selectedWorkspaceIdsRef.current;
    const nextSelectedWorkspaceIds = checked
      ? (current.includes(workspaceId) ? current : [...current, workspaceId])
      : current.filter((id) => id !== workspaceId);
    persistSelectedWorkspaceIds(nextSelectedWorkspaceIds);
  }

  function handleSelectAllVisibleWorkspaces() {
    if (!visibleWorkspaceIds.length) {
      return;
    }

    const merged = new Set(selectedWorkspaceIdsRef.current);
    for (const workspaceId of visibleWorkspaceIds) {
      merged.add(workspaceId);
    }
    persistSelectedWorkspaceIds(Array.from(merged));
  }

  function handleClearSelectedWorkspaces() {
    if (!selectedWorkspaceIdsRef.current.length) {
      return;
    }

    persistSelectedWorkspaceIds([]);
  }

  function getSelectedWorkspaceRunQueue() {
    return orderedWorkspaceItems.filter((item) =>
      !item.workspace.isArchived && selectedWorkspaceIdsRef.current.includes(item.workspace.id),
    );
  }

  async function handleRunSelectedWorkspaces(selectedWorkspaces = getSelectedWorkspaceRunQueue()) {
    if (!selectedWorkspaces.length) {
      toast.error("请先勾选至少一个工作区");
      return;
    }

    await runSelectedWorkspacesMutation.mutateAsync({
      workspaces: selectedWorkspaces,
    });
  }

  function confirmRunSelectedWorkspaces() {
    const selectedWorkspaces = getSelectedWorkspaceRunQueue();
    if (!selectedWorkspaces.length) {
      toast.error("请先勾选至少一个工作区");
      return;
    }

    const previewNames = selectedWorkspaces
      .slice(0, 3)
      .map((item) => item.workspace.name)
      .join("、");
    const remainingCount = selectedWorkspaces.length - Math.min(selectedWorkspaces.length, 3);

    Modal.confirm({
      centered: true,
      cancelText: "取消",
      okText: "确认运行",
      title: "确认运行所选工作区？",
      content: (
        <div className="space-y-1 text-[12px] leading-5 text-fg-2">
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-fg-2">
            即将按左侧当前顺序运行 {selectedWorkspaces.length} 个工作区，每个工作区都会各开一个系统终端窗口。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-fg-2">
            窗口内命令会继续按顺序执行，并保持 10 秒错峰。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-fg-2">
            {remainingCount > 0
              ? `本次将运行：${previewNames} 等 ${selectedWorkspaces.length} 个工作区。`
              : `本次将运行：${previewNames}。`}
          </Typography.Paragraph>
        </div>
      ),
      onOk: async () => {
        await handleRunSelectedWorkspaces(selectedWorkspaces);
      },
    });
  }

  async function handleImportWorkspaceList() {
    if (!desktopRuntimeAvailable || workspaceImportSourcePath || exportWorkspaceListMutation.isPending) {
      return;
    }
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: "选择 Bexo Studio 工作区列表",
        filters: [
          {
            name: "Bexo Studio 工作区列表",
            extensions: ["json"],
          },
        ],
      });
      if (typeof selected === "string") {
        setWorkspaceImportSourcePath(selected);
      }
    } catch (error) {
      const resolved = getErrorSummary(error);
      toast.error("无法打开工作区列表文件", {
        description: `${resolved.message}（${resolved.code}）`,
      });
    }
  }

  function confirmExportWorkspaceList() {
    if (!desktopRuntimeAvailable || exportWorkspaceListMutation.isPending) {
      return;
    }
    Modal.confirm({
      centered: true,
      cancelText: "取消",
      okText: "选择保存位置",
      title: "导出全部工作区项目列表？",
      content: (
        <div className="space-y-2 text-[12px] leading-5 text-fg-2">
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-fg-2">
            将导出当前全部工作区、项目启动配置、终端命令和置顶状态，用于在本机或另一台电脑恢复列表。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-warning">
            文件会包含绝对目录路径、终端命令和参数，请勿上传到公开仓库或发送给不可信的人。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[11px] !leading-5 !text-fg-2">
            不会导出 Codex/OSS 凭据、日志、快照、恢复历史或全局设置。
          </Typography.Paragraph>
        </div>
      ),
      onOk: async () => {
        try {
          const destinationPath = await save({
            title: "保存 Bexo Studio 工作区列表",
            defaultPath: buildWorkspaceExportFileName(),
            filters: [
              {
                name: "Bexo Studio 工作区列表",
                extensions: ["json"],
              },
            ],
          });
          if (!destinationPath) {
            return;
          }
          const result = await exportWorkspaceListMutation.mutateAsync({ destinationPath });
          toast.success("工作区项目列表已导出", {
            description: `${result.workspaceCount} 个工作区、${result.projectCount} 个项目、${result.launchTaskCount} 条启动任务`,
          });
          if (result.warnings.length) {
            toast.warning("导出完成，但有需要留意的提示", {
              description: result.warnings.join("；"),
              duration: 8_000,
            });
          }
        } catch (error) {
          const resolved = getErrorSummary(error);
          toast.error("导出工作区项目列表失败", {
            description: `${resolved.message}（${resolved.code}）`,
          });
          throw error;
        }
      },
    });
  }

  async function handleWorkspaceImportCompleted() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: sidebarWorkspacesQueryKey }),
      queryClient.invalidateQueries({ queryKey: workspacesQueryKey }),
      queryClient.invalidateQueries({ queryKey: recentRestoreTargetsQueryKey }),
      queryClient.invalidateQueries({ queryKey: appPreferencesQueryKey }),
    ]);
  }

  const workspaceActionItems: MenuProps["items"] = [
    {
      key: "select-all",

      icon: <CheckSquareOutlined />,
      label: "全选当前搜索结果",
      disabled: !desktopRuntimeAvailable || !filteredWorkspaceItems.length,
    },
    {
      key: "clear-selection",
      icon: <CloseOutlined />,
      label: "取消全选工作区",
      disabled: !desktopRuntimeAvailable || !selectedWorkspaceIds.length,
    },
    {
      key: "run-selected",
      icon: <CaretRightOutlined />,
      label: "运行所选工作区",
      disabled:
        !desktopRuntimeAvailable ||
        !getSelectedWorkspaceRunQueue().length || archivedView || archiveMutation.isPending ||
        runSelectedWorkspacesMutation.isPending,
    },
    {
      type: "divider",
    },
    {
      key: "archive-selected",
      label: archivedView ? "恢复所选项目" : "归档所选项目",
      disabled: !selectedWorkspaceIds.length || archiveMutation.isPending || runSelectedWorkspacesMutation.isPending,
    },
    {
      key: "import-list",
      icon: <UploadOutlined />,
      label: "导入项目列表",
      disabled:
        !desktopRuntimeAvailable ||
        Boolean(workspaceImportSourcePath) ||
        exportWorkspaceListMutation.isPending,
    },
    {
      key: "export-list",
      icon: <DownloadOutlined />,
      label: "导出项目列表",
      disabled:
        !desktopRuntimeAvailable ||
        !workspaceQuery.data?.length ||
        exportWorkspaceListMutation.isPending,
    },
  ];

  async function handleWorkspaceActionClick(key: string) {
    if (key === "archive-selected") {
      archiveWorkspaces(workspaceScope(orderedWorkspaceItems, archivedView).filter((item) => selectedWorkspaceIds.includes(item.workspace.id)).map((item) => item.workspace.id), !archivedView);
      return;
    }
    if (key === "create") {
      await handleCreateWorkspace();
      return;
    }

    if (key === "select-all") {
      handleSelectAllVisibleWorkspaces();
      return;
    }

    if (key === "clear-selection") {
      handleClearSelectedWorkspaces();
      return;
    }

    if (key === "run-selected") {
      confirmRunSelectedWorkspaces();
      return;
    }

    if (key === "import-list") {
      await handleImportWorkspaceList();
      return;
    }

    if (key === "export-list") {
      confirmExportWorkspaceList();
    }
  }

  const workspaceReorderEnabled =
    content.dataSource === "workspaces" &&
    !query.trim() && !reorderWorkspacesMutation.isPending && !archiveMutation.isPending;
  const workspaceActionsBusy =
    registerWorkspaceMutation.isPending ||
    exportWorkspaceListMutation.isPending ||
    runSelectedWorkspacesMutation.isPending ||
    workspaceDropPending || archiveMutation.isPending;
  const workspaceCreationDescriptionCount = countUnicodeCharacters(workspaceCreationDescription);
  const workspaceCreationDescriptionError = validateWorkspaceDescription(
    workspaceCreationDescription,
  );

    const isWorkspaceSidebar = content.dataSource === "workspaces";
    const activeWorkspaceCount = workspaceScope(orderedWorkspaceItems, false).length;
    const archivedWorkspaceCount = workspaceScope(orderedWorkspaceItems, true).length;

    return (
      <aside
        className="relative flex h-full min-h-0 flex-col overflow-hidden bg-surface"
        ref={isWorkspaceSidebar ? workspaceDropContainerRef : undefined}
      >
      <PaneHeader
        actions={isWorkspaceSidebar ? (
          <>
            <Tooltip mouseEnterDelay={0.3} title="新建工作区（也可把文件夹拖进列表）">
              <Button
                aria-label="新建工作区"
                disabled={!desktopRuntimeAvailable || registerWorkspaceMutation.isPending}
                icon={<PlusOutlined />}
                loading={registerWorkspaceMutation.isPending}
                onClick={() => void handleCreateWorkspace()}
                size="small"
                type="text"
              />
            </Tooltip>
            <Dropdown
              menu={{
                items: workspaceActionItems,
                onClick: ({ key }) => void handleWorkspaceActionClick(key),
              }}
              placement="bottomRight"
              trigger={["click"]}
            >
              <Button
                aria-label="工作区批量操作"
                disabled={!desktopRuntimeAvailable}
                icon={<MoreOutlined />}
                loading={workspaceActionsBusy && !registerWorkspaceMutation.isPending}
                size="small"
                type="text"
              />
            </Dropdown>
          </>
        ) : null}
        meta={isWorkspaceSidebar ? activeWorkspaceCount : null}
        title={content.title}
      />
      {content.searchPlaceholder ? (
        <div className="shrink-0 space-y-1.5 border-b border-line px-2 py-2">
          <Input
            ref={searchRef}
            aria-label="搜索项目，Ctrl+K"
            onKeyDown={(event) => {
              if (!isWorkspaceSidebar || event.nativeEvent.isComposing) return;
              if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                event.preventDefault();
                const current = filteredWorkspaceItems.findIndex((item) => item.workspace.id === keyboardId);
                const index = Math.max(0, Math.min(filteredWorkspaceItems.length - 1, current + (event.key === "ArrowDown" ? 1 : -1)));
                setKeyboardId(filteredWorkspaceItems[index]?.workspace.id ?? null);
              } else if (event.key === "Enter" && !archivedView) {
                const target = filteredWorkspaceItems.find((item) => item.workspace.id === keyboardId) ?? filteredWorkspaceItems[0];
                if (target) setSelectedHomeWorkspaceId(target.workspace.id);
              } else if (event.key === "Escape") setQuery("");
            }}
            allowClear
            onChange={(event) => setQuery(event.target.value)}
            placeholder={content.searchPlaceholder}
            prefix={<SearchOutlined className="text-fg-3" />}
            value={query}
          />
          {isWorkspaceSidebar ? (
            selectedWorkspaceIds.length ? (
              <div className="flex h-6 items-center gap-1 text-[12px] text-fg-2">
                <span className="mr-auto">已选 {selectedWorkspaceIds.length} 项</span>
                {!archivedView ? (
                  <Button
                    disabled={!getSelectedWorkspaceRunQueue().length || archiveMutation.isPending}
                    loading={runSelectedWorkspacesMutation.isPending}
                    onClick={confirmRunSelectedWorkspaces}
                    size="small"
                    type="link"
                  >
                    运行
                  </Button>
                ) : null}
                <Button
                  disabled={runSelectedWorkspacesMutation.isPending}
                  loading={archiveMutation.isPending}
                  onClick={() => archiveWorkspaces(workspaceScope(orderedWorkspaceItems, archivedView).filter((item) => selectedWorkspaceIds.includes(item.workspace.id)).map((item) => item.workspace.id), !archivedView)}
                  size="small"
                  type="link"
                >
                  {archivedView ? "恢复" : "归档"}
                </Button>
                <Button onClick={handleClearSelectedWorkspaces} size="small" type="text">取消</Button>
              </div>
            ) : (
              <div aria-label="项目范围" className="flex h-6 items-center gap-3 px-1 text-[12px]" role="tablist">
                <button
                  aria-selected={!archivedView}
                  className={cn("h-6 border-b-2 transition-colors", !archivedView ? "border-primary font-medium text-fg" : "border-transparent text-fg-3 hover:text-fg-2")}
                  onClick={() => switchArchiveView(false)}
                  role="tab"
                  type="button"
                >
                  使用中 {activeWorkspaceCount}
                </button>
                <button
                  aria-selected={archivedView}
                  className={cn("h-6 border-b-2 transition-colors", archivedView ? "border-primary font-medium text-fg" : "border-transparent text-fg-3 hover:text-fg-2")}
                  onClick={() => switchArchiveView(true)}
                  role="tab"
                  type="button"
                >
                  已归档 {archivedWorkspaceCount}
                </button>
                {query.trim() && !archivedView && archivedMatchCount > 0 ? (
                  <button className="ml-auto text-fg-3 hover:text-primary" onClick={() => switchArchiveView(true)} type="button">
                    归档中另有 {archivedMatchCount} 项
                  </button>
                ) : null}
              </div>
            )
          ) : null}
        </div>
      ) : null}

        <motion.div
        className={cn(
          "min-h-0 flex-1 overflow-y-auto",
          content.dataSource === "workspaces" && "flex flex-col",
        )}
        layoutScroll={content.dataSource === "workspaces"}
        style={{ contain: "strict" }}
      >
      {content.dataSource === "workspaces" && filteredWorkspaceItems.length ? (
        <WorkspaceNavigationList items={filteredWorkspaceItems} pinnedIds={persistedPinnedWorkspaceIds}
          searching={Boolean(query.trim())} resetKey={`${archivedView}:${query}`} focusId={keyboardId}
          reorderEnabled={workspaceReorderEnabled} onMove={moveWorkspace}
          renderItem={(item) => (
                  <WorkspaceSidebarCard
                    desktopRuntimeAvailable={desktopRuntimeAvailable}
                    key={item.key}
                    editorKey={resolveWorkspaceEditorKey(item.workspace, workspaceEditorOptions)}
                    editorOptions={workspaceEditorOptions}
                    onCopyPath={() => void handleCopyWorkspacePath(item.workspace)}
                    onOpenCodexHistory={() => void handleOpenCodexHistory(item.workspace)}
                    onChangeEditor={(editorKey) =>
                      void handleWorkspaceEditorChange(item.workspace, editorKey)
                    }
                    onOpenDirectory={() => void handleOpenWorkspaceDirectory(item.workspace)}
                    onOpenInEditor={(editorKey) =>
                      void handleOpenWorkspaceInEditor(item.workspace, editorKey)
                    }
                    onOpenTerminal={() => void handleOpenWorkspaceTerminal(item.workspace)}
                    onTogglePinned={(pinned) =>
                      void handleWorkspacePinnedChange(item.workspace, pinned)
                    }
                    onRemove={() => void handleRemoveWorkspace(item.workspace)}
                    onSelect={() => { if (!item.workspace.isArchived) startTransition(() => setSelectedHomeWorkspaceId(item.workspace.id)); }}
                    onEditNote={() => setNoteEditor({ workspace: item.workspace, draft: item.workspace.description ?? "", error: null })}
                    onArchive={() => archiveWorkspaces([item.workspace.id], !item.workspace.isArchived)}
                    archivePending={archiveMutation.isPending || runSelectedWorkspacesMutation.isPending}
                    onMove={(direction) => moveWorkspaceStep(item.workspace.id, direction)}
                    keyboardActive={keyboardId === item.workspace.id}
                    onSelectionChange={(checked) =>
                      handleWorkspaceSelectionChange(item.workspace.id, checked)
                    }
                    openInEditorLoading={
                      openWorkspaceInEditorMutation.isPending &&
                      openWorkspaceInEditorMutation.variables?.workspaceId === item.workspace.id
                    }
                    openCodexHistoryDisabled={openCodexHistoryWindowMutation.isPending}
                    openCodexHistoryLoading={
                      openCodexHistoryWindowMutation.isPending &&
                      openCodexHistoryWindowMutation.variables?.workspaceId === item.workspace.id
                    }
                    openTerminalDisabled={openWorkspaceTerminalMutation.isPending}
                    openTerminalLoading={
                      openWorkspaceTerminalMutation.isPending &&
                      openWorkspaceTerminalMutation.variables?.workspaceId === item.workspace.id
                    }
                    editorSaving={
                      updateWorkspaceEditorMutation.isPending &&
                      updateWorkspaceEditorMutation.variables?.workspace.id === item.workspace.id
                    }
                    noteSaving={
                      updateWorkspaceDescriptionMutation.isPending &&
                      updateWorkspaceDescriptionMutation.variables?.workspaceId === item.workspace.id
                    }
                    removePending={removeWorkspaceMutation.isPending}
                    reorderEnabled={workspaceReorderEnabled}
                    pinned={persistedPinnedWorkspaceIds.includes(item.workspace.id)}
                    selected={selectedHomeWorkspaceId === item.workspace.id}
                    selectionChecked={selectedWorkspaceIds.includes(item.workspace.id)}
                    selectionActive={selectedWorkspaceIds.length > 0}
                    workspaceItem={item}
                  />
          )} />
      ) : content.dataSource !== "workspaces" && filteredItems.length ? (
        <div className="min-h-0 p-1">
          {filteredItems.map((item) => {
            const itemActive = item.href ? isSidebarRouteActive(location.pathname, item.href) : false;
            const className = cn(
              "flex h-8 items-center rounded-md px-2 text-[13px] transition-colors",
              itemActive ? "bg-selected font-medium !text-primary" : "!text-fg-2 hover:bg-hover hover:!text-fg",
            );
            return item.href ? (
              <Link aria-current={itemActive ? "page" : undefined} className={className} key={item.key} to={item.href}>
                {item.label}
              </Link>
            ) : (
              <div className={className} key={item.key}>{item.label}</div>
            );
          })}
        </div>
      ) : content.dataSource === "workspaces" ? (
        <div className="flex min-h-[160px] flex-1 items-center justify-center px-4">
          <Empty
            description={
              !desktopRuntimeAvailable
                ? "请在桌面应用中管理工作区"
                : workspaceQuery.error
                ? getErrorSummary(workspaceQuery.error).message
                : workspaceQuery.isLoading
                ? "正在加载工作区..."
                : query.trim()
                  ? "没有匹配的工作区"
                  : archivedView
                    ? "没有已归档的工作区"
                    : "点击右上角 + 或把文件夹拖到这里来添加工作区"
            }
            image={Empty.PRESENTED_IMAGE_SIMPLE}
          />
        </div>
      ) : null}
      </motion.div>

      <Modal
        centered
        cancelText="取消"
        confirmLoading={registerWorkspaceMutation.isPending}
        okButtonProps={{ disabled: Boolean(workspaceCreationDescriptionError) }}
        okText="创建工作区"
        onCancel={handleCancelCreateWorkspace}
        onOk={() => void handleConfirmCreateWorkspace()}
        open={Boolean(workspaceCreationPath)}
        title="添加工作区"
      >
        <div className="space-y-3">
          <div>
            <div className="text-[12px] text-fg-3">目录</div>
            <div className="mt-0.5 truncate text-[13px] text-fg" title={workspaceCreationPath ?? ""}>
              {workspaceCreationPath}
            </div>
          </div>
          <div className="text-[12px] text-fg-3">备注（可选）</div>
          <Input.TextArea
            autoFocus
            aria-label="项目备注"
            autoSize={{ minRows: 3, maxRows: 5 }}
            onChange={(event) => setWorkspaceCreationDescription(event.target.value)}
            placeholder="例如：活动报名系统，负责后端 API 和管理端"
            value={workspaceCreationDescription}
          />
          <div
            className={cn(
              "text-right text-[11px]",
              workspaceCreationDescriptionError ? "text-danger" : "text-fg-3",
            )}
          >
            {workspaceCreationDescriptionCount}/{MAX_WORKSPACE_DESCRIPTION_CHARS}
          </div>
          {workspaceCreationDescriptionError ? (
            <Typography.Text type="danger" className="block text-[11px] leading-4">
              {workspaceCreationDescriptionError}
            </Typography.Text>
          ) : null}
        </div>
      </Modal>

      <Modal title="项目备注" open={Boolean(noteEditor)} confirmLoading={updateWorkspaceDescriptionMutation.isPending}
        okText="保存" cancelText="取消" destroyOnHidden
        onCancel={() => {
          if (updateWorkspaceDescriptionMutation.isPending) return;
          if (noteEditor && noteEditor.draft !== (noteEditor.workspace.description ?? "")) {
            Modal.confirm({ title: "放弃未保存备注？", okText: "放弃修改", cancelText: "继续编辑", onOk: () => setNoteEditor(null) });
          } else setNoteEditor(null);
        }}
        onOk={async () => {
          if (!noteEditor || updateWorkspaceDescriptionMutation.isPending) return;
          const error = validateWorkspaceDescription(noteEditor.draft);
          if (error) { setNoteEditor({ ...noteEditor, error }); return; }
          try {
            await updateWorkspaceDescriptionMutation.mutateAsync({ workspaceId: noteEditor.workspace.id, description: noteEditor.draft });
            setNoteEditor(null);
          } catch (error) { setNoteEditor({ ...noteEditor, error: getErrorSummary(error).message }); }
        }}>
        <Input.TextArea disabled={updateWorkspaceDescriptionMutation.isPending} aria-label="项目备注" value={noteEditor?.draft ?? ""} rows={4}
          onChange={(event) => setNoteEditor((current) => current ? { ...current, draft: event.target.value, error: null } : null)} />
        <div className="mt-2 text-xs text-muted-foreground">{countUnicodeCharacters(noteEditor?.draft ?? "")} / {MAX_WORKSPACE_DESCRIPTION_CHARS}</div>
        {noteEditor?.error ? <div role="alert" className="text-[12px] text-danger">{noteEditor.error}</div> : null}
      </Modal>
      <WorkspaceTransferDialog
        onClose={() => setWorkspaceImportSourcePath(null)}
        onImported={handleWorkspaceImportCompleted}
        open={Boolean(workspaceImportSourcePath)}
        sourcePath={workspaceImportSourcePath}
      />

    </aside>
  );
}

type WorkspaceSidebarItem = SidebarItem & {
  description: string;
  note: string;
  workspace: WorkspaceRecord;
  recentRestoreTarget?: RecentRestoreTarget;
};

type WorkspaceEditorOption = {
  key: WorkspaceEditorKey;
  label: string;
  available: boolean;
  message: string;
};

type WorkspaceSidebarCardProps = {
  desktopRuntimeAvailable: boolean;
  onChangeEditor: (key: WorkspaceEditorKey) => void;
  onCopyPath: () => void;
  onEditNote: () => void;
  onOpenCodexHistory: () => void;
  onOpenDirectory: () => void;
  onOpenInEditor: (key: WorkspaceEditorKey) => void;
  onOpenTerminal: () => void;
  onTogglePinned: (pinned: boolean) => void;
  onRemove: () => void;
  onArchive: () => void;
  onMove: (direction: number) => void;
  onSelect: () => void;
  onSelectionChange: (checked: boolean) => void;
  openInEditorLoading: boolean;
  openCodexHistoryDisabled: boolean;
  openCodexHistoryLoading: boolean;
  openTerminalDisabled: boolean;
  openTerminalLoading: boolean;
  editorSaving: boolean;
  noteSaving: boolean;
  removePending: boolean;
  archivePending: boolean;
  reorderEnabled: boolean;
  keyboardActive: boolean;
  pinned: boolean;
  selected: boolean;
  selectionChecked: boolean;
  selectionActive: boolean;
  workspaceItem: WorkspaceSidebarItem;
  editorKey: WorkspaceEditorKey;
  editorOptions: WorkspaceEditorOption[];
};

function WorkspaceSidebarCard(props: WorkspaceSidebarCardProps) {
  const { workspaceItem: item, pinned, editorKey, editorOptions } = props;
  const archived = item.workspace.isArchived;
  const displayPath = splitWorkspacePath(item.description);
  const preferred = editorOptions.find((option) => option.key === editorKey);
  const editorDisabled = !props.desktopRuntimeAvailable || !resolveWorkspacePath(item.workspace) || !preferred?.available || archived;
  const menu: MenuProps = {
    items: [
      { key: "note", label: item.note ? "编辑备注" : "添加备注", icon: <EditOutlined />, disabled: props.noteSaving },
      { key: "pin", label: pinned ? "取消常用" : "设为常用", icon: <StarOutlined /> },
      { type: "divider" },
      { key: "history", label: "Codex 会话历史", icon: <HistoryOutlined />, disabled: props.openCodexHistoryDisabled },
      { key: "copy", label: "复制完整路径", icon: <CopyOutlined /> },
      { key: "folder", label: "在资源管理器中打开", icon: <FolderOpenOutlined /> },
      { key: "terminal", label: "打开终端", icon: <CodeOutlined />, disabled: props.openTerminalDisabled || archived },
      { key: "editor", label: "默认编辑器", children: editorOptions.map((option) => ({ key: `editor:${option.key}`, label: option.label, disabled: !option.available || props.editorSaving })) },
      { type: "divider" },
      { key: "up", label: "上移", disabled: !props.reorderEnabled },
      { key: "down", label: "下移", disabled: !props.reorderEnabled },
      { key: "archive", label: archived ? "恢复项目" : "归档项目", disabled: props.archivePending },
      { key: "remove", label: "从列表移除…", danger: true, disabled: props.removePending },
    ],
    onClick: ({ key, domEvent }) => {
      domEvent.stopPropagation();
      if (key.startsWith("editor:")) { props.onChangeEditor(key.slice(7) as WorkspaceEditorKey); return; }
      switch (key) {
        case "note": props.onEditNote(); break;
        case "pin": props.onTogglePinned(!pinned); break;
        case "history": props.onOpenCodexHistory(); break;
        case "copy": props.onCopyPath(); break;
        case "folder": props.onOpenDirectory(); break;
        case "terminal": props.onOpenTerminal(); break;
        case "up": props.onMove(-1); break;
        case "down": props.onMove(1); break;
        case "archive": props.onArchive(); break;
        case "remove": Modal.confirm({ title: `移除 ${item.label}？`, content: "从 Bexo Studio 移除工作区及其项目、启动任务配置，不删除磁盘文件。暂时不用建议归档。", okText: "移除", cancelText: "取消", okButtonProps: { danger: true }, onOk: props.onRemove }); break;
      }
    },
  };
  const actionBusy = props.openCodexHistoryLoading || props.openTerminalLoading || props.editorSaving || props.openInEditorLoading;
  const tooltip = (
    <div className="space-y-0.5">
      <div className="break-all">{item.description || "未配置目录"}</div>
      {item.note ? <div>{item.note}</div> : null}
      <div>最后运行：{formatLastRunAt(item.recentRestoreTarget)}</div>
    </div>
  );
  return (
    <Dropdown menu={menu} trigger={["contextMenu"]}>
      <div
        className={cn(
          "group flex h-[52px] items-center gap-1.5 rounded-md pl-0.5 pr-1 text-fg transition-colors",
          props.selected ? "bg-selected" : "hover:bg-hover",
          props.keyboardActive && "ring-1 ring-inset ring-primary",
          archived && "opacity-75",
        )}
      >
        <Checkbox
          aria-label={`选择 ${item.label}`}
          checked={props.selectionChecked}
          className={cn("shrink-0", !props.selectionActive && "reveal-on-hover")}
          onChange={(event) => props.onSelectionChange(event.target.checked)}
        />
        <Tooltip mouseEnterDelay={0.6} placement="right" title={tooltip} trigger={["hover", "focus"]}>
          <button
            aria-label={`${item.label}${archived ? "，已归档" : ""}`}
            className="block min-w-0 flex-1 text-left focus-visible:outline-none"
            onClick={props.onSelect}
            type="button"
          >
            <span className="flex min-w-0 items-baseline gap-2">
              <span className={cn("max-w-full shrink-0 truncate text-[13px] leading-5", props.selected ? "font-semibold" : "font-medium")}>
                {item.label}
              </span>
              {item.note ? (
                <span className="min-w-0 truncate text-[12px] leading-5 text-fg-3" title={item.note}>{item.note}</span>
              ) : null}
            </span>
            <span aria-label={`目录：${item.description}`} className="flex min-w-0 text-[12px] leading-5 text-fg-3">
              <span className="min-w-0 truncate">{displayPath.prefix}</span>
              <span className="max-w-[70%] shrink-0 truncate">{displayPath.leaf}</span>
            </span>
          </button>
        </Tooltip>
        <div className="flex shrink-0 items-center">
          <Button
            aria-label={pinned ? "取消常用" : "设为常用"}
            className={cn(!pinned && "reveal-on-hover")}
            icon={pinned ? <StarFilled /> : <StarOutlined />}
            onClick={() => props.onTogglePinned(!pinned)}
            size="small"
            style={{ color: pinned ? "#eab308" : undefined }}
            title={pinned ? "取消常用" : "设为常用"}
            type="text"
          />
          {archived ? (
            <Button loading={props.archivePending} onClick={props.onArchive} size="small" type="link">恢复</Button>
          ) : (
            <Tooltip title={preferred?.available ? `使用 ${preferred.label} 打开` : preferred?.message}>
              <Button
                aria-label={`使用 ${preferred?.label ?? "编辑器"} 打开 ${item.label}`}
                className="reveal-on-hover"
                data-force-visible={props.openInEditorLoading ? "true" : undefined}
                disabled={editorDisabled}
                loading={props.openInEditorLoading}
                onClick={() => props.onOpenInEditor(editorKey)}
                size="small"
                type="text"
              >
                <WorkspaceEditorGlyph editorKey={editorKey} label={preferred?.label ?? "编辑器"} />
              </Button>
            </Tooltip>
          )}
          <Dropdown menu={menu} trigger={["click"]}>
            <Button
              aria-label={`${item.label} 更多操作`}
              className="reveal-on-hover"
              data-force-visible={actionBusy ? "true" : undefined}
              icon={<MoreOutlined />}
              loading={props.openCodexHistoryLoading || props.openTerminalLoading || props.editorSaving}
              size="small"
              type="text"
            />
          </Dropdown>
        </div>
      </div>
    </Dropdown>
  );
}

function resolveWorkspacePath(workspace: WorkspaceRecord) {
  return resolveWorkspacePrimaryProject(workspace)?.path?.trim() || "";
}

function buildWorkspaceSidebarItem(
  workspace: WorkspaceRecord,
  recentRestoreTarget?: RecentRestoreTarget,
): WorkspaceSidebarItem {
  return {
    key: workspace.id,
    label: workspace.name,
    description:
      resolveWorkspacePath(workspace) || `${workspace.projects.length} 个项目`,
    note: workspace.description?.trim() || "",
    workspace,
    recentRestoreTarget,
  };
}

function replaceWorkspaceRecord(
  current: WorkspaceRecord[] | undefined,
  workspace: WorkspaceRecord,
) {
  return current?.map((item) => (item.id === workspace.id ? workspace : item)) ?? current;
}

function resolveWorkspacePrimaryProject(workspace: WorkspaceRecord) {
  return workspace.projects[0];
}

function resolveWorkspaceEditorKey(
  workspace: WorkspaceRecord,
  editorOptions: WorkspaceEditorOption[],
): WorkspaceEditorKey {
  return normalizeWorkspaceEditorKey(
    resolveWorkspacePrimaryProject(workspace)?.ideType,
    editorOptions,
  );
}

function buildProjectEditorUpdatePayload(
  project: ProjectRecord,
  editorKey: WorkspaceEditorKey,
) {
  return {
    id: project.id,
    workspaceId: project.workspaceId,
    name: project.name,
    path: project.path,
    platform: project.platform,
    terminalType: project.terminalType,
    ideType: editorKey,
    codexProfileId: project.codexProfileId ?? undefined,
    openTerminal: project.openTerminal,
    openIde: project.openIde,
    autoResumeCodex: project.autoResumeCodex,
    sortOrder: project.sortOrder,
  };
}

function setWorkspaceProjectEditorKey(
  workspace: WorkspaceRecord,
  projectId: string,
  editorKey: WorkspaceEditorKey,
) {
  return {
    ...workspace,
    projects: workspace.projects.map((project) =>
      project.id === projectId
        ? {
            ...project,
            ideType: editorKey,
          }
        : project,
    ),
  };
}

function reconcileWorkspaceSidebarItems(
  currentItems: WorkspaceSidebarItem[],
  nextSourceItems: WorkspaceSidebarItem[],
) {
  if (!currentItems.length) {
    return nextSourceItems;
  }

  const nextItemById = new Map(
    nextSourceItems.map((item) => [item.workspace.id, item] as const),
  );
  const reconciledItems: WorkspaceSidebarItem[] = [];

  for (const item of currentItems) {
    const nextItem = nextItemById.get(item.workspace.id);
    if (!nextItem) {
      continue;
    }

    reconciledItems.push(nextItem);
    nextItemById.delete(item.workspace.id);
  }

  for (const item of nextSourceItems) {
    if (nextItemById.has(item.workspace.id)) {
      reconciledItems.push(item);
    }
  }

  return reconciledItems;
}

function formatLastRunAt(recentRestoreTarget?: RecentRestoreTarget) {
  if (!recentRestoreTarget?.lastRestoreAt) {
    return "未运行";
  }

  const date = new Date(recentRestoreTarget.lastRestoreAt);
  if (Number.isNaN(date.getTime())) {
    return recentRestoreTarget.lastRestoreAt;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

function isSidebarRouteActive(pathname: string, href: string) {
  if (pathname === href) {
    return true;
  }

  if (href === "/settings/general" && pathname === "/settings") {
    return true;
  }

  return pathname.startsWith(`${href}/`);
}

function normalizeWorkspaceSelectionIds(workspaceIds: string[]) {
  return normalizeWorkspacePreferenceIds(workspaceIds);
}

function normalizeWorkspacePinnedIds(workspaceIds: string[]) {
  return normalizeWorkspacePreferenceIds(workspaceIds);
}

function normalizeWorkspacePreferenceIds(workspaceIds: string[]) {
  const seenIds = new Set<string>();
  const normalizedIds: string[] = [];

  for (const workspaceId of workspaceIds) {
    const trimmedWorkspaceId = workspaceId.trim();
    if (!trimmedWorkspaceId || seenIds.has(trimmedWorkspaceId)) {
      continue;
    }

    seenIds.add(trimmedWorkspaceId);
    normalizedIds.push(trimmedWorkspaceId);
  }

  return normalizedIds;
}

function normalizeDroppedWorkspacePaths(paths: string[]) {
  const seenPaths = new Set<string>();
  const normalizedPaths: string[] = [];

  for (const path of paths) {
    const trimmedPath = path.trim();
    if (!trimmedPath || seenPaths.has(trimmedPath.toLowerCase())) {
      continue;
    }

    seenPaths.add(trimmedPath.toLowerCase());
    normalizedPaths.push(trimmedPath);
  }

  return normalizedPaths;
}

function buildDroppedWorkspaceSignature(paths: string[]) {
  return [...paths]
    .map((path) => path.trim().toLowerCase())
    .filter(Boolean)
    .sort()
    .join("|");
}

function isPositionInsideElement(
  element: HTMLElement | null,
  payload: Extract<DragDropEvent, { type: "enter" | "over" | "drop" }>,
  scaleFactor: number,
) {
  if (!element) {
    return false;
  }

  const rect = element.getBoundingClientRect();
  const logicalPosition = payload.position.toLogical(scaleFactor > 0 ? scaleFactor : 1);
  const x = logicalPosition.x;
  const y = logicalPosition.y;

  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

function normalizeWorkspaceEditorKey(
  editorKey: string | null | undefined,
  editorOptions: WorkspaceEditorOption[],
): WorkspaceEditorKey {
  const normalized = editorKey?.trim();
  if (normalized && editorOptions.some((option) => option.key === normalized)) {
    return normalized;
  }

  return editorOptions[0]?.key ?? "vscode";
}

function withWorkspaceSelectedIds(
  preferences: AppPreferences,
  selectedWorkspaceIds: string[],
): AppPreferences {
  return {
    ...preferences,
    workspace: {
      ...preferences.workspace,
      selectedWorkspaceIds: normalizeWorkspaceSelectionIds(selectedWorkspaceIds),
    },
  };
}

function withWorkspacePinnedIds(
  preferences: AppPreferences,
  pinnedWorkspaceIds: string[],
): AppPreferences {
  return {
    ...preferences,
    workspace: {
      ...preferences.workspace,
      pinnedWorkspaceIds: normalizeWorkspacePinnedIds(pinnedWorkspaceIds),
    },
  };
}

function applyPinnedWorkspaceGrouping(
  items: WorkspaceSidebarItem[],
  pinnedWorkspaceIds: string[],
) {
  if (!pinnedWorkspaceIds.length) {
    return items;
  }

  const pinnedWorkspaceIdSet = new Set(pinnedWorkspaceIds);
  const pinnedItems: WorkspaceSidebarItem[] = [];
  const unpinnedItems: WorkspaceSidebarItem[] = [];

  for (const item of items) {
    if (pinnedWorkspaceIdSet.has(item.workspace.id)) {
      pinnedItems.push(item);
      continue;
    }

    unpinnedItems.push(item);
  }

  return [...pinnedItems, ...unpinnedItems];
}

function moveWorkspaceItemToIndex(
  items: WorkspaceSidebarItem[],
  workspaceId: string,
  targetIndex: number,
) {
  const currentIndex = items.findIndex((item) => item.workspace.id === workspaceId);
  if (currentIndex < 0) {
    return items;
  }

  const nextItems = [...items];
  const [targetItem] = nextItems.splice(currentIndex, 1);
  nextItems.splice(Math.max(0, Math.min(targetIndex, nextItems.length)), 0, targetItem);
  return nextItems;
}

function buildWorkspaceEditorOptions(
  capabilities?: RestoreCapabilities,
  preferences: AppPreferences = defaultAppPreferences,
): WorkspaceEditorOption[] {
  const builtInOptions: WorkspaceEditorOption[] = [
    {
      key: "vscode",
      label: "Visual Studio Code",
      available: capabilities?.vscode.available ?? false,
      message: capabilities?.vscode.message ?? "VS Code 当前不可用",
    },
    {
      key: "jetbrains",
      label: "JetBrains IDE",
      available: capabilities?.jetbrains.available ?? false,
      message: capabilities?.jetbrains.message ?? "JetBrains 当前不可用",
    },
  ];
  const customOptions = buildCustomWorkspaceEditorOptions(preferences.ide.customEditors);

  return [...builtInOptions, ...customOptions];
}

function WorkspaceEditorGlyph({
  editorKey,
  label,
  compact = false,
}: {
  editorKey: WorkspaceEditorKey;
  label?: string;
  compact?: boolean;
}) {
  const normalizedKey = editorKey.trim().toLowerCase();
  const isVSCode = normalizedKey === "vscode";
  const isJetBrains = normalizedKey === "jetbrains";
  const glyphText = resolveWorkspaceEditorGlyphText(editorKey, label);
  const toneClassName = isVSCode
    ? "bg-selected text-primary"
    : isJetBrains
      ? "bg-warning-soft text-warning"
      : "bg-surface-2 text-fg-2";

  return (
    <span
      className={cn(
        "inline-flex items-center justify-center rounded-[4px] font-semibold leading-none",
        compact ? "h-4 w-4 text-[8px]" : "h-4 w-4 text-[8px]",
        toneClassName,
      )}
    >
      {glyphText}
    </span>
  );
}

function buildCustomWorkspaceEditorOptions(
  customEditors: CustomEditorRecord[] | undefined,
): WorkspaceEditorOption[] {
  const editors = customEditors ?? [];
  if (!editors.length) {
    return [];
  }

  const seenKeys = new Set<string>();
  const options: WorkspaceEditorOption[] = [];

  for (const editor of editors) {
    const key = editor.id.trim();
    if (
      !key ||
      key === "vscode" ||
      key === "jetbrains" ||
      seenKeys.has(key)
    ) {
      continue;
    }

    const name = editor.name.trim();
    const command = editor.command.trim();
    if (!name || !command) {
      continue;
    }

    seenKeys.add(key);
    options.push({
      key,
      label: name,
      available: true,
      message: `使用自定义编辑器命令：${command}`,
    });
  }

  return options;
}

function resolveWorkspaceEditorGlyphText(editorKey: string, label?: string) {
  const normalizedKey = editorKey.trim().toLowerCase();
  if (normalizedKey === "vscode") {
    return "VS";
  }
  if (normalizedKey === "jetbrains") {
    return "JB";
  }

  const seed = label?.trim() || editorKey.trim();
  const firstAlphaNumeric = seed
    .replace(/[^a-zA-Z0-9]/g, "")
    .slice(0, 2)
    .toUpperCase();
  if (firstAlphaNumeric.length >= 2) {
    return firstAlphaNumeric;
  }

  return "ED";
}

function areStringArraysEqual(left: string[], right: string[]) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((value, index) => value === right[index]);
}

function isSameWorkspaceOrder(
  previousOrder: string[],
  nextItems: WorkspaceSidebarItem[],
) {
  if (previousOrder.length !== nextItems.length) {
    return false;
  }

  return previousOrder.every(
    (workspaceId, index) => nextItems[index]?.workspace.id === workspaceId,
  );
}

function buildWorkspaceExportFileName(now = new Date()) {
  const date = now.toISOString().slice(0, 10);
  return `bexo-workspaces-${date}.bexo-workspaces.json`;
}
