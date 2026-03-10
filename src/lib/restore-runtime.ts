import type { RestoreActionPlan, RestoreProjectPlan, RestoreRunEvent, RestoreRunProjectRecord } from "@/types/backend";

export type RestoreRuntimeOverlayState = {
  actions: Record<string, RestoreActionPlan>;
  projectTaskIds: Record<string, string>;
};

export function createRestoreRuntimeOverlayState(): RestoreRuntimeOverlayState {
  return {
    actions: {},
    projectTaskIds: {},
  };
}

export function restoreActionOverlayKey(projectTaskId: string, actionId: string) {
  return `${projectTaskId}:${actionId}`;
}

export function applyRestoreRunEventToOverlay(
  current: RestoreRuntimeOverlayState,
  event: RestoreRunEvent,
): RestoreRuntimeOverlayState {
  const next: RestoreRuntimeOverlayState = {
    actions: { ...current.actions },
    projectTaskIds: { ...current.projectTaskIds },
  };

  if (event.projectTaskId && event.projectId) {
    next.projectTaskIds[event.projectId] = event.projectTaskId;
  }

  if (event.project) {
    if (event.project.projectId) {
      next.projectTaskIds[event.project.projectId] = event.project.id;
    }
    for (const action of event.project.actions) {
      next.actions[restoreActionOverlayKey(event.project.id, action.id)] = action;
    }
  }

  if (event.action && event.projectTaskId) {
    next.actions[restoreActionOverlayKey(event.projectTaskId, event.action.id)] = event.action;
  }

  return next;
}

export function overlayRestoreRunTasks(
  tasks: RestoreRunProjectRecord[],
  overlay: RestoreRuntimeOverlayState,
): RestoreRunProjectRecord[] {
  return tasks.map((task) => ({
    ...task,
    actions: task.actions.map((action) => overlay.actions[restoreActionOverlayKey(task.id, action.id)] ?? action),
  }));
}

export function overlayRestorePreviewProjects(
  projects: RestoreProjectPlan[],
  overlay: RestoreRuntimeOverlayState,
): RestoreProjectPlan[] {
  return projects.map((project) => {
    const projectTaskId = overlay.projectTaskIds[project.projectId];
    if (!projectTaskId) {
      return project;
    }

    return {
      ...project,
      actions: project.actions.map((action) => overlay.actions[restoreActionOverlayKey(projectTaskId, action.id)] ?? action),
    };
  });
}

export function canCancelRestoreAction(action: RestoreActionPlan, runStatus?: string | null) {
  if (!["running", "cancel_requested"].includes(runStatus ?? "")) {
    return false;
  }

  return ["planned", "running", "cancel_requested"].includes(action.status);
}
