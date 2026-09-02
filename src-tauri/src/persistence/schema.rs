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

CREATE TABLE IF NOT EXISTS codex_auth_profiles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT,
  codex_home TEXT NOT NULL,
  auth_json TEXT NOT NULL,
  config_toml TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 0,
  last_quota_json TEXT,
  last_quota_checked_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS oss_accounts (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL UNIQUE,
  access_key_id_hint TEXT NOT NULL,
  credential_ref TEXT NOT NULL UNIQUE,
  last_probe_status TEXT NOT NULL DEFAULT 'unknown',
  last_probe_error TEXT,
  last_probe_at TEXT,
  is_disabled INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS oss_targets (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  bucket TEXT NOT NULL,
  region TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  prefix TEXT NOT NULL DEFAULT '',
  is_default INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(account_id, bucket, endpoint, prefix),
  FOREIGN KEY(account_id) REFERENCES oss_accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS oss_transfer_tasks (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  target_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  object_key TEXT NOT NULL,
  local_path TEXT NOT NULL,
  status TEXT NOT NULL,
  bytes_completed INTEGER NOT NULL DEFAULT 0,
  total_bytes INTEGER NOT NULL DEFAULT 0,
  upload_id TEXT,
  checkpoint_json TEXT,
  last_error_code TEXT,
  last_error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(account_id) REFERENCES oss_accounts(id) ON DELETE CASCADE,
  FOREIGN KEY(target_id) REFERENCES oss_targets(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS prompts (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
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

CREATE INDEX IF NOT EXISTS idx_codex_auth_profiles_active_updated
  ON codex_auth_profiles(is_active DESC, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_oss_targets_account_sort
  ON oss_targets(account_id, is_default DESC, sort_order ASC, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_oss_transfer_tasks_status_updated
  ON oss_transfer_tasks(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_prompts_sort_order
  ON prompts(sort_order ASC, created_at ASC, id ASC);
"#;
