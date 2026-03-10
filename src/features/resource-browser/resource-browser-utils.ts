import type {
  WorkspaceResourceEntry,
  WorkspaceResourceGitStatus,
  WorkspaceResourceGitStatusEntry,
} from "@/types/backend";

export type ResourceChildrenCache = Record<string, WorkspaceResourceEntry[]>;

export type ResourceTreeRow = {
  entry: WorkspaceResourceEntry;
  depth: number;
};

export type RenameMapping = {
  from: string;
  to: string;
};

export const DEFAULT_PANE_WIDTH = 320;
export const MIN_PANE_WIDTH = 220;
export const MAX_PANE_WIDTH = 520;

const STATUS_PRIORITY: Record<WorkspaceResourceGitStatus, number> = {
  modified: 4,
  renamed: 3,
  untracked: 2,
  ignored: 1,
};

export function flattenVisibleRows(
  rootEntry: WorkspaceResourceEntry,
  childrenByPath: ResourceChildrenCache,
  expandedPaths: Set<string>,
) {
  const rows: ResourceTreeRow[] = [];

  function visit(entry: WorkspaceResourceEntry, depth: number) {
    const normalizedPath = normalizePath(entry.path);
    rows.push({ entry, depth });

    if (entry.kind !== "directory" || !expandedPaths.has(normalizedPath)) {
      return;
    }

    const children = childrenByPath[normalizedPath] ?? [];
    for (const child of children) {
      visit(child, depth + 1);
    }
  }

  visit(rootEntry, 0);
  return rows;
}

export function prunePathsByLoadedParents(
  paths: string[],
  cache: ResourceChildrenCache,
  rootPath: string,
) {
  const deduped = Array.from(new Set(paths.map(normalizePath).filter(Boolean)));
  return deduped.filter((path) => {
    if (path === rootPath) {
      return true;
    }

    const parentPath = normalizePath(resolveParentPath(path));
    if (!parentPath || !cache[parentPath]) {
      return true;
    }

    return cache[parentPath].some((entry) => normalizePath(entry.path) === path);
  });
}

export function buildRangeSelection(
  visiblePaths: string[],
  anchorPath: string,
  targetPath: string,
) {
  const anchorIndex = visiblePaths.indexOf(anchorPath);
  const targetIndex = visiblePaths.indexOf(targetPath);
  if (anchorIndex < 0 || targetIndex < 0) {
    return [targetPath];
  }

  const [start, end] =
    anchorIndex <= targetIndex ? [anchorIndex, targetIndex] : [targetIndex, anchorIndex];
  return visiblePaths.slice(start, end + 1);
}

export function resolveRootLabel(rootPath: string, fallback: string) {
  const normalizedPath = normalizePath(rootPath);
  if (!normalizedPath) {
    return fallback.trim() || "工作区";
  }

  const segments = normalizedPath.split(/[\\/]/).filter(Boolean);
  return segments.at(-1) ?? (fallback.trim() || "工作区");
}

export function normalizePath(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }

  const withoutVerbatimPrefix = stripWindowsVerbatimPrefix(trimmed);
  const normalized = withoutVerbatimPrefix.replace(/[\\/]+$/, "");
  if (/^[A-Za-z]:$/.test(normalized)) {
    return `${normalized.toUpperCase()}\\`;
  }

  if (/^[A-Za-z]:[\\/]/.test(normalized)) {
    return `${normalized.charAt(0).toUpperCase()}${normalized.slice(1)}`;
  }

  return normalized;
}

export function isSameOrChildPath(path: string, parentPath: string) {
  const normalizedPath = toComparablePath(path);
  const normalizedParentPath = toComparablePath(parentPath);
  if (!normalizedPath || !normalizedParentPath) {
    return false;
  }

  return (
    normalizedPath === normalizedParentPath ||
    normalizedPath.startsWith(`${normalizedParentPath}\\`)
  );
}

