import {
  deleteLaunchTask,
  deleteWorkspace,
  listLaunchTasks,
  listWorkspaces,
  registerWorkspaceFolder,
  removeWorkspaceRegistration,
  upsertLaunchTask,
  upsertProject,
  upsertWorkspace,
} from "@/lib/command-client";

export const workspacesQueryKey = ["workspaces"] as const;
export const sidebarWorkspacesQueryKey = ["sidebar", "workspaces"] as const;

export {
  deleteLaunchTask,
  deleteWorkspace,
  listLaunchTasks,
  listWorkspaces,
  registerWorkspaceFolder,
  removeWorkspaceRegistration,
  upsertLaunchTask,
  upsertProject,
  upsertWorkspace,
};
