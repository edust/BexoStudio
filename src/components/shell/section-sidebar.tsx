import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CaretRightOutlined,
  CheckSquareOutlined,
  CloseOutlined,
  CodeOutlined,
  CopyOutlined,
  DownloadOutlined,
  DownOutlined,
  EditOutlined,
  FileTextOutlined,
  FolderOpenOutlined,
  HistoryOutlined,
  HolderOutlined,
  PlusOutlined,
  SearchOutlined,
  UploadOutlined,
} from "@ant-design/icons";
import { getCurrentWindow, type DragDropEvent } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Button, Checkbox, Dropdown, Empty, Input, Modal, Popconfirm, Tag, Tooltip, Typography, type MenuProps } from "antd";
import { motion, Reorder, useDragControls } from "motion/react";
import { memo, startTransition, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { toast } from "sonner";

import { copyTextToClipboard, getClipboardErrorMessage } from "@/lib/clipboard";
import { defaultAppPreferences } from "@/lib/app-preferences";
import { cn } from "@/lib/cn";
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
  removeWorkspaceRegistration,
  runWorkspaceTerminalCommands,
  updateWorkspaceDescription,
  upsertProject,
} from "@/lib/command-client";
import { sendDesktopNotification } from "@/lib/desktop-notification";
import { reorderLayoutTransition } from "@/lib/reorder-motion";
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
import { WorkspaceNotePopover } from "@/features/workspaces/workspace-note-popover";
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
  const [draggingWorkspaceId, setDraggingWorkspaceId] = useState<string | null>(null);
  const [workspaceDropPending, setWorkspaceDropPending] = useState(false);
  const [workspaceCreationPath, setWorkspaceCreationPath] = useState<string | null>(null);
  const [workspaceCreationDescription, setWorkspaceCreationDescription] = useState("");
  const [workspaceImportSourcePath, setWorkspaceImportSourcePath] = useState<string | null>(null);
  const location = useLocation();
  const selectedHomeWorkspaceId = useShellStore((state) => state.selectedHomeWorkspaceId);
  const setSelectedHomeWorkspaceId = useShellStore((state) => state.setSelectedHomeWorkspaceId);
  const themeMode = useShellStore((state) => state.themeMode);
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
  const workspaceDragStartOrderRef = useRef<string[] | null>(null);
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
      orderedWorkspaceItems.map((item) => item.workspace.id),
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
  ]);

  useEffect(() => {
    if (content.dataSource !== "workspaces" || !workspaceQuery.isSuccess) {
      return;
    }

    const validWorkspaceIds = new Set(orderedWorkspaceItems.map((item) => item.workspace.id));
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
  ]);

  useEffect(() => {
    if (content.dataSource !== "workspaces") {
      return;
    }

    if (!orderedWorkspaceItems.length) {
      setSelectedHomeWorkspaceId(null);
      return;
    }

    if (!selectedHomeWorkspaceId) {
      setSelectedHomeWorkspaceId(orderedWorkspaceItems[0].workspace.id);
      return;
    }

    const stillExists = orderedWorkspaceItems.some(
      (item) => item.workspace.id === selectedHomeWorkspaceId,
    );
    if (!stillExists) {
      setSelectedHomeWorkspaceId(orderedWorkspaceItems[0].workspace.id);
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

    return filterWorkspaceItems(orderedWorkspaceItems, query);
  }, [content.dataSource, orderedWorkspaceItems, query]);

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

  const handlePreviewWorkspaceReorder = useCallback((nextWorkspaceIds: string[]) => {
    const currentItems = latestOrderedWorkspaceItemsRef.current;
    const itemByWorkspaceId = new Map(
      currentItems.map((item) => [item.workspace.id, item] as const),
    );
    const nextItems = nextWorkspaceIds
      .map((workspaceId) => itemByWorkspaceId.get(workspaceId))
      .filter((item): item is WorkspaceSidebarItem => Boolean(item));

    if (nextItems.length !== currentItems.length) {
      return;
    }

    const normalizedItems = applyPinnedWorkspaceGrouping(nextItems, pinnedWorkspaceIdsRef.current);
    latestOrderedWorkspaceItemsRef.current = normalizedItems;
    setOrderedWorkspaceItems(normalizedItems);
  }, []);

  const handleWorkspaceDragStart = useCallback(
    (workspaceId: string) => {
      if (reorderWorkspacesMutation.isPending) {
        return;
      }

      workspaceDragStartOrderRef.current = latestOrderedWorkspaceItemsRef.current.map(
        (item) => item.workspace.id,
      );
      setDraggingWorkspaceId(workspaceId);
    },
    [reorderWorkspacesMutation.isPending],
  );

  const handleWorkspaceDragEnd = useCallback(async () => {
    const previousOrder = workspaceDragStartOrderRef.current;
    const nextItems = latestOrderedWorkspaceItemsRef.current;
    workspaceDragStartOrderRef.current = null;
    setDraggingWorkspaceId(null);

    if (!previousOrder || reorderWorkspacesMutation.isPending) {
      return;
    }

    if (isSameWorkspaceOrder(previousOrder, nextItems)) {
      return;
    }

    try {
      await reorderWorkspacesMutation.mutateAsync({ nextItems });
    } catch {
      return;
    }
  }, [reorderWorkspacesMutation]);

  async function handleWorkspacePinnedChange(workspace: WorkspaceRecord, nextPinned: boolean) {
    if (reorderWorkspacesMutation.isPending || updatePreferencesMutation.isPending) {
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
      selectedWorkspaceIdsRef.current.includes(item.workspace.id),
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
        <div className="space-y-1 text-[12px] leading-5 text-[#475467]">
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-[#475467]">
            即将按左侧当前顺序运行 {selectedWorkspaces.length} 个工作区，每个工作区都会各开一个 Windows Terminal 窗口。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-[#475467]">
            窗口内命令会继续按顺序执行，并保持 10 秒错峰。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-[#667085]">
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
        <div className="space-y-2 text-[12px] leading-5 text-[#475467]">
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-[#475467]">
            将导出当前全部工作区、项目启动配置、终端命令和置顶状态，用于在本机或另一台电脑恢复列表。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-[#b54708]">
            文件会包含绝对目录路径、终端命令和参数，请勿上传到公开仓库或发送给不可信的人。
          </Typography.Paragraph>
          <Typography.Paragraph className="!mb-0 !text-[11px] !leading-5 !text-[#667085]">
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
      key: "create",
      icon: <PlusOutlined />,
      label: "新建工作区",
      disabled: !desktopRuntimeAvailable || registerWorkspaceMutation.isPending,
    },
    {
      key: "select-all",
      icon: <CheckSquareOutlined />,
      label: "全选工作区",
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
        !selectedWorkspaceIds.length ||
        runSelectedWorkspacesMutation.isPending,
    },
    {
      type: "divider",
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
    !query.trim();
  const workspaceReorderTooltip = query.trim()
    ? "搜索过滤时暂不支持拖拽排序"
    : "拖动调整工作区顺序";
  const workspaceActionsBusy =
    registerWorkspaceMutation.isPending ||
    exportWorkspaceListMutation.isPending ||
    runSelectedWorkspacesMutation.isPending ||
    workspaceDropPending;
  const workspaceCreationDescriptionCount = countUnicodeCharacters(workspaceCreationDescription);
  const workspaceCreationDescriptionError = validateWorkspaceDescription(
    workspaceCreationDescription,
  );

    return (
      <aside
        className="bexo-shell-surface relative flex h-full min-h-0 flex-col overflow-hidden rounded-[16px] transition-[box-shadow,border-color,background-color] duration-150"
        ref={content.dataSource === "workspaces" ? workspaceDropContainerRef : undefined}
      >
      <div
        className="border-b border-[#e6edf5] px-4 py-4 pb-1"
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <Typography.Text className="block text-[11px] font-semibold uppercase tracking-[0.24em] text-[#667085]">
              {content.eyebrow}
            </Typography.Text>
            {content.title ? (
              <Typography.Title className="!mb-1.5 !mt-2 !text-[18px] !font-semibold !text-[#1f2937]" level={4}>
                {content.title}
              </Typography.Title>
            ) : null}
          </div>
          {content.dataSource === "workspaces" ? (
            <Dropdown
              menu={{
                items: workspaceActionItems,
                onClick: ({ key }) => void handleWorkspaceActionClick(key),
              }}
              trigger={["click"]}
            >
              <Button
                className="!px-2 !text-[11px]"
                disabled={!desktopRuntimeAvailable}
                loading={workspaceActionsBusy}
                size="small"
                type="text"
              >
                <span className="inline-flex items-center gap-1">
                  <PlusOutlined />
                  操作
                  <DownOutlined className="text-[10px]" />
                </span>
              </Button>
            </Dropdown>
          ) : null}
        </div>
        {content.description ? (
          <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-[#667085]">
            {content.description}
          </Typography.Paragraph>
        ) : null}
        {content.searchPlaceholder ? (
          <Input
            allowClear
            className={content.description ? "mt-3" : "mt-2"}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={content.searchPlaceholder}
            prefix={<SearchOutlined className="text-[#98a2b3]" />}
            value={query}
          />
        ) : null}
        {content.dataSource === "workspaces" && desktopRuntimeAvailable ? (
          <Typography.Text className="mt-2 block min-h-[40px] text-[11px] leading-5 text-[#98a2b3]">
            可将文件夹拖入此侧栏，直接创建工作区
          </Typography.Text>
        ) : null}
      </div>

        <motion.div
        className={cn(
          "min-h-0 flex-1 overflow-y-auto",
          content.dataSource === "workspaces" && "flex flex-col",
        )}
        layoutScroll={content.dataSource === "workspaces"}
        style={{ contain: "strict" }}
      >
      {content.dataSource === "workspaces" && filteredWorkspaceItems.length ? (
        <>
          <div className="min-h-0 px-2 pb-[5px] pt-2">
            {workspaceReorderEnabled ? (
              <Reorder.Group
                as="div"
                axis="y"
                className="flex flex-col gap-[5px]"
                onReorder={handlePreviewWorkspaceReorder}
                values={orderedWorkspaceItems.map((item) => item.workspace.id)}
              >
                {orderedWorkspaceItems.map((item) => (
                  <WorkspaceSidebarCard
                    desktopRuntimeAvailable={desktopRuntimeAvailable}
                    dragging={draggingWorkspaceId === item.workspace.id}
                    key={item.key}
                    editorKey={resolveWorkspaceEditorKey(item.workspace, workspaceEditorOptions)}
                    editorOptions={workspaceEditorOptions}
                    onCopyPath={() => void handleCopyWorkspacePath(item.workspace)}
                    onSaveNote={(description) =>
                      updateWorkspaceDescriptionMutation.mutateAsync({
                        workspaceId: item.workspace.id,
                        description,
                      }).then(() => undefined)
                    }
                    onOpenCodexHistory={() => void handleOpenCodexHistory(item.workspace)}
                    onChangeEditor={(editorKey) =>
                      void handleWorkspaceEditorChange(item.workspace, editorKey)
                    }
                    onDragEnd={() => void handleWorkspaceDragEnd()}
                    onDragStart={() => handleWorkspaceDragStart(item.workspace.id)}
                    onOpenDirectory={() => void handleOpenWorkspaceDirectory(item.workspace)}
                    onOpenInEditor={(editorKey) =>
                      void handleOpenWorkspaceInEditor(item.workspace, editorKey)
                    }
                    onOpenTerminal={() => void handleOpenWorkspaceTerminal(item.workspace)}
                    onTogglePinned={(pinned) =>
                      void handleWorkspacePinnedChange(item.workspace, pinned)
                    }
                    onRemove={() => void handleRemoveWorkspace(item.workspace)}
                    onSelect={() =>
                      startTransition(() => setSelectedHomeWorkspaceId(item.workspace.id))
                    }
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
                    reorderEnabled
                    pinned={persistedPinnedWorkspaceIds.includes(item.workspace.id)}
                    themeMode={themeMode}
                    selected={selectedHomeWorkspaceId === item.workspace.id}
                    selectionChecked={selectedWorkspaceIds.includes(item.workspace.id)}
                    workspaceItem={item}
                  />
                ))}
              </Reorder.Group>
            ) : (
              <div className="space-y-[5px]">
                {filteredWorkspaceItems.map((item) => (
                  <WorkspaceSidebarCard
                    desktopRuntimeAvailable={desktopRuntimeAvailable}
                    dragging={false}
                    key={item.key}
                    editorKey={resolveWorkspaceEditorKey(item.workspace, workspaceEditorOptions)}
                    editorOptions={workspaceEditorOptions}
                    onCopyPath={() => void handleCopyWorkspacePath(item.workspace)}
                    onSaveNote={(description) =>
                      updateWorkspaceDescriptionMutation.mutateAsync({
                        workspaceId: item.workspace.id,
                        description,
                      }).then(() => undefined)
                    }
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
                    onSelect={() =>
                      startTransition(() => setSelectedHomeWorkspaceId(item.workspace.id))
                    }
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
                    reorderEnabled={false}
                    reorderTooltip={workspaceReorderTooltip}
                    pinned={persistedPinnedWorkspaceIds.includes(item.workspace.id)}
                    themeMode={themeMode}
                    selected={selectedHomeWorkspaceId === item.workspace.id}
                    selectionChecked={selectedWorkspaceIds.includes(item.workspace.id)}
                    workspaceItem={item}
                  />
                ))}
              </div>
            )}
          </div>
          <div className="flex-1" />
        </>
      ) : filteredItems.length ? (
        <div className="min-h-0 px-2 pb-[5px] pt-2">
          <div className="space-y-[7px]">
            {filteredItems.map((item) => {
              const itemActive = item.href ? isSidebarRouteActive(location.pathname, item.href) : false;
              const inner = (
                <div
                  className={cn(
                    "rounded-[12px] border px-3 py-2.5 transition-colors",
                    itemActive
                      ? "border-[#8fd4ec] bg-[#f4fbfe]"
                      : themeMode === "dark"
                        ? "border-transparent bg-transparent hover:border-[#3c3c3c] hover:bg-[#2a2d2e]"
                        : "border-transparent bg-transparent hover:border-[#d9e2ec] hover:bg-[#f8fafc]",
                  )}
                >
                  <div className="flex items-center gap-2">
                    <Typography.Text className="text-[13px] font-medium text-[#1f2937]">{item.label}</Typography.Text>
                    {item.badge ? (
                      <Tag bordered={false} className="m-0 rounded-full bg-[#eef6fb] px-2 py-0 text-[10px] font-semibold uppercase tracking-[0.12em] text-[#1283ab]">
                        {item.badge}
                      </Tag>
                    ) : null}
                  </div>
                  <Typography.Paragraph className="!mb-0 !mt-1 !text-[12px] !leading-5 !text-[#7b8794]">
                    {item.description}
                  </Typography.Paragraph>
                </div>
              );

              if (item.href) {
                return (
                  <Link className="block w-full" key={item.key} to={item.href}>
                    {inner}
                  </Link>
                );
              }

              return <div key={item.key}>{inner}</div>;
            })}
          </div>
        </div>
      ) : content.dataSource === "workspaces" ? (
        <div className="px-2 pb-[5px] pt-2">
          <div className="flex min-h-[120px] flex-1 items-center justify-center rounded-[12px] border border-dashed border-[#e6edf5] bg-[#fbfcfe]">
            <Empty
              description={
                !desktopRuntimeAvailable
                  ? "请在桌面应用中管理工作区"
                  : workspaceQuery.isLoading
                  ? "正在加载工作区..."
                  : query.trim()
                    ? "没有匹配的工作区"
                    : "暂无工作区"
              }
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          </div>
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
            <Typography.Text className="block text-[11px] font-semibold uppercase tracking-[0.12em] text-[#98a2b3]">
              工作区目录
            </Typography.Text>
            <Typography.Text
              className="mt-1 block truncate text-[12px] text-[#475467]"
              title={workspaceCreationPath ?? ""}
            >
              {workspaceCreationPath}
            </Typography.Text>
          </div>
          <div>
            <Typography.Text className="block text-[12px] font-semibold text-[#1f2937]">
              项目备注（可选）
            </Typography.Text>
            <Typography.Text className="mt-0.5 block text-[11px] leading-4 text-[#667085]">
              用一句话记住这个目录的用途，之后也可以从左侧卡片继续编辑。
            </Typography.Text>
          </div>
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
              workspaceCreationDescriptionError ? "text-[#cf5a4a]" : "text-[#98a2b3]",
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

      <WorkspaceTransferDialog
        onClose={() => setWorkspaceImportSourcePath(null)}
        onImported={handleWorkspaceImportCompleted}
        open={Boolean(workspaceImportSourcePath)}
        sourcePath={workspaceImportSourcePath}
      />

      {content.footerTitle || content.footerDescription ? (
        <div className="mt-[5px] border-t border-[#e6edf5] px-4 py-3">
          {content.footerTitle ? (
            <Typography.Text className="block text-[10px] font-semibold uppercase tracking-[0.18em] text-[#98a2b3]">
              {content.footerTitle}
            </Typography.Text>
          ) : null}
          {content.footerDescription ? (
            <Typography.Paragraph className="!mb-0 !mt-1 !text-[11px] !leading-5 !text-[#98a2b3]">
              {content.footerDescription}
            </Typography.Paragraph>
          ) : null}
        </div>
      ) : null}
    </aside>
  );
}

type WorkspaceSidebarItem = SidebarItem & {
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
  dragging: boolean;
  editorKey: WorkspaceEditorKey;
  editorOptions: WorkspaceEditorOption[];
  onCopyPath: () => void;
  onSaveNote: (description: string) => Promise<void>;
  onChangeEditor: (editorKey: WorkspaceEditorKey) => void;
  onDragEnd?: () => void;
  onDragStart?: () => void;
  onOpenCodexHistory: () => void;
  onOpenDirectory: () => void;
  onOpenInEditor: (editorKey: WorkspaceEditorKey) => void;
  onOpenTerminal: () => void;
  onTogglePinned: (pinned: boolean) => void;
  onRemove: () => void;
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
  reorderEnabled: boolean;
  reorderTooltip?: string;
  pinned: boolean;
  themeMode: "light" | "dark";
  selected: boolean;
  selectionChecked: boolean;
  workspaceItem: WorkspaceSidebarItem;
};

const WorkspaceSidebarCard = memo(function WorkspaceSidebarCard({
  desktopRuntimeAvailable,
  dragging,
  editorKey,
  editorOptions,
  onCopyPath,
  onSaveNote,
  onChangeEditor,
  onDragEnd,
  onDragStart,
  onOpenCodexHistory,
  onOpenDirectory,
  onOpenInEditor,
  onOpenTerminal,
  onTogglePinned,
  onRemove,
  onSelect,
  onSelectionChange,
  openInEditorLoading,
  openCodexHistoryDisabled,
  openCodexHistoryLoading,
  openTerminalDisabled,
  openTerminalLoading,
  editorSaving,
  noteSaving,
  removePending,
  reorderEnabled,
  reorderTooltip = "拖动调整工作区顺序",
  pinned,
  themeMode,
  selected,
  selectionChecked,
  workspaceItem,
}: WorkspaceSidebarCardProps) {
  const dragControls = useDragControls();
  const [noteEditorOpen, setNoteEditorOpen] = useState(false);
  const contextMenu = {
    items: [
      {
        key: "edit-note",
        icon: <EditOutlined />,
        label: workspaceItem.workspace.description?.trim() ? "编辑备注" : "添加备注",
      },
      {
        key: pinned ? "unpin" : "pin",
        label: pinned ? "取消置顶" : "置顶",
      },
    ],
    onClick: ({ key, domEvent }) => {
      domEvent.stopPropagation();
      if (key === "edit-note") {
        setNoteEditorOpen(true);
        return;
      }
      onTogglePinned(key === "pin");
    },
  } satisfies MenuProps;
  const preferredEditorOption =
    editorOptions.find((option) => option.key === editorKey) ?? editorOptions[0];
  const workspacePath = resolveWorkspacePath(workspaceItem.workspace);
  const openEditorTooltip = !desktopRuntimeAvailable
    ? "请在桌面应用中打开工作区编辑器"
    : !workspacePath
      ? "当前工作区没有可打开的目录路径"
      : preferredEditorOption?.available
        ? `使用 ${preferredEditorOption.label} 打开工作区`
        : preferredEditorOption?.message ?? "当前默认编辑器不可用";
  const openEditorDisabled =
    !desktopRuntimeAvailable ||
    !workspacePath ||
    !preferredEditorOption?.available;
  const editorMenuItems = editorOptions.map((option) => ({
    key: option.key,
    disabled: !option.available || editorSaving,
    label: (
      <div className="flex min-w-[168px] items-center gap-2 py-0.5">
        <WorkspaceEditorGlyph editorKey={option.key} label={option.label} />
        <div className="min-w-0 text-[12px] font-medium text-[#1f2937]">{option.label}</div>
      </div>
    ),
  }));
  const content = (
    <div className="flex items-start justify-between gap-2">
      <div className="flex min-w-0 flex-1 items-start gap-2">
        <Tooltip placement="bottom" title={reorderTooltip}>
          <span
            aria-label={`拖动排序 ${workspaceItem.label}`}
            className={cn(
              "mt-0.5 flex h-6 w-6 items-center justify-center rounded-[6px] border border-[#d8e1eb] bg-white text-[#667085]",
              reorderEnabled ? "cursor-grab active:cursor-grabbing" : "cursor-not-allowed opacity-60",
            )}
            onClick={(event) => event.stopPropagation()}
            onPointerDown={(event) => {
              event.stopPropagation();
              if (!reorderEnabled) {
                return;
              }
              dragControls.start(event);
            }}
            role="button"
            tabIndex={-1}
          >
            <HolderOutlined />
          </span>
        </Tooltip>
        <Checkbox
          checked={selectionChecked}
          className="mt-0.5"
          onClick={(event) => event.stopPropagation()}
          onChange={(event) => onSelectionChange(event.target.checked)}
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <Typography.Text className="text-[13px] font-medium text-[#1f2937]">
              {workspaceItem.label}
            </Typography.Text>
            {workspaceItem.badge ? (
              <Tag
                bordered={false}
                className="m-0 rounded-full bg-[#eef6fb] px-2 py-0 text-[10px] font-semibold uppercase tracking-[0.12em] text-[#1283ab]"
              >
                {workspaceItem.badge}
              </Tag>
            ) : null}
          </div>
          <Typography.Paragraph
            className="!mb-0 !mt-1 !text-[12px] !leading-5 !text-[#7b8794]"
            ellipsis={{ rows: 2, tooltip: workspaceItem.description }}
          >
            {workspaceItem.description}
          </Typography.Paragraph>
          <Typography.Text
            className="mt-0.5 block !text-[9px] !leading-[13px] text-[#98a2b3]"
            style={{ fontSize: 9, lineHeight: "13px" }}
          >
            最后运行：{formatLastRunAt(workspaceItem.recentRestoreTarget)}
          </Typography.Text>
          {workspaceItem.note ? (
            <div
              className="mt-1 flex min-w-0 items-center gap-1 text-[11px] leading-4 text-[#667085]"
              title={workspaceItem.note}
            >
              <FileTextOutlined className="shrink-0 text-[10px] text-[#1283ab]" />
              <span className="min-w-0 truncate">{workspaceItem.note}</span>
            </div>
          ) : null}
          <div className="mt-1 flex items-center gap-1">
            <Tooltip placement="bottom" title={workspaceItem.note ? "编辑项目备注" : "添加项目备注"}>
              <WorkspaceNotePopover
                loading={noteSaving}
                onOpenChange={setNoteEditorOpen}
                onSave={onSaveNote}
                open={noteEditorOpen}
                trigger={
                  <Button
                    aria-label={workspaceItem.note ? "编辑项目备注" : "添加项目备注"}
                    className={cn(
                      "!h-6 !w-6 !min-w-6 !p-0",
                      workspaceItem.note ? "!text-[#1283ab]" : "",
                    )}
                    icon={workspaceItem.note ? <FileTextOutlined /> : <EditOutlined />}
                    size="small"
                    type="text"
                  />
                }
                value={workspaceItem.workspace.description}
              />
            </Tooltip>
            <Tooltip placement="bottom" title="查看 Codex 会话历史">
              <span onClick={(event) => event.stopPropagation()}>
                <Button
                  className="!h-6 !w-6 !min-w-6 !p-0"
                  disabled={!desktopRuntimeAvailable || openCodexHistoryDisabled || !workspacePath}
                  icon={<HistoryOutlined />}
                  loading={openCodexHistoryLoading}
                  onClick={(event) => {
                    event.stopPropagation();
                    onOpenCodexHistory();
                  }}
                  size="small"
                  type="text"
                />
              </span>
            </Tooltip>
            <Tooltip placement="bottom" title="复制工作区绝对路径">
              <span onClick={(event) => event.stopPropagation()}>
                <Button
                  className="!h-6 !w-6 !min-w-6 !p-0"
                  disabled={!workspacePath}
                  icon={<CopyOutlined />}
                  onClick={(event) => {
                    event.stopPropagation();
                    onCopyPath();
                  }}
                  size="small"
                  type="text"
                />
              </span>
            </Tooltip>
            <Tooltip placement="bottom" title="在该工作区目录打开终端">
              <span onClick={(event) => event.stopPropagation()}>
                <Button
                  className="!h-6 !w-6 !min-w-6 !p-0"
                  disabled={
                    !desktopRuntimeAvailable ||
                    openTerminalDisabled ||
                    !workspacePath
                  }
                  icon={<CodeOutlined />}
                  loading={openTerminalLoading}
                  onClick={(event) => {
                    event.stopPropagation();
                    onOpenTerminal();
                  }}
                  size="small"
                  type="text"
                />
              </span>
            </Tooltip>
            <Tooltip placement="bottom" title="在资源管理器中打开工作区目录">
              <span onClick={(event) => event.stopPropagation()}>
                <Button
                  className="!h-6 !w-6 !min-w-6 !p-0"
                  disabled={!desktopRuntimeAvailable || !workspacePath}
                  icon={<FolderOpenOutlined />}
                  onClick={(event) => {
                    event.stopPropagation();
                    onOpenDirectory();
                  }}
                  size="small"
                  type="text"
                />
              </span>
            </Tooltip>
            <div
              className="inline-flex items-center overflow-hidden rounded-[6px] border border-[#d8e1eb] bg-white"
              onClick={(event) => event.stopPropagation()}
            >
              <Tooltip placement="bottom" title={openEditorTooltip}>
                <span>
                  <Button
                    className="!h-6 !min-w-0 !rounded-none !border-0 !px-1.5"
                    disabled={openEditorDisabled}
                    icon={
                      <WorkspaceEditorGlyph
                        compact
                        editorKey={preferredEditorOption?.key ?? editorKey}
                        label={preferredEditorOption?.label}
                      />
                    }
                    loading={openInEditorLoading}
                    onClick={(event) => {
                      event.stopPropagation();
                      onOpenInEditor(preferredEditorOption?.key ?? editorKey);
                    }}
                    size="small"
                    type="text"
                  />
                </span>
              </Tooltip>
              <Dropdown
                menu={{
                  items: editorMenuItems,
                  onClick: ({ key, domEvent }) => {
                    domEvent.stopPropagation();
                    onChangeEditor(key as WorkspaceEditorKey);
                  },
                }}
                trigger={["click"]}
              >
                <Button
                  className="!h-6 !w-5 !min-w-5 !rounded-none !border-0 !border-l !border-[#d8e1eb] !p-0"
                  disabled={!editorOptions.length}
                  icon={<DownOutlined className="text-[10px]" />}
                  onClick={(event) => event.stopPropagation()}
                  size="small"
                  type="text"
                />
              </Dropdown>
            </div>
          </div>
        </div>
      </div>
      <div className="flex min-h-[72px] flex-col items-end justify-start gap-2 self-stretch">
        <Popconfirm
          description="这不会删除磁盘上的文件夹。"
          okButtonProps={{ danger: true, loading: removePending }}
          okText="移除"
          onConfirm={onRemove}
          title="是否从 Bexo Studio 中移除此工作区？"
        >
          <Button
            className="!h-7 !w-7 !min-w-7 !p-0"
            disabled={removePending}
            icon={<CloseOutlined />}
            onClick={(event) => event.stopPropagation()}
            size="small"
            type="text"
          />
        </Popconfirm>
      </div>
    </div>
  );

  if (!reorderEnabled) {
    return (
      <Dropdown menu={contextMenu} trigger={["contextMenu"]}>
        <div
          className={cn(
            "relative cursor-pointer rounded-[12px] border px-3 py-2.5 transition-colors",
            selected
              ? "border-[#8fd4ec] bg-[#f4fbfe]"
              : themeMode === "dark"
                ? "border-transparent hover:border-[#3c3c3c] hover:bg-[#2a2d2e]"
                : "border-transparent hover:border-[#d9e2ec] hover:bg-[#f8fafc]",
          )}
          onClick={onSelect}
          onContextMenu={(event) => {
            onSelect();
            event.preventDefault();
          }}
        >
          {content}
        </div>
      </Dropdown>
    );
  }

  return (
    <Dropdown menu={contextMenu} trigger={["contextMenu"]}>
      <Reorder.Item
        as="div"
        className={cn(
          "relative cursor-pointer rounded-[12px] border px-3 py-2.5 transition-[border-color,background-color,opacity] duration-150",
          selected
            ? "border-[#8fd4ec] bg-[#f4fbfe]"
            : themeMode === "dark"
              ? "border-transparent hover:border-[#3c3c3c] hover:bg-[#2a2d2e]"
              : "border-transparent hover:border-[#d9e2ec] hover:bg-[#f8fafc]",
          dragging && "border-[#1697c5] bg-[#f0fbff] opacity-95",
        )}
        dragControls={dragControls}
        dragListener={false}
        layout="position"
        onClick={onSelect}
        onContextMenu={(event) => {
          onSelect();
          event.preventDefault();
        }}
        onDragEnd={onDragEnd}
        onDragStart={onDragStart}
        transition={reorderLayoutTransition}
        value={workspaceItem.workspace.id}
        whileDrag={{
          scale: 1.015,
          zIndex: 3,
          boxShadow: "0 22px 44px -28px rgba(22,151,197,0.45)",
        }}
      >
        {content}
      </Reorder.Item>
    </Dropdown>
  );
}, (previous, next) =>
  previous.desktopRuntimeAvailable === next.desktopRuntimeAvailable &&
  previous.dragging === next.dragging &&
  previous.editorKey === next.editorKey &&
  previous.openInEditorLoading === next.openInEditorLoading &&
  previous.openCodexHistoryDisabled === next.openCodexHistoryDisabled &&
  previous.openCodexHistoryLoading === next.openCodexHistoryLoading &&
  previous.openTerminalDisabled === next.openTerminalDisabled &&
  previous.openTerminalLoading === next.openTerminalLoading &&
  previous.editorSaving === next.editorSaving &&
  previous.noteSaving === next.noteSaving &&
  previous.removePending === next.removePending &&
  previous.reorderEnabled === next.reorderEnabled &&
  previous.reorderTooltip === next.reorderTooltip &&
  previous.pinned === next.pinned &&
  previous.themeMode === next.themeMode &&
  previous.selected === next.selected &&
  previous.selectionChecked === next.selectionChecked &&
  previous.workspaceItem.key === next.workspaceItem.key &&
  previous.workspaceItem.label === next.workspaceItem.label &&
  previous.workspaceItem.description === next.workspaceItem.description &&
  previous.workspaceItem.note === next.workspaceItem.note &&
  previous.workspaceItem.badge === next.workspaceItem.badge &&
  previous.workspaceItem.workspace.id === next.workspaceItem.workspace.id &&
  previous.workspaceItem.recentRestoreTarget?.lastRestoreAt ===
    next.workspaceItem.recentRestoreTarget?.lastRestoreAt
);

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
    badge: workspace.isDefault ? "default" : undefined,
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
    ? "bg-[#e8f1ff] text-[#2563eb]"
    : isJetBrains
      ? "bg-[#fff2e8] text-[#d46b08]"
      : "bg-[#edf2f7] text-[#475467]";

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
