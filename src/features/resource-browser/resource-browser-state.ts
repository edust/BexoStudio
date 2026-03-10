import { useEffect, useState } from "react";

import type { WorkspaceResourceEntry } from "@/types/backend";

import {
  DEFAULT_PANE_WIDTH,
  isSameOrChildPath,
  MAX_PANE_WIDTH,
  MIN_PANE_WIDTH,
  normalizePath,
  type RenameMapping,
  type ResourceChildrenCache,
} from "@/features/resource-browser/resource-browser-utils";

export function remapSinglePath(path: string | null, mappings: RenameMapping[]) {
  if (!path) {
    return path;
  }
  return remapPath(path, mappings);
}

export function remapPathList(paths: string[], mappings: RenameMapping[]) {
  return Array.from(new Set(paths.map((path) => remapPath(path, mappings))));
}

export function remapPath(path: string, mappings: RenameMapping[]) {
  const normalizedPath = normalizePath(path);
  for (const mapping of mappings) {
    if (normalizedPath === mapping.from) {
      return mapping.to;
    }
    if (isSameOrChildPath(normalizedPath, mapping.from)) {
      return normalizePath(`${mapping.to}${normalizedPath.slice(mapping.from.length)}`);
    }
  }
  return normalizedPath;
}

export function renameCachedPaths(cache: ResourceChildrenCache, mappings: RenameMapping[]) {
  const renamedEntries = Object.entries(cache).map(([path, children]) => [
    remapPath(path, mappings),
    children.map((entry) => ({
      ...entry,
      path: remapPath(entry.path, mappings),
    })),
  ]);
  return Object.fromEntries(renamedEntries);
}

export function sortPathsByVisibleOrder(paths: string[], visiblePathOrder: string[]) {
  const orderMap = new Map(visiblePathOrder.map((path, index) => [path, index] as const));
  return [...paths].sort((left, right) => (orderMap.get(left) ?? 0) - (orderMap.get(right) ?? 0));
}

export function toFileUri(path: string) {
  return encodeURI(`file:///${normalizePath(path).replace(/\\/g, "/")}`);
}

export function isSamePathList(left: string[], right: string[]) {
  if (left.length !== right.length) {
    return false;
  }
  return left.every((path, index) => right[index] === path);
}

export function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

export function usePersistentNumber(key: string, fallback = DEFAULT_PANE_WIDTH) {
  const [value, setValue] = useState(() => {
    if (typeof window === "undefined") {
      return fallback;
    }
    const raw = window.localStorage.getItem(key);
    if (!raw) {
      return fallback;
    }
    const parsed = Number.parseInt(raw, 10);
    return Number.isFinite(parsed) ? clamp(parsed, MIN_PANE_WIDTH, MAX_PANE_WIDTH) : fallback;
  });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(key, String(value));
  }, [key, value]);

  return [value, setValue] as const;
}

export function usePersistentBoolean(key: string, fallback: boolean) {
  const [value, setValue] = useState(() => {
    if (typeof window === "undefined") {
      return fallback;
    }
    const raw = window.localStorage.getItem(key);
    return raw === null ? fallback : raw === "1";
  });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(key, value ? "1" : "0");
  }, [key, value]);

  return [value, setValue] as const;
}

export function normalizeChildren(entries: WorkspaceResourceEntry[]) {
  return entries.map((entry) => ({
    ...entry,
    path: normalizePath(entry.path),
  }));
}
