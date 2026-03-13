import { useQuery } from "@tanstack/react-query";
import {
  CaretRightOutlined,
  ConsoleSqlOutlined,
  CopyOutlined,
  EyeOutlined,
  FileOutlined,
  FolderOpenOutlined,
  FolderOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import {
  Alert,
  Button,
  Dropdown,
  Empty,
  Spin,
  Tag,
  Tooltip,
  Typography,
  type MenuProps,
} from "antd";
import { watch, type UnwatchFn as FsUnwatchFn } from "@tauri-apps/plugin-fs";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { startDrag } from "@crabnebula/tauri-plugin-drag";
import {
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type UIEvent as ReactUIEvent,
  memo,
  startTransition,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";

import { copyTextToClipboard, getClipboardErrorMessage } from "@/lib/clipboard";
import { cn } from "@/lib/cn";
import {
  allowWorkspaceResourceScope,
  getErrorSummary,
  getWorkspaceResourceGitStatuses,
  listWorkspaceResourceChildren,
  openWorkspaceTerminalAtPath,
} from "@/lib/command-client";
import {
  clamp,
  isSamePathList,
  normalizeChildren,
  remapPathList,
  remapSinglePath,
  renameCachedPaths,
  sortPathsByVisibleOrder,
  usePersistentBoolean,
  usePersistentNumber,
} from "@/features/resource-browser/resource-browser-state";
import {
  buildRangeSelection,
  buildStatusMaps,
  DEFAULT_PANE_WIDTH,
  flattenVisibleRows,
  getStatusToneClass,
  isSameOrChildPath,
  MAX_PANE_WIDTH,
  MIN_PANE_WIDTH,
  normalizePath,
  prunePathsByLoadedParents,
  resolveParentPath,
  resolveRootLabel,
  statusLabel,
  type ResourceChildrenCache,
  type ResourceTreeRow,
} from "@/features/resource-browser/resource-browser-utils";
import type {
  WorkspaceResourceEntry,
  WorkspaceResourceGitStatus,
} from "@/types/backend";

type ResourceBrowserPaneProps = {
  workspaceId: string;
  workspaceName: string;
  workspacePath: string;
};

const PANE_WIDTH_KEY = "bexo.resourceBrowser.width";
const PANE_COLLAPSED_KEY = "bexo.resourceBrowser.collapsed";
const CHANGED_ONLY_KEY = "bexo.resourceBrowser.changedOnly";
const WATCH_REFRESH_INTERVAL_MS = 3000;
const NATIVE_WATCH_DELAY_MS = 280;
const WATCH_REFRESH_MIN_GAP_MS = 900;
const WATCH_MAX_ACTIVE_DIRS = 64;
const GIT_HYDRATE_DELAY_MS = 180;
const WATCH_HYDRATE_DELAY_MS = 1200;
const ENABLE_RESOURCE_WATCH = true;
const REFRESH_BATCH_CONCURRENCY = 4;
const TREE_ROW_HEIGHT = 34;
const TREE_OVERSCAN_ROWS = 10;
const TREE_VIRTUALIZATION_THRESHOLD = 240;
const WATCH_IGNORED_DIRECTORY_NAMES = new Set([
  ".git",
  "node_modules",
  ".pnpm-store",
  ".yarn",
  ".next",
  "dist",
  "build",
  "target",
  ".turbo",
  ".cache",
]);
const DRAG_PREVIEW_IMAGE =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGklEQVR42mMQm370PyWYYdSAUQNGDRguBgAAYdxxH+pCSSMAAAAASUVORK5CYII=";

type WatchStrategy = "native" | "polling" | "disabled";

export function ResourceBrowserPane({
  workspaceId,
  workspaceName,
  workspacePath,
}: ResourceBrowserPaneProps) {
  const [rootPath, setRootPath] = useState(() => normalizePath(workspacePath));
  const [childrenByPath, setChildrenByPath] = useState<ResourceChildrenCache>({});
  const [expandedPaths, setExpandedPaths] = useState<string[]>([]);
  const [selectedPaths, setSelectedPaths] = useState<string[]>([]);
  const [anchorPath, setAnchorPath] = useState<string | null>(null);
  const [activePath, setActivePath] = useState<string | null>(null);
  const [treeError, setTreeError] = useState<string | null>(null);
  const [loadingPaths, setLoadingPaths] = useState<string[]>([]);
  const [isCollapsed, setIsCollapsed] = usePersistentBoolean(PANE_COLLAPSED_KEY, false);
  const [showChangedOnly, setShowChangedOnly] = usePersistentBoolean(CHANGED_ONLY_KEY, false);
  const [watchStrategy, setWatchStrategy] = useState<WatchStrategy>(
    ENABLE_RESOURCE_WATCH ? "native" : "disabled",
  );
  const [watchRootPath, setWatchRootPath] = useState<string | null>(null);
  const [gitHydrated, setGitHydrated] = useState(false);
  const [watchHydrated, setWatchHydrated] = useState(false);
  const [treeViewportHeight, setTreeViewportHeight] = useState(0);
  const [treeScrollTop, setTreeScrollTop] = useState(0);
  const [paneWidth, setPaneWidth] = usePersistentNumber(
    PANE_WIDTH_KEY,
    DEFAULT_PANE_WIDTH,
  );

  const browserContainerRef = useRef<HTMLDivElement | null>(null);
  const treeViewportRef = useRef<HTMLDivElement | null>(null);
  const childrenByPathRef = useRef<ResourceChildrenCache>({});
  const rootPathRef = useRef(rootPath);
  const expandedPathsRef = useRef<string[]>([]);
  const selectedPathsRef = useRef<string[]>([]);
  const anchorPathRef = useRef<string | null>(null);
  const activePathRef = useRef<string | null>(null);
  const workspaceSessionRef = useRef(0);
  const refreshInFlightRef = useRef(false);
  const refreshQueuedRef = useRef(false);
  const queuedRefreshPathsRef = useRef<Set<string>>(new Set());
  const resizeStateRef = useRef<{
    pointerId: number;
    startX: number;
    startWidth: number;
  } | null>(null);
  const watchRefreshTimerRef = useRef<number | null>(null);
  const watchEventPathsRef = useRef<Set<string>>(new Set());
  const lastWatchRefreshAtRef = useRef(0);
  const dragInFlightRef = useRef(false);
  const treeScrollRafRef = useRef<number | null>(null);

  useEffect(() => {
    childrenByPathRef.current = childrenByPath;
  }, [childrenByPath]);
  useEffect(() => {
    rootPathRef.current = rootPath;
  }, [rootPath]);
  useEffect(() => {
    expandedPathsRef.current = expandedPaths;
  }, [expandedPaths]);
  useEffect(() => {
    selectedPathsRef.current = selectedPaths;
  }, [selectedPaths]);
  useEffect(() => {
    anchorPathRef.current = anchorPath;
  }, [anchorPath]);
  useEffect(() => {
    activePathRef.current = activePath;
  }, [activePath]);
  useEffect(() => {
    const viewportElement = treeViewportRef.current;
    if (!viewportElement) {
      return;
    }

    const syncViewportHeight = () => {
      setTreeViewportHeight(viewportElement.clientHeight);
    };
    syncViewportHeight();

    if (typeof ResizeObserver === "undefined") {
      const handleWindowResize = () => syncViewportHeight();
      window.addEventListener("resize", handleWindowResize);
      return () => window.removeEventListener("resize", handleWindowResize);
    }

    const observer = new ResizeObserver(() => {
      syncViewportHeight();
    });
    observer.observe(viewportElement);
    return () => observer.disconnect();
  }, [isCollapsed, rootPath]);
  useEffect(
    () => () => {
      if (treeScrollRafRef.current !== null) {
        window.cancelAnimationFrame(treeScrollRafRef.current);
        treeScrollRafRef.current = null;
      }
    },
    [],
  );

  const rootLayerHydrated = useMemo(
    () => Boolean(rootPath) && Object.prototype.hasOwnProperty.call(childrenByPath, rootPath),
    [childrenByPath, rootPath],
  );

  const isRootLayerLoading = Boolean(rootPath) && !rootLayerHydrated;

  const gitStatusQuery = useQuery({
    queryKey: ["workspaceResourceGitStatuses", workspaceId],
    queryFn: () => getWorkspaceResourceGitStatuses(workspaceId),
    enabled: Boolean(workspaceId) && rootLayerHydrated && gitHydrated,
    retry: false,
    staleTime: 20_000,
    refetchOnWindowFocus: false,
  });

  useEffect(() => {
    workspaceSessionRef.current += 1;
    refreshInFlightRef.current = false;
    refreshQueuedRef.current = false;
    queuedRefreshPathsRef.current.clear();
    watchEventPathsRef.current.clear();
    lastWatchRefreshAtRef.current = 0;
    if (watchRefreshTimerRef.current !== null) {
      window.clearTimeout(watchRefreshTimerRef.current);
      watchRefreshTimerRef.current = null;
    }

    const normalizedRootPath = normalizePath(workspacePath);
    setRootPath(normalizedRootPath);
    setExpandedPaths(normalizedRootPath ? [normalizedRootPath] : []);
    setSelectedPaths(normalizedRootPath ? [normalizedRootPath] : []);
    setAnchorPath(normalizedRootPath || null);
    setActivePath(normalizedRootPath || null);
    setLoadingPaths([]);
    setTreeError(null);
    setWatchRootPath(null);
    setWatchStrategy(ENABLE_RESOURCE_WATCH ? "native" : "disabled");
    setGitHydrated(false);
    setWatchHydrated(false);
    setTreeScrollTop(0);
    if (treeViewportRef.current) {
      treeViewportRef.current.scrollTop = 0;
    }
  }, [workspaceId, workspacePath]);

  useEffect(() => {
    if (!workspaceId || !rootLayerHydrated) {
      setGitHydrated(false);
      setWatchHydrated(false);
      return;
    }

    let disposed = false;
    const gitTimer = window.setTimeout(() => {
      if (!disposed) {
        setGitHydrated(true);
      }
    }, GIT_HYDRATE_DELAY_MS);
    if (!ENABLE_RESOURCE_WATCH) {
      setWatchHydrated(false);
      return () => {
        disposed = true;
        window.clearTimeout(gitTimer);
      };
    }
    const watchTimer = window.setTimeout(() => {
      if (!disposed) {
        setWatchHydrated(true);
      }
    }, WATCH_HYDRATE_DELAY_MS);

    return () => {
      disposed = true;
      window.clearTimeout(gitTimer);
      window.clearTimeout(watchTimer);
    };
  }, [rootLayerHydrated, workspaceId]);

  const gitStatusData = gitStatusQuery.data ?? null;

  const renameMappings = useMemo(() => {
    return (gitStatusData?.statuses ?? [])
      .filter((entry) => entry.status === "renamed" && entry.originalPath)
      .map((entry) => ({
        from: normalizePath(entry.originalPath ?? ""),
        to: normalizePath(entry.path),
      }))
      .filter((entry) => entry.from && entry.to && entry.from !== entry.to)
      .sort((left, right) => right.from.length - left.from.length);
  }, [gitStatusData?.statuses]);

  useEffect(() => {
    if (!renameMappings.length) {
      return;
    }

    startTransition(() => {
      setChildrenByPath((current) => renameCachedPaths(current, renameMappings));
      setExpandedPaths((current) => remapPathList(current, renameMappings));
      setSelectedPaths((current) => remapPathList(current, renameMappings));
      setAnchorPath((current) => remapSinglePath(current, renameMappings));
      setActivePath((current) => remapSinglePath(current, renameMappings));
    });
  }, [renameMappings]);

  const rootEntry = useMemo<WorkspaceResourceEntry>(
    () => ({
      path: rootPath,
      name: resolveRootLabel(rootPath, workspaceName),
      kind: "directory",
      isHidden: false,
    }),
    [rootPath, workspaceName],
  );

  const flattenedRows = useMemo(() => {
    if (!rootEntry.path) {
      return [];
    }
    return flattenVisibleRows(rootEntry, childrenByPath, new Set(expandedPaths));
  }, [childrenByPath, expandedPaths, rootEntry]);

  const watchTargets = useMemo(() => {
    if (!ENABLE_RESOURCE_WATCH || !watchRootPath) {
      return [];
    }

    const normalizedRootPath = normalizePath(watchRootPath);
    if (!normalizedRootPath) {
      return [];
    }

    const loadedDirectories = new Set(
      Object.keys(childrenByPath).map((path) => normalizePath(path)).filter(Boolean),
    );
    const candidateTargets = new Set<string>([normalizedRootPath]);
    for (const path of expandedPaths) {
      const normalizedPath = normalizePath(path);
      if (!normalizedPath || !isSameOrChildPath(normalizedPath, normalizedRootPath)) {
        continue;
      }
      candidateTargets.add(normalizedPath);
    }

    return Array.from(candidateTargets)
      .filter((path) => path === normalizedRootPath || loadedDirectories.has(path))
      .filter((path) => !containsIgnoredWatchDirectory(path))
      .sort((left, right) => left.length - right.length)
      .slice(0, WATCH_MAX_ACTIVE_DIRS);
  }, [childrenByPath, expandedPaths, watchRootPath]);

  const flattenedVisiblePathSet = useMemo(
    () => new Set(flattenedRows.map((row) => normalizePath(row.entry.path)).filter(Boolean)),
    [flattenedRows],
  );

  const { exactStatusMap, derivedStatusMap } = useMemo(
    () => buildStatusMaps(rootPath, gitStatusData?.statuses ?? [], flattenedVisiblePathSet),
    [flattenedVisiblePathSet, gitStatusData?.statuses, rootPath],
  );

  const statusCounts = useMemo(() => {
    const counts: Record<WorkspaceResourceGitStatus, number> = {
      modified: 0,
      renamed: 0,
      untracked: 0,
      ignored: 0,
    };
    for (const entry of gitStatusData?.statuses ?? []) {
      counts[entry.status] += 1;
    }
    return counts;
  }, [gitStatusData?.statuses]);

  const visibleRows = useMemo(() => {
    if (!rootEntry.path) {
      return [];
    }
    if (!showChangedOnly || !gitStatusData?.gitAvailable) {
      return flattenedRows;
    }

    return flattenedRows.filter((row) => {
      const normalizedPath = normalizePath(row.entry.path);
      return (
        normalizedPath === rootEntry.path ||
        exactStatusMap.has(normalizedPath) ||
        derivedStatusMap.has(normalizedPath)
      );
    });
  }, [
    derivedStatusMap,
    exactStatusMap,
    flattenedRows,
    gitStatusData?.gitAvailable,
    rootEntry,
    showChangedOnly,
  ]);

  const virtualizationEnabled = visibleRows.length >= TREE_VIRTUALIZATION_THRESHOLD;

  const virtualTreeWindow = useMemo(() => {
    if (!virtualizationEnabled) {
      return null;
    }

    const totalRows = visibleRows.length;
    const safeViewportHeight = Math.max(treeViewportHeight, TREE_ROW_HEIGHT);
    const startIndex = Math.max(
      0,
      Math.floor(treeScrollTop / TREE_ROW_HEIGHT) - TREE_OVERSCAN_ROWS,
    );
    const visibleCount = Math.ceil(safeViewportHeight / TREE_ROW_HEIGHT) + TREE_OVERSCAN_ROWS * 2;
    const endIndex = Math.min(totalRows, startIndex + visibleCount);
    const rows = Array.from({ length: Math.max(0, endIndex - startIndex) }, (_, offset) => {
      const index = startIndex + offset;
      return {
        index,
        row: visibleRows[index],
      };
    });

    return {
      rows,
      totalHeight: totalRows * TREE_ROW_HEIGHT,
    };
  }, [treeScrollTop, treeViewportHeight, virtualizationEnabled, visibleRows]);

  const visiblePathOrder = useMemo(
    () => visibleRows.map((row) => row.entry.path),
    [visibleRows],
  );

  useEffect(() => {
    if (!showChangedOnly || !visiblePathOrder.length) {
      return;
    }

    const visiblePathSet = new Set(visiblePathOrder);
    const nextSelectedPaths = selectedPathsRef.current.filter((path) => visiblePathSet.has(path));
    const fallbackSelection =
      nextSelectedPaths.length > 0
        ? nextSelectedPaths
        : rootPath && visiblePathSet.has(rootPath)
          ? [rootPath]
          : visiblePathOrder.slice(0, 1);
    const nextActivePath =
      fallbackSelection.find((path) => path === activePathRef.current) ??
      fallbackSelection.at(-1) ??
      null;
    const nextAnchorPath =
      fallbackSelection.find((path) => path === anchorPathRef.current) ?? nextActivePath;

    if (!isSamePathList(selectedPathsRef.current, fallbackSelection)) {
      setSelectedPaths(fallbackSelection);
    }
    if (activePathRef.current !== nextActivePath) {
      setActivePath(nextActivePath);
    }
    if (anchorPathRef.current !== nextAnchorPath) {
      setAnchorPath(nextAnchorPath);
    }
  }, [rootPath, showChangedOnly, visiblePathOrder]);

  useEffect(() => {
    if (showChangedOnly && gitStatusData?.gitAvailable === false) {
      setShowChangedOnly(false);
    }
  }, [gitStatusData?.gitAvailable, setShowChangedOnly, showChangedOnly]);

  const selectedVisiblePaths = useMemo(() => {
    const selectedSet = new Set(selectedPaths);
    return visiblePathOrder.filter((path) => selectedSet.has(path));
  }, [selectedPaths, visiblePathOrder]);

  const activeEntry = useMemo(() => {
    const targetPath = activePath || selectedVisiblePaths.at(-1) || rootPath;
    return visibleRows.find((row) => row.entry.path === targetPath)?.entry ?? rootEntry;
  }, [activePath, rootEntry, rootPath, selectedVisiblePaths, visibleRows]);

  const reconcileTreeState = useCallback(
    (nextCache: ResourceChildrenCache) => {
      const nextSelectedPaths = prunePathsByLoadedParents(
        selectedPathsRef.current,
        nextCache,
        rootPath,
      );
      const nextExpandedPaths = prunePathsByLoadedParents(
        expandedPathsRef.current,
        nextCache,
        rootPath,
      );
      const nextActivePath =
        nextSelectedPaths.find((path) => path === activePathRef.current) ??
        nextSelectedPaths.at(-1) ??
        rootPath;
      const nextAnchorPath =
        nextSelectedPaths.find((path) => path === anchorPathRef.current) ?? nextActivePath;

      if (!isSamePathList(selectedPathsRef.current, nextSelectedPaths)) {
        setSelectedPaths(nextSelectedPaths);
      }
      if (!isSamePathList(expandedPathsRef.current, nextExpandedPaths)) {
        setExpandedPaths(nextExpandedPaths);
      }
      if (activePathRef.current !== nextActivePath) {
        setActivePath(nextActivePath);
      }
      if (anchorPathRef.current !== nextAnchorPath) {
        setAnchorPath(nextAnchorPath);
      }
    },
    [rootPath],
  );

  const requestDirectoryChildren = useCallback(
    async (targetPath: string, sessionId: number) => {
      const normalizedTargetPath = normalizePath(targetPath);
      if (!normalizedTargetPath) {
        return null;
      }

      const activeRootPath = rootPathRef.current;
      if (
        activeRootPath &&
        normalizedTargetPath !== activeRootPath &&
        !isSameOrChildPath(normalizedTargetPath, activeRootPath)
      ) {
        return null;
      }
      if (workspaceSessionRef.current !== sessionId) {
        return null;
      }

      const children = await listWorkspaceResourceChildren(workspaceId, normalizedTargetPath);
      if (workspaceSessionRef.current !== sessionId) {
        return null;
      }

      return {
        path: normalizedTargetPath,
        children: normalizeChildren(children),
      };
    },
    [workspaceId],
  );

  const applyDirectoryEntries = useCallback(
    (entries: Array<{ path: string; children: WorkspaceResourceEntry[] }>) => {
      if (!entries.length) {
        return;
      }

      const nextCache: ResourceChildrenCache = { ...childrenByPathRef.current };
      let hasChanges = false;

      for (const { path, children } of entries) {
        const existingChildren = nextCache[path];
        if (areResourceChildrenEqual(existingChildren, children)) {
          continue;
        }
        nextCache[path] = children;
        hasChanges = true;
      }

      if (!hasChanges) {
        return;
      }

      childrenByPathRef.current = nextCache;
      startTransition(() => {
        setChildrenByPath(nextCache);
      });
      reconcileTreeState(nextCache);
    },
    [reconcileTreeState],
  );

  const loadDirectoriesBatch = useCallback(
    async (paths: string[], sessionId: number) => {
      const normalizedPaths = Array.from(
        new Set(paths.map((path) => normalizePath(path)).filter(Boolean)),
      );
      if (!normalizedPaths.length) {
        return {
          entries: [] as Array<{ path: string; children: WorkspaceResourceEntry[] }>,
          errorMessage: null as string | null,
        };
      }

      const entries: Array<{ path: string; children: WorkspaceResourceEntry[] }> = [];
      let cursor = 0;
      let errorMessage: string | null = null;

      const workerCount = Math.min(REFRESH_BATCH_CONCURRENCY, normalizedPaths.length);
      const workers = Array.from({ length: workerCount }, async () => {
        while (true) {
          const currentIndex = cursor;
          cursor += 1;
          if (currentIndex >= normalizedPaths.length) {
            return;
          }

          const targetPath = normalizedPaths[currentIndex];
          try {
            const result = await requestDirectoryChildren(targetPath, sessionId);
            if (result) {
              entries.push(result);
            }
          } catch (error) {
            if (!errorMessage) {
              errorMessage = getErrorSummary(error).message;
            }
          }
        }
      });

      await Promise.all(workers);
      return { entries, errorMessage };
    },
    [requestDirectoryChildren],
  );

  const loadDirectory = useCallback(
    async (
      targetPath: string,
      options?: {
        silent?: boolean;
        skipLoadingIndicator?: boolean;
        sessionId?: number;
      },
    ) => {
      const normalizedTargetPath = normalizePath(targetPath);
      if (!normalizedTargetPath) {
        return;
      }

      const sessionId = options?.sessionId ?? workspaceSessionRef.current;
      if (!options?.skipLoadingIndicator) {
        setLoadingPaths((current) =>
          current.includes(normalizedTargetPath) ? current : [...current, normalizedTargetPath],
        );
      }

      try {
        const directoryEntry = await requestDirectoryChildren(normalizedTargetPath, sessionId);
        if (!directoryEntry || workspaceSessionRef.current !== sessionId) {
          return;
        }

        applyDirectoryEntries([directoryEntry]);
        setTreeError(null);
      } catch (error) {
        if (workspaceSessionRef.current !== sessionId) {
          return;
        }
        const summary = getErrorSummary(error);
        setTreeError(summary.message);
        if (!options?.silent) {
          toast.error(summary.message);
        }
      } finally {
        if (!options?.skipLoadingIndicator && workspaceSessionRef.current === sessionId) {
          setLoadingPaths((current) => current.filter((path) => path !== normalizedTargetPath));
        }
      }
    },
    [applyDirectoryEntries, requestDirectoryChildren],
  );

  useEffect(() => {
    if (!workspaceId) {
      setWatchRootPath(null);
      return;
    }
    if (!ENABLE_RESOURCE_WATCH) {
      setWatchRootPath(null);
      setWatchStrategy("disabled");
      return;
    }
    if (!rootLayerHydrated || !watchHydrated) {
      return;
    }

    let disposed = false;
    const sessionId = workspaceSessionRef.current;

    const allowWatchScope = async () => {
      try {
        const scopedRootPath = normalizePath(
          await allowWorkspaceResourceScope(workspaceId),
        );
        if (disposed || workspaceSessionRef.current !== sessionId) {
          return;
        }

        setWatchRootPath(scopedRootPath || normalizePath(workspacePath) || null);
        setWatchStrategy("native");
      } catch (error) {
        if (disposed || workspaceSessionRef.current !== sessionId) {
          return;
        }

        setWatchRootPath(null);
        setWatchStrategy("polling");
        toast.warning("原生文件监听授权失败，已回退到轮询刷新", {
          description: getErrorSummary(error).message,
        });
      }
    };

    void allowWatchScope();

    return () => {
      disposed = true;
    };
  }, [rootLayerHydrated, watchHydrated, workspaceId, workspacePath]);

  useEffect(() => {
    if (!rootPath) {
      return;
    }
    const sessionId = workspaceSessionRef.current;
    void loadDirectory(rootPath, {
      silent: true,
      skipLoadingIndicator: true,
      sessionId,
    });
  }, [loadDirectory, rootPath, workspaceId]);

  const resolveRefreshTargets = useCallback(
    (paths?: string[]) => {
      if (!rootPath) {
        return [];
      }

      if (!paths?.length) {
        return Array.from(new Set([rootPath, ...expandedPathsRef.current]));
      }

      const loadedDirectories = new Set([
        rootPath,
        ...Object.keys(childrenByPathRef.current).map((path) => normalizePath(path)),
      ]);
      const refreshTargets = new Set<string>([rootPath]);

      for (const rawPath of paths) {
        let currentPath = normalizePath(rawPath);
        if (!currentPath) {
          continue;
        }

        if (!isSameOrChildPath(currentPath, rootPath) && currentPath !== rootPath) {
          currentPath = normalizePath(resolveParentPath(currentPath));
        }

        while (currentPath && isSameOrChildPath(currentPath, rootPath)) {
          if (loadedDirectories.has(currentPath)) {
            refreshTargets.add(currentPath);
          }

          if (currentPath === rootPath) {
            break;
          }

          const parentPath = normalizePath(resolveParentPath(currentPath));
          if (!parentPath || parentPath === currentPath) {
            break;
          }
          currentPath = parentPath;
        }
      }

      return Array.from(refreshTargets);
    },
    [rootPath],
  );

  const queueRefreshTargets = useCallback((paths?: string[]) => {
    if (!paths?.length) {
      refreshQueuedRef.current = true;
      queuedRefreshPathsRef.current.clear();
      return;
    }

    for (const path of paths) {
      queuedRefreshPathsRef.current.add(path);
    }
    refreshQueuedRef.current = true;
  }, []);

  const consumeQueuedRefreshTargets = useCallback(() => {
    if (!refreshQueuedRef.current) {
      return null;
    }

    refreshQueuedRef.current = false;
    if (!queuedRefreshPathsRef.current.size) {
      return [];
    }

    const nextPaths = Array.from(queuedRefreshPathsRef.current);
    queuedRefreshPathsRef.current.clear();
    return nextPaths;
  }, []);

  const refreshVisibleDirectories = useCallback(async (paths?: string[]) => {
    if (!rootPath) {
      return;
    }

    if (refreshInFlightRef.current) {
      queueRefreshTargets(paths);
      return;
    }

    refreshInFlightRef.current = true;
    let refreshTargets: string[] = [];
    const sessionId = workspaceSessionRef.current;
    try {
      refreshTargets = Array.from(
        new Set(resolveRefreshTargets(paths).map((path) => normalizePath(path)).filter(Boolean)),
      );
      if (!refreshTargets.length) {
        return;
      }

      setLoadingPaths((current) => {
        const next = new Set(current);
        for (const path of refreshTargets) {
          next.add(path);
        }
        return Array.from(next);
      });

      const { entries, errorMessage } = await loadDirectoriesBatch(refreshTargets, sessionId);
      if (workspaceSessionRef.current !== sessionId) {
        return;
      }

      applyDirectoryEntries(entries);
      if (errorMessage) {
        setTreeError(errorMessage);
      } else if (entries.length) {
        setTreeError(null);
      }
      if (gitHydrated) {
        await gitStatusQuery.refetch();
      }
    } finally {
      if (refreshTargets.length && workspaceSessionRef.current === sessionId) {
        const targetSet = new Set(refreshTargets);
        setLoadingPaths((current) => current.filter((path) => !targetSet.has(path)));
      }
      refreshInFlightRef.current = false;
      const queuedPaths = consumeQueuedRefreshTargets();
      if (queuedPaths) {
        void refreshVisibleDirectories(queuedPaths);
      }
    }
  }, [
    consumeQueuedRefreshTargets,
    applyDirectoryEntries,
    gitStatusQuery,
    gitHydrated,
    loadDirectoriesBatch,
    queueRefreshTargets,
    resolveRefreshTargets,
    rootPath,
  ]);

  const scheduleWatchRefresh = useCallback(
    (paths: string[]) => {
      const normalizedEventPaths = paths
        .map((path) => normalizePath(path))
        .filter(Boolean)
        .filter((path) => !containsIgnoredWatchDirectory(path));
      if (!normalizedEventPaths.length) {
        return;
      }

      const refreshTargets = resolveRefreshTargets(normalizedEventPaths);
      for (const path of refreshTargets) {
        watchEventPathsRef.current.add(path);
      }

      if (watchRefreshTimerRef.current !== null) {
        return;
      }

      const elapsedSinceLastRefresh = Date.now() - lastWatchRefreshAtRef.current;
      const refreshDelay =
        elapsedSinceLastRefresh >= WATCH_REFRESH_MIN_GAP_MS
          ? NATIVE_WATCH_DELAY_MS
          : Math.max(NATIVE_WATCH_DELAY_MS, WATCH_REFRESH_MIN_GAP_MS - elapsedSinceLastRefresh);

      watchRefreshTimerRef.current = window.setTimeout(() => {
        const pendingPaths = Array.from(watchEventPathsRef.current);
        watchEventPathsRef.current.clear();
        watchRefreshTimerRef.current = null;
        lastWatchRefreshAtRef.current = Date.now();
        void refreshVisibleDirectories(pendingPaths);
      }, refreshDelay);
    },
    [refreshVisibleDirectories, resolveRefreshTargets],
  );

  useEffect(() => {
    if (!watchRootPath) {
      return;
    }

    let disposed = false;
    const unwatchers: FsUnwatchFn[] = [];

    const setupWatch = async () => {
      try {
        if (!watchTargets.length) {
          if (!disposed) {
            setWatchStrategy("native");
          }
          return;
        }

        for (const targetPath of watchTargets) {
          const unwatch = await watch(
            targetPath,
            (event) => {
              if (disposed) {
                return;
              }
              scheduleWatchRefresh(event.paths);
            },
            {
              recursive: false,
              delayMs: NATIVE_WATCH_DELAY_MS,
            },
          );
          if (disposed) {
            unwatch();
            return;
          }
          unwatchers.push(unwatch);
        }

        if (!disposed) {
          setWatchStrategy("native");
        }
      } catch (error) {
        if (disposed) {
          return;
        }
        setWatchStrategy("polling");
        toast.warning("原生文件监听启动失败，已回退到轮询刷新", {
          description: getErrorSummary(error).message,
        });
      }
    };

    void setupWatch();

    return () => {
      disposed = true;
      if (watchRefreshTimerRef.current !== null) {
        window.clearTimeout(watchRefreshTimerRef.current);
        watchRefreshTimerRef.current = null;
      }
      watchEventPathsRef.current.clear();
      for (const stopWatching of unwatchers) {
        stopWatching();
      }
    };
  }, [scheduleWatchRefresh, watchRootPath, watchTargets]);

  useEffect(() => {
    if (!rootPath || watchStrategy !== "polling") {
      return;
    }

    const intervalId = window.setInterval(() => {
      if (typeof document !== "undefined" && document.visibilityState !== "visible") {
        return;
      }
      void refreshVisibleDirectories();
    }, WATCH_REFRESH_INTERVAL_MS);
    return () => window.clearInterval(intervalId);
  }, [refreshVisibleDirectories, rootPath, watchStrategy]);

  const handleNativeDrag = useCallback(
    async (paths: string[]) => {
      if (dragInFlightRef.current) {
        return;
      }

      const normalizedPaths = Array.from(
        new Set(paths.map((path) => normalizePath(path)).filter(Boolean)),
      );
      if (!normalizedPaths.length) {
        toast.error("没有可拖拽的资源");
        return;
      }

      dragInFlightRef.current = true;
      try {
        await startDrag(
          {
            item: normalizedPaths,
            icon: DRAG_PREVIEW_IMAGE,
            mode: "copy",
          },
          ({ result }) => {
            if (result === "Cancelled") {
              return;
            }
            toast.success(
              normalizedPaths.length > 1
                ? `已拖出 ${normalizedPaths.length} 个资源`
                : "已拖出资源",
              {
                description: normalizedPaths[0],
              },
            );
          },
        );
      } catch (error) {
        toast.error(getErrorSummary(error).message);
      } finally {
        dragInFlightRef.current = false;
      }
    },
    [],
  );

  const handleStartDrag = useCallback(
    (path: string, useSelection: boolean) => {
      const dragPaths = sortPathsByVisibleOrder(
        useSelection ? selectedPathsRef.current : [path],
        visiblePathOrder,
      );
      void handleNativeDrag(dragPaths);
    },
    [handleNativeDrag, visiblePathOrder],
  );

  const handleCopySelectedPaths = useCallback(async () => {
    const paths = selectedVisiblePaths.length
      ? selectedVisiblePaths
      : activeEntry.path
        ? [activeEntry.path]
        : [];
    if (!paths.length) {
      toast.error("请先选择文件或文件夹");
      return;
    }

    try {
      await copyTextToClipboard(paths.join("\n"), "选中资源为空");
      toast.success(paths.length > 1 ? `已复制 ${paths.length} 个资源路径` : "已复制资源路径", {
        description: paths[0],
      });
    } catch (error) {
      toast.error(getClipboardErrorMessage(error, "复制资源路径失败"));
    }
  }, [activeEntry.path, selectedVisiblePaths]);

  const handleRevealPath = useCallback(
    async (path?: string | null) => {
      const targetPath = normalizePath(path ?? activeEntry.path ?? "");
      if (!targetPath) {
        toast.error("请先选择文件或文件夹");
        return;
      }

      try {
        await revealItemInDir(targetPath);
      } catch (error) {
        toast.error(getErrorSummary(error).message);
      }
    },
    [activeEntry.path],
  );

  const handleToggleExpand = useCallback(
    (path: string) => {
      const normalizedPath = normalizePath(path);
      if (!normalizedPath) {
        return;
      }

      if (expandedPathsRef.current.includes(normalizedPath)) {
        setExpandedPaths((current) => current.filter((item) => item !== normalizedPath));
        return;
      }

      setExpandedPaths((current) => [...current, normalizedPath]);
      if (!childrenByPathRef.current[normalizedPath]) {
        void loadDirectory(normalizedPath, { silent: true });
      }
    },
    [loadDirectory],
  );

  const handleSelectPath = useCallback(
    (path: string, event?: ReactMouseEvent) => {
      const normalizedPath = normalizePath(path);
      if (!normalizedPath) {
        return;
      }

      browserContainerRef.current?.focus();
      if (event?.shiftKey && anchorPathRef.current) {
        const nextSelection = buildRangeSelection(
          visiblePathOrder,
          anchorPathRef.current,
          normalizedPath,
        );
        setSelectedPaths(nextSelection);
        setActivePath(normalizedPath);
        return;
      }

      if (event?.ctrlKey || event?.metaKey) {
        setSelectedPaths((current) => {
          const nextSelection = current.includes(normalizedPath)
            ? current.filter((item) => item !== normalizedPath)
            : [...current, normalizedPath];
          return nextSelection.length ? nextSelection : [normalizedPath];
        });
        setAnchorPath(normalizedPath);
        setActivePath(normalizedPath);
        return;
      }

      setSelectedPaths([normalizedPath]);
      setAnchorPath(normalizedPath);
      setActivePath(normalizedPath);
    },
    [visiblePathOrder],
  );

  useEffect(() => {
    const element = browserContainerRef.current;
    if (!element) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || !event.shiftKey) {
        return;
      }

      const key = event.key.toLowerCase();
      if (key === "c") {
        event.preventDefault();
        void handleCopySelectedPaths();
      }
      if (key === "r") {
        event.preventDefault();
        void handleRevealPath();
      }
    };

    element.addEventListener("keydown", handleKeyDown);
    return () => element.removeEventListener("keydown", handleKeyDown);
  }, [handleCopySelectedPaths, handleRevealPath]);

  const handleTreeScroll = useCallback((event: ReactUIEvent<HTMLDivElement>) => {
    const nextScrollTop = event.currentTarget.scrollTop;
    if (treeScrollRafRef.current !== null) {
      window.cancelAnimationFrame(treeScrollRafRef.current);
    }

    treeScrollRafRef.current = window.requestAnimationFrame(() => {
      treeScrollRafRef.current = null;
      setTreeScrollTop(nextScrollTop);
    });
  }, []);

  const handleResizePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      resizeStateRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startWidth: paneWidth,
      };

      const target = event.currentTarget;
      target.setPointerCapture(event.pointerId);

      const handlePointerMove = (moveEvent: PointerEvent) => {
        const state = resizeStateRef.current;
        if (!state || state.pointerId !== moveEvent.pointerId) {
          return;
        }

        const nextWidth = clamp(
          state.startWidth + (moveEvent.clientX - state.startX),
          MIN_PANE_WIDTH,
          MAX_PANE_WIDTH,
        );
        setPaneWidth(nextWidth);
        if (isCollapsed && nextWidth > MIN_PANE_WIDTH) {
          setIsCollapsed(false);
        }
      };

      const handlePointerDone = (pointerEvent: PointerEvent) => {
        if (resizeStateRef.current?.pointerId !== pointerEvent.pointerId) {
          return;
        }
        target.releasePointerCapture(pointerEvent.pointerId);
        target.removeEventListener("pointermove", handlePointerMove);
        target.removeEventListener("pointerup", handlePointerDone);
        target.removeEventListener("pointercancel", handlePointerDone);
        resizeStateRef.current = null;
      };

      target.addEventListener("pointermove", handlePointerMove);
      target.addEventListener("pointerup", handlePointerDone);
      target.addEventListener("pointercancel", handlePointerDone);
    },
    [isCollapsed, paneWidth, setIsCollapsed, setPaneWidth],
  );

  const contextMenuItems = useMemo<MenuProps["items"]>(
    () => [
      { key: "reveal", icon: <EyeOutlined />, label: "在资源管理器中显示" },
      { key: "open-terminal", icon: <ConsoleSqlOutlined />, label: "在这里打开终端" },
      { key: "copy", icon: <CopyOutlined />, label: "复制绝对路径" },
    ],
    [],
  );

  const renderTreeRow = useCallback(
    (row: ResourceTreeRow) => {
      const normalizedPath = normalizePath(row.entry.path);
      const isDirectory = row.entry.kind === "directory";
      const isExpanded = expandedPaths.includes(normalizedPath);
      const isSelected = selectedPaths.includes(normalizedPath);
      const isLoading = loadingPaths.includes(normalizedPath);
      const displayStatus =
        exactStatusMap.get(normalizedPath)?.status ?? derivedStatusMap.get(normalizedPath);

      return (
        <ResourceTreeItem
          activePath={normalizedPath}
          contextMenuItems={contextMenuItems}
          depth={row.depth}
          displayStatus={displayStatus}
          entry={row.entry}
          expanded={isExpanded}
          isDirectory={isDirectory}
          isLoading={isLoading}
          isSelected={isSelected}
          onStartDrag={handleStartDrag}
          onContextMenuAction={(key) => {
            if (key === "reveal") {
              void handleRevealPath(normalizedPath);
              return;
            }
            if (key === "open-terminal") {
              void openWorkspaceTerminalAtPath(workspaceId, normalizedPath)
                .then((result) => {
                  toast.success("已打开终端", {
                    description: result.workspacePath,
                  });
                })
                .catch((error) => {
                  toast.error(getErrorSummary(error).message);
                });
              return;
            }
            if (key === "copy") {
              void copyTextToClipboard(normalizedPath, "选中资源为空")
                .then(() => {
                  toast.success("已复制资源路径", {
                    description: normalizedPath,
                  });
                })
                .catch((error) => {
                  toast.error(
                    getClipboardErrorMessage(error, "复制资源路径失败"),
                  );
                });
            }
          }}
          onSelect={handleSelectPath}
          onToggleExpand={handleToggleExpand}
        />
      );
    },
    [
      contextMenuItems,
      derivedStatusMap,
      exactStatusMap,
      expandedPaths,
      handleRevealPath,
      handleSelectPath,
      handleStartDrag,
      handleToggleExpand,
      loadingPaths,
      selectedPaths,
      workspaceId,
    ],
  );

  const renderedPaneWidth = isCollapsed
    ? 44
    : clamp(paneWidth, MIN_PANE_WIDTH, MAX_PANE_WIDTH);

  return (
    <div className="flex min-h-0 shrink-0">
      <div
        className={cn(
          "flex min-h-0 shrink-0 flex-col border-r border-[#e6edf5] bg-[#fbfcfe] transition-[width] duration-200",
          isCollapsed && "items-center",
        )}
        ref={browserContainerRef}
        style={{ width: renderedPaneWidth }}
        tabIndex={0}
      >
        <div
          className={cn(
            "flex items-center border-b border-[#e6edf5] px-3 py-3",
            isCollapsed ? "justify-center" : "justify-between",
          )}
        >
          {isCollapsed ? (
            <Tooltip placement="right" title="展开资源浏览器">
              <Button
                className="!h-8 !w-8 !min-w-8 !p-0"
                icon={<MenuUnfoldOutlined />}
                onClick={() => setIsCollapsed(false)}
                size="small"
                type="text"
              />
            </Tooltip>
          ) : (
            <>
              <div className="min-w-0">
                <Typography.Text className="block text-[11px] font-semibold uppercase tracking-[0.16em] text-[#1f2937]">
                  Resource Browser
                </Typography.Text>
                <Typography.Text className="block text-[11px] text-[#667085]">
                  {workspaceName}
                </Typography.Text>
              </div>
              <div className="flex items-center gap-1">
                <ToolbarButton
                  icon={
                    isRootLayerLoading || loadingPaths.length || gitStatusQuery.isFetching ? (
                      <Spin size="small" />
                    ) : (
                      <ReloadOutlined />
                    )
                  }
                  title="刷新资源树"
                  onClick={() => void refreshVisibleDirectories()}
                />
                <ToolbarButton
                  icon={<CopyOutlined />}
                  title="复制选中资源绝对路径（Ctrl+Shift+C）"
                  onClick={() => void handleCopySelectedPaths()}
                />
                <ToolbarButton
                  icon={<FolderOpenOutlined />}
                  title="在资源管理器中显示（Ctrl+Shift+R）"
                  onClick={() => void handleRevealPath()}
                />
                <ToolbarButton
                  icon={<MenuFoldOutlined />}
                  title="折叠资源浏览器"
                  onClick={() => setIsCollapsed(true)}
                />
              </div>
            </>
          )}
        </div>

        {!isCollapsed ? (
          <>
            <div className="border-b border-[#eef2f6] px-3 py-2">
              <div className="flex flex-wrap items-center gap-1.5">
                <StatusTag count={statusCounts.modified} label="M" tone="modified" />
                <StatusTag count={statusCounts.renamed} label="R" tone="renamed" />
                <StatusTag count={statusCounts.untracked} label="U" tone="untracked" />
                <StatusTag count={statusCounts.ignored} label="I" tone="ignored" />
                <Button
                  className={cn(
                    "!h-5 !rounded-full !border !px-2 !text-[10px] !font-semibold !shadow-none",
                    showChangedOnly
                      ? "!border-[#1697c5] !bg-[#e7f5fb] !text-[#0d6987]"
                      : "!border-[#d8e1eb] !bg-white !text-[#667085]",
                  )}
                  disabled={!gitStatusData?.gitAvailable}
                  onClick={() => setShowChangedOnly((current) => !current)}
                  size="small"
                  type="default"
                >
                  仅看变更
                </Button>
              </div>
              {gitStatusData?.gitAvailable === false ? (
                <Typography.Text className="mt-2 block text-[11px] text-[#98a2b3]">
                  当前目录不是 Git 仓库，已关闭状态筛选。
                </Typography.Text>
              ) : null}
              <Typography.Text
                className="mt-2 block font-mono text-[11px] leading-5 text-[#98a2b3]"
                title={rootPath}
              >
                {rootPath}
              </Typography.Text>
            </div>

            {treeError ? (
              <Alert
                className="m-3 mb-0"
                message="资源浏览器读取失败"
                type="error"
                showIcon
                description={treeError}
              />
            ) : null}

            {!rootPath ? (
              <div className="flex min-h-0 flex-1 items-center justify-center px-3 py-4">
                <Empty description="当前工作区没有有效目录" image={Empty.PRESENTED_IMAGE_SIMPLE} />
              </div>
            ) : (
              <div
                className="min-h-0 flex-1 overflow-auto px-2 py-2"
                style={{ contain: "strict" }}
                ref={treeViewportRef}
                onScroll={virtualizationEnabled ? handleTreeScroll : undefined}
              >
                {visibleRows.length ? (
                  virtualizationEnabled && virtualTreeWindow ? (
                    <div
                      style={{
                        height: `${virtualTreeWindow.totalHeight}px`,
                        position: "relative",
                      }}
                    >
                      {virtualTreeWindow.rows.map(({ index, row }) => (
                        <div
                          className="absolute left-0 right-0"
                          key={`${row.entry.path}-${index}`}
                          style={{
                            height: `${TREE_ROW_HEIGHT}px`,
                            top: `${index * TREE_ROW_HEIGHT}px`,
                          }}
                        >
                          {renderTreeRow(row)}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="space-y-0.5">
                      {visibleRows.map((row) => (
                        <div key={`${row.entry.path}-${row.depth}`}>
                          {renderTreeRow(row)}
                        </div>
                      ))}
                      {isRootLayerLoading ? <ResourceTreeLoadingRows depth={1} /> : null}
                    </div>
                  )
                ) : (
                  <div className="flex min-h-full items-center justify-center rounded-[12px] border border-dashed border-[#e6edf5] bg-white/80 px-4 py-6">
                    <Empty description="当前目录没有可显示的资源" image={Empty.PRESENTED_IMAGE_SIMPLE} />
                  </div>
                )}
              </div>
            )}
          </>
        ) : null}
      </div>

      {!isCollapsed ? (
        <div
          className="group flex w-2 cursor-col-resize items-center justify-center bg-transparent"
          onPointerDown={handleResizePointerDown}
        >
          <div className="h-12 w-[3px] rounded-full bg-[#d8e1eb] transition-colors group-hover:bg-[#1697c5]" />
        </div>
      ) : null}
    </div>
  );
}

function areResourceChildrenEqual(
  left: WorkspaceResourceEntry[] | undefined,
  right: WorkspaceResourceEntry[] | undefined,
) {
  if (!left || !right) {
    return false;
  }
  if (left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    const leftEntry = left[index];
    const rightEntry = right[index];
    if (
      leftEntry.path !== rightEntry.path ||
      leftEntry.name !== rightEntry.name ||
      leftEntry.kind !== rightEntry.kind ||
      leftEntry.isHidden !== rightEntry.isHidden
    ) {
      return false;
    }
  }

  return true;
}

function containsIgnoredWatchDirectory(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) {
    return false;
  }

  const segments = normalizedPath
    .replace(/^[A-Za-z]:[\\/]?/, "")
    .split(/[\\/]+/)
    .filter(Boolean);

  for (const segment of segments) {
    if (WATCH_IGNORED_DIRECTORY_NAMES.has(segment.toLowerCase())) {
      return true;
    }
  }

  return false;
}

type ResourceTreeItemProps = {
  activePath: string;
  contextMenuItems: MenuProps["items"];
  depth: number;
  displayStatus?: WorkspaceResourceGitStatus;
  entry: WorkspaceResourceEntry;
  expanded: boolean;
  isDirectory: boolean;
  isLoading: boolean;
  isSelected: boolean;
  onStartDrag: (path: string, useSelection: boolean) => void;
  onContextMenuAction: (key: string) => void;
  onSelect: (path: string, event?: ReactMouseEvent) => void;
  onToggleExpand: (path: string) => void;
};

const ResourceTreeItem = memo(function ResourceTreeItem({
  activePath,
  contextMenuItems,
  depth,
  displayStatus,
  entry,
  expanded,
  isDirectory,
  isLoading,
  isSelected,
  onStartDrag,
  onContextMenuAction,
  onSelect,
  onToggleExpand,
}: ResourceTreeItemProps) {
  const menu = {
    items: contextMenuItems,
    onClick: ({ key }: { key: string }) => onContextMenuAction(key),
  } satisfies MenuProps;

  return (
    <Dropdown menu={menu} trigger={["contextMenu"]}>
      <div
        className={cn(
          "group flex h-8 items-center gap-1 rounded-[8px] px-2 text-[12px] text-[#1f2937] transition-colors",
          isSelected ? "bg-[#e7f5fb] text-[#0d6987]" : "hover:bg-[#f1f5f9]",
          displayStatus === "ignored" && "text-[#98a2b3]",
        )}
        draggable
        style={{ paddingLeft: `${depth * 14 + 8}px` }}
        onClick={(event) => onSelect(activePath, event)}
        onContextMenu={(event) => {
          onSelect(activePath, event);
          event.preventDefault();
        }}
        onDoubleClick={() => {
          if (isDirectory) {
            onToggleExpand(activePath);
          }
        }}
        onDragStart={(event) => {
          event.preventDefault();
          onStartDrag(activePath, isSelected);
        }}
      >
        {isDirectory ? (
          <button
            className="flex h-5 w-5 items-center justify-center rounded-[6px] text-[#98a2b3]"
            onClick={(event) => {
              event.stopPropagation();
              onToggleExpand(activePath);
            }}
            type="button"
          >
            {isLoading ? (
              <Spin size="small" />
            ) : (
              <CaretRightOutlined
                className={cn("transition-transform", expanded && "rotate-90")}
              />
            )}
          </button>
        ) : (
          <span className="h-5 w-5" />
        )}

        <span
          className={cn(
            "flex h-4 w-4 shrink-0 items-center justify-center",
            getStatusToneClass(displayStatus, "icon"),
          )}
        >
          {isDirectory ? (
            expanded ? (
              <FolderOpenOutlined />
            ) : (
              <FolderOutlined />
            )
          ) : (
            <FileOutlined />
          )}
        </span>

        <span
          className={cn(
            "min-w-0 flex-1 truncate",
            getStatusToneClass(displayStatus, "text"),
            entry.isHidden && "opacity-70",
          )}
          title={entry.path}
        >
          {entry.name}
        </span>

        {displayStatus ? (
          <span
            className={cn(
              "shrink-0 rounded-full px-1.5 py-0 text-[10px] font-semibold uppercase tracking-[0.12em]",
              getStatusToneClass(displayStatus, "badge"),
            )}
          >
            {statusLabel(displayStatus)}
          </span>
        ) : null}
      </div>
    </Dropdown>
  );
}, (previous, next) =>
  previous.activePath === next.activePath &&
  previous.depth === next.depth &&
  previous.displayStatus === next.displayStatus &&
  previous.expanded === next.expanded &&
  previous.isDirectory === next.isDirectory &&
  previous.isLoading === next.isLoading &&
  previous.isSelected === next.isSelected &&
  previous.entry.path === next.entry.path &&
  previous.entry.name === next.entry.name &&
  previous.entry.kind === next.entry.kind &&
  previous.entry.isHidden === next.entry.isHidden
);

type ToolbarButtonProps = {
  icon: ReactNode;
  title: string;
  onClick: () => void;
};

function ToolbarButton({ icon, title, onClick }: ToolbarButtonProps) {
  return (
    <Tooltip placement="bottom" title={title}>
      <Button
        className="!h-7 !w-7 !min-w-7 !p-0"
        icon={icon}
        onClick={onClick}
        size="small"
        type="text"
      />
    </Tooltip>
  );
}

type StatusTagProps = {
  count: number;
  label: string;
  tone: WorkspaceResourceGitStatus;
};

function StatusTag({ count, label, tone }: StatusTagProps) {
  return (
    <Tag
      bordered={false}
      className={cn(
        "m-0 rounded-full px-2 py-0 text-[10px] font-semibold uppercase tracking-[0.12em]",
        getStatusToneClass(tone, "badge"),
      )}
    >
      {label} {count}
    </Tag>
  );
}

type ResourceTreeLoadingRowsProps = {
  depth: number;
};

function ResourceTreeLoadingRows({ depth }: ResourceTreeLoadingRowsProps) {
  return (
    <>
      {Array.from({ length: 4 }, (_, index) => (
        <div
          className="flex h-8 items-center gap-2 rounded-[8px] px-2 opacity-70"
          key={`resource-tree-loading-${index}`}
          style={{ paddingLeft: `${depth * 14 + 8}px` }}
        >
          <span className="h-4 w-4 shrink-0 rounded-[4px] bg-[#e6edf5]" />
          <span
            className={cn(
              "h-3 rounded-full bg-[#e6edf5] animate-pulse",
              index % 2 === 0 ? "w-[42%]" : "w-[58%]",
            )}
          />
        </div>
      ))}
    </>
  );
}
