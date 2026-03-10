pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT,
  icon TEXT,
  color TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  is_default INTEGER NOT NULL DEFAULT 0,
  is_archived INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS codex_profiles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT,
  codex_home TEXT NOT NULL,
  startup_mode TEXT NOT NULL,
  resume_strategy TEXT NOT NULL,
  default_args_json TEXT NOT NULL DEFAULT '[]',
  is_default INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  path TEXT NOT NULL,
  platform TEXT NOT NULL,
  terminal_type TEXT NOT NULL,
  ide_type TEXT,
  codex_profile_id TEXT,
  open_terminal INTEGER NOT NULL DEFAULT 1,
  open_ide INTEGER NOT NULL DEFAULT 0,
  auto_resume_codex INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE RESTRICT,
  FOREIGN KEY(codex_profile_id) REFERENCES codex_profiles(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS launch_tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  name TEXT NOT NULL,
  task_type TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  command TEXT NOT NULL,
  args_json TEXT NOT NULL DEFAULT '[]',
  working_dir TEXT NOT NULL,
  timeout_ms INTEGER NOT NULL DEFAULT 30000,
  continue_on_failure INTEGER NOT NULL DEFAULT 0,
  retry_policy_json TEXT NOT NULL DEFAULT '{}',
  sort_order INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS snapshots (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}',
  last_restore_at TEXT,
  last_restore_status TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS restore_runs (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  snapshot_id TEXT,
  run_mode TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  error_summary TEXT,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY(snapshot_id) REFERENCES snapshots(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS restore_run_tasks (
  id TEXT PRIMARY KEY,
  restore_run_id TEXT NOT NULL,
  project_id TEXT,
  launch_task_id TEXT,
  status TEXT NOT NULL,
  attempt_count INTEGER NOT NULL DEFAULT 0,
  started_at TEXT,
  finished_at TEXT,
  error_message TEXT,
  FOREIGN KEY(restore_run_id) REFERENCES restore_runs(id) ON DELETE CASCADE,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE SET NULL,
  FOREIGN KEY(launch_task_id) REFERENCES launch_tasks(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_snapshots_workspace_name
  ON snapshots(workspace_id, name);

CREATE INDEX IF NOT EXISTS idx_snapshots_workspace_updated
  ON snapshots(workspace_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_restore_runs_started_at
  ON restore_runs(started_at DESC);

CREATE INDEX IF NOT EXISTS idx_restore_run_tasks_restore_run_id
  ON restore_run_tasks(restore_run_id);
"#;
