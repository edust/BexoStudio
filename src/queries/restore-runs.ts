import {
  cancelRestoreAction,
  cancelRestoreRun,
  getRestoreCapabilities,
  getRestoreRunDetail,
  listRecentRestoreTargets,
  listRestoreRuns,
  openLogDirectory,
  restoreRecentTarget,
  startRestoreRun,
} from "@/lib/command-client";

export const restoreRunsQueryKey = ["restoreRuns"] as const;
export const restoreCapabilitiesQueryKey = ["restoreCapabilities"] as const;
export const recentRestoreTargetsQueryKey = ["recentRestoreTargets"] as const;

export const restoreRunDetailQueryKey = (runId?: string | null) =>
  [...restoreRunsQueryKey, runId ?? "none"] as const;

export {
  cancelRestoreAction,
  cancelRestoreRun,
  getRestoreCapabilities,
  getRestoreRunDetail,
  listRecentRestoreTargets,
  listRestoreRuns,
  openLogDirectory,
  restoreRecentTarget,
  startRestoreRun,
};
