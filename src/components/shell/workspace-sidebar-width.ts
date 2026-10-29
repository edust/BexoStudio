export const DEFAULT_WORKSPACE_SIDEBAR_WIDTH = 320;
export const MIN_WORKSPACE_SIDEBAR_WIDTH = 260;
export const MAX_WORKSPACE_SIDEBAR_WIDTH = 520;
export const WORKSPACE_SIDEBAR_WIDTH_KEY = "bexo.workbench.sidebarWidth";

export function workspaceSidebarWidth(requested: number, viewportWidth: number) {
  // Rail + resource pane minimum + command pane minimum + splitters.
  const maximum = Math.max(MIN_WORKSPACE_SIDEBAR_WIDTH, Math.min(MAX_WORKSPACE_SIDEBAR_WIDTH, viewportWidth - 672));
  return Math.min(maximum, Math.max(MIN_WORKSPACE_SIDEBAR_WIDTH,
    Number.isFinite(requested) ? requested : DEFAULT_WORKSPACE_SIDEBAR_WIDTH));
}

export function splitWorkspacePath(path: string) {
  const boundary = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return { prefix: path.slice(0, boundary + 1), leaf: path.slice(boundary + 1) };
}