function stripWindowsVerbatimPrefix(path: string) {
  if (path.startsWith("\\\\?\\UNC\\")) {
    return `\\\\${path.slice(8)}`;
  }
  if (path.startsWith("\\\\?\\")) {
    return path.slice(4);
  }
  if (path.startsWith("//?/UNC/")) {
    return `//${path.slice(8)}`;
  }
  if (path.startsWith("//?/")) {
    return path.slice(4);
  }
  return path;
}

function toComparablePath(path: string) {
  return normalizePath(path).replace(/\//g, "\\").toLowerCase();
}

export function resolveParentPath(path: string) {
  const normalizedPath = normalizePath(path);
  const nextValue = normalizedPath.replace(/[\\/][^\\/]+$/, "");
  return nextValue === normalizedPath ? normalizedPath : normalizePath(nextValue);
}

export function buildStatusMaps(
  rootPath: string,
  statuses: WorkspaceResourceGitStatusEntry[],
  visiblePaths?: Set<string>,
) {
  const exactStatusMap = new Map<string, WorkspaceResourceGitStatusEntry>();
  const derivedStatusMap = new Map<string, WorkspaceResourceGitStatus>();

  for (const entry of statuses) {
    const normalizedPath = normalizePath(entry.path);
    if (!visiblePaths || visiblePaths.has(normalizedPath)) {
      exactStatusMap.set(normalizedPath, entry);
    }

    if (rootPath && isSameOrChildPath(normalizedPath, rootPath)) {
      applyStatusToAncestors(
        derivedStatusMap,
        normalizedPath,
        entry.status,
        rootPath,
        visiblePaths,
      );
    }

    if (entry.originalPath) {
      const normalizedOriginalPath = normalizePath(entry.originalPath);
      if (rootPath && isSameOrChildPath(normalizedOriginalPath, rootPath)) {
        applyStatusToAncestors(
          derivedStatusMap,
          normalizedOriginalPath,
          entry.status,
          rootPath,
          visiblePaths,
        );
      }
    }
  }

  return {
    exactStatusMap,
    derivedStatusMap,
  };
}

function applyStatusToAncestors(
  statusMap: Map<string, WorkspaceResourceGitStatus>,
  path: string,
  status: WorkspaceResourceGitStatus,
  rootPath: string,
  visiblePaths?: Set<string>,
) {
  let currentPath = normalizePath(path);

  while (currentPath && isSameOrChildPath(currentPath, rootPath)) {
    if (!visiblePaths || visiblePaths.has(currentPath)) {
      const previousStatus = statusMap.get(currentPath);
      if (!previousStatus || STATUS_PRIORITY[status] > STATUS_PRIORITY[previousStatus]) {
        statusMap.set(currentPath, status);
      }
    }

    if (currentPath === rootPath) {
      break;
    }

    currentPath = normalizePath(resolveParentPath(currentPath));
  }
}

export function getStatusToneClass(
  status: WorkspaceResourceGitStatus | undefined,
  target: "text" | "icon" | "badge",
) {
  if (status === "modified") {
    return target === "badge"
      ? "text-[#b8860b]"
      : target === "icon"
        ? "text-[#c99600]"
        : "text-[#c99600]";
  }
  if (status === "renamed") {
    return target === "badge"
      ? "text-[#c2410c]"
      : target === "icon"
        ? "text-[#c2410c]"
        : "text-[#b45309]";
  }
  if (status === "untracked") {
    return target === "badge"
      ? "text-[#22c55e]"
      : target === "icon"
        ? "text-[#22c55e]"
        : "text-[#22c55e]";
  }
  if (status === "ignored") {
    return target === "badge"
      ? "text-[#6b7280]"
      : target === "icon"
        ? "text-[#9ca3af]"
        : "text-[#98a2b3]";
  }
  return "";
}

export function statusLabel(status: WorkspaceResourceGitStatus) {
  if (status === "modified") return "M";
  if (status === "renamed") return "R";
  if (status === "untracked") return "U";
  return "I";
}
