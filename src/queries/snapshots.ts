import { createSnapshot, listSnapshots, previewRestore, startRestoreDryRun, updateSnapshot } from "@/lib/command-client";

export const snapshotsQueryKey = ["snapshots"] as const;

export const snapshotListQueryKey = (workspaceId?: string | null) =>
  [...snapshotsQueryKey, workspaceId ?? "all"] as const;

export const restorePreviewQueryKey = (snapshotId?: string | null, mode = "full") =>
  ["restorePreview", snapshotId ?? "none", mode] as const;

export { createSnapshot, listSnapshots, previewRestore, startRestoreDryRun, updateSnapshot };
