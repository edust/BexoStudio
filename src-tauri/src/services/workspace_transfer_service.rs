use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use crate::{
    domain::{
        validate_launch_task_command, validate_launch_task_working_dir,
        validate_workspace_transfer_document, ApplyWorkspaceListImportInput,
        ExportWorkspaceListInput, ExportWorkspaceListResult, PreviewWorkspaceListImportInput,
        WorkspaceImportAction, WorkspaceImportApplyResult, WorkspaceImportItemPreview,
        WorkspaceImportItemStatus, WorkspaceImportPreview, WorkspaceImportPreviewSummary,
        WorkspaceImportProjectPreview, WorkspaceImportProjectStatus, WorkspaceImportRelocation,
        WorkspaceRecord, WorkspaceTransferDocument, WorkspaceTransferProject,
        WORKSPACE_TRANSFER_FORMAT, WORKSPACE_TRANSFER_MAX_FILE_BYTES,
        WORKSPACE_TRANSFER_SCHEMA_VERSION,
    },
    error::{AppError, AppResult},
    persistence::{
        apply_workspace_import_plan, collect_workspace_transfer_workspaces, list_codex_profiles,
        list_workspaces, unique_workspace_name_for_transfer, Database,
        WorkspaceImportPersistenceOutcome, WorkspaceImportPlanEntry,
    },
};

const WORKSPACE_TRANSFER_FILE_IO_TIMEOUT: Duration = Duration::from_secs(10);
const WORKSPACE_TRANSFER_ANALYSIS_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(windows)]
const MOVE_FILE_REPLACE_EXISTING: u32 = 0x0000_0001;
#[cfg(windows)]
const MOVE_FILE_WRITE_THROUGH: u32 = 0x0000_0008;

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn MoveFileExW(existing_file_name: *const u16, new_file_name: *const u16, flags: u32) -> i32;
}

#[derive(Debug, Clone)]
pub struct WorkspaceTransferService {
    database: Database,
    import_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceImportApplyOutcome {
    pub result: WorkspaceImportApplyResult,
    pub pinned_workspace_ids_to_add: Vec<String>,
    pub pinned_workspace_ids_to_remove: Vec<String>,
}

#[derive(Debug, Clone)]
struct WorkspaceTransferExistingState {
    workspaces: Vec<WorkspaceRecord>,
    codex_profile_names: Vec<String>,
    custom_editor_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct WorkspaceImportAnalysis {
    preview: WorkspaceImportPreview,
    plans: Vec<WorkspaceImportPlanEntry>,
}

#[derive(Debug, Clone)]
struct ProjectAnalysis {
    project: WorkspaceTransferProject,
    preview: WorkspaceImportProjectPreview,
    path_key: String,
    existing_project_id: Option<String>,
}

impl WorkspaceTransferService {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            import_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn export_workspace_list(
        &self,
        input: ExportWorkspaceListInput,
        pinned_workspace_ids: Vec<String>,
    ) -> AppResult<ExportWorkspaceListResult> {
        let destination_path = validate_destination_path(&input.destination_path)?;
        let pinned_workspace_ids = pinned_workspace_ids.into_iter().collect::<HashSet<_>>();
        let workspaces = self
            .database
            .read("collect_workspace_transfer_export", move |connection| {
                collect_workspace_transfer_workspaces(connection, &pinned_workspace_ids)
            })
            .await?;
        let workspace_count = workspaces.len();
        let project_count = workspaces.iter().map(|item| item.projects.len()).sum();
        let launch_task_count = workspaces
            .iter()
            .flat_map(|workspace| workspace.projects.iter())
            .map(|project| project.launch_tasks.len())
            .sum();
        let contains_custom_editor_references = workspaces
            .iter()
            .flat_map(|workspace| workspace.projects.iter())
            .filter_map(|project| project.ide_type.as_deref())
            .any(|ide_type| !matches!(ide_type, "vscode" | "jetbrains"));
        let document = WorkspaceTransferDocument {
            format: WORKSPACE_TRANSFER_FORMAT.to_string(),
            schema_version: WORKSPACE_TRANSFER_SCHEMA_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at: Utc::now().to_rfc3339(),
            workspaces,
        };
        let mut bytes = serde_json::to_vec_pretty(&document).map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_JSON_INVALID",
                "生成工作区列表 JSON 失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        bytes.push(b'\n');
        if bytes.len() as u64 > WORKSPACE_TRANSFER_MAX_FILE_BYTES {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_FILE_TOO_LARGE",
                "导出的工作区列表超过 5 MiB 限制",
            )
            .with_detail("sizeBytes", bytes.len().to_string())
            .with_detail("limitBytes", WORKSPACE_TRANSFER_MAX_FILE_BYTES.to_string()));
        }

        let write_path = destination_path.clone();
        run_blocking_with_timeout(
            "export_workspace_list",
            WORKSPACE_TRANSFER_FILE_IO_TIMEOUT,
            "WORKSPACE_TRANSFER_WRITE_TIMEOUT",
            move |cancelled| write_workspace_transfer_file(&write_path, &bytes, &cancelled),
        )
        .await?;

        Ok(ExportWorkspaceListResult {
            destination_path: destination_path.display().to_string(),
            workspace_count,
            project_count,
            launch_task_count,
            warnings: contains_custom_editor_references
                .then(|| {
                    "导出文件包含自定义编辑器引用；目标机器没有相同编辑器 ID 时，导入会解除该项目的编辑器绑定。"
                        .to_string()
                })
                .into_iter()
                .collect(),
        })
    }

    pub async fn preview_workspace_list_import(
        &self,
        input: PreviewWorkspaceListImportInput,
        custom_editor_ids: Vec<String>,
    ) -> AppResult<WorkspaceImportPreview> {
        let source_path = validate_source_path(&input.source_path)?;
        let bytes = read_workspace_transfer_file(source_path.clone()).await?;
        let file_hash = sha256_hex(&bytes);
        let document = parse_workspace_transfer_document(&bytes)?;
        let existing_state = self.load_existing_state(custom_editor_ids).await?;
        let source_path_text = source_path.display().to_string();
        let relocations = input.relocations;
        let analysis = run_blocking_with_timeout(
            "preview_workspace_list_import",
            WORKSPACE_TRANSFER_ANALYSIS_TIMEOUT,
            "WORKSPACE_TRANSFER_READ_TIMEOUT",
            move |cancelled| {
                analyze_workspace_import(
                    source_path_text,
                    file_hash,
                    document,
                    relocations,
                    existing_state,
                    &cancelled,
                )
            },
        )
        .await?;
        Ok(analysis.preview)
    }

    pub async fn apply_workspace_list_import(
        &self,
        input: ApplyWorkspaceListImportInput,
        custom_editor_ids: Vec<String>,
    ) -> AppResult<WorkspaceImportApplyOutcome> {
        let _guard = self.import_lock.lock().await;
        validate_expected_hash(&input.expected_file_hash)?;
        let source_path = validate_source_path(&input.source_path)?;
        let bytes = read_workspace_transfer_file(source_path.clone()).await?;
        let file_hash = sha256_hex(&bytes);
        if file_hash != input.expected_file_hash.to_lowercase() {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_FILE_CHANGED",
                "导入文件在预检后已发生变化，请重新预检",
            )
            .with_detail("expectedHash", input.expected_file_hash.to_lowercase())
            .with_detail("actualHash", file_hash));
        }

        let document = parse_workspace_transfer_document(&bytes)?;
        let existing_state = self.load_existing_state(custom_editor_ids).await?;
        let source_path_text = source_path.display().to_string();
        let relocations = input.relocations;
        let mut analysis = run_blocking_with_timeout(
            "analyze_workspace_list_import_for_apply",
            WORKSPACE_TRANSFER_ANALYSIS_TIMEOUT,
            "WORKSPACE_TRANSFER_READ_TIMEOUT",
            move |cancelled| {
                analyze_workspace_import(
                    source_path_text,
                    file_hash,
                    document,
                    relocations,
                    existing_state,
                    &cancelled,
                )
            },
        )
        .await?;
        apply_selections(&mut analysis, input.selections)?;
        let plans = analysis.plans;
        let persistence_outcome = self
            .database
            .write("apply_workspace_list_import", move |connection| {
                apply_workspace_import_plan(connection, plans)
            })
            .await?;
        Ok(map_persistence_outcome(persistence_outcome))
    }

    async fn load_existing_state(
        &self,
        custom_editor_ids: Vec<String>,
    ) -> AppResult<WorkspaceTransferExistingState> {
        self.database
            .read(
                "load_workspace_transfer_existing_state",
                move |connection| {
                    Ok(WorkspaceTransferExistingState {
                        workspaces: list_workspaces(connection)?,
                        codex_profile_names: list_codex_profiles(connection)?
                            .into_iter()
                            .map(|profile| profile.name)
                            .collect(),
                        custom_editor_ids,
                    })
                },
            )
            .await
    }
}

fn map_persistence_outcome(
    outcome: WorkspaceImportPersistenceOutcome,
) -> WorkspaceImportApplyOutcome {
    WorkspaceImportApplyOutcome {
        result: outcome.result,
        pinned_workspace_ids_to_add: outcome.pinned_workspace_ids_to_add,
        pinned_workspace_ids_to_remove: outcome.pinned_workspace_ids_to_remove,
    }
}

fn parse_workspace_transfer_document(bytes: &[u8]) -> AppResult<WorkspaceTransferDocument> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let document = serde_json::from_slice::<WorkspaceTransferDocument>(bytes).map_err(|error| {
        AppError::new("WORKSPACE_TRANSFER_JSON_INVALID", "无法解析工作区列表 JSON")
            .with_detail("line", error.line().to_string())
            .with_detail("column", error.column().to_string())
            .with_detail("reason", error.to_string())
    })?;
    validate_workspace_transfer_document(document)
}

fn analyze_workspace_import(
    source_path: String,
    file_hash: String,
    document: WorkspaceTransferDocument,
    relocations: Vec<WorkspaceImportRelocation>,
    existing_state: WorkspaceTransferExistingState,
    cancelled: &AtomicBool,
) -> AppResult<WorkspaceImportAnalysis> {
    let relocation_map = validate_relocations(&document, relocations, cancelled)?;
    let existing_projects = build_existing_project_index(&existing_state.workspaces);
    let existing_profile_names = existing_state
        .codex_profile_names
        .iter()
        .map(|name| name.to_lowercase())
        .collect::<HashSet<_>>();
    let existing_custom_editor_ids = existing_state
        .custom_editor_ids
        .iter()
        .map(|id| id.to_lowercase())
        .collect::<HashSet<_>>();
    let mut analyzed_projects = Vec::with_capacity(document.workspaces.len());
    let mut imported_path_counts = HashMap::<String, usize>::new();

    for (workspace_index, workspace) in document.workspaces.iter().enumerate() {
        check_cancelled(cancelled)?;
        let mut projects = Vec::with_capacity(workspace.projects.len());
        for (project_index, project) in workspace.projects.iter().enumerate() {
            check_cancelled(cancelled)?;
            let relocation = relocation_map.get(&(workspace_index, project_index));
            let mut resolved_project = project.clone();
            let source_project_path = resolved_project.path.clone();
            if let Some(relocated_path) = relocation {
                remap_project_paths(&mut resolved_project, &source_project_path, relocated_path);
            }
            let resolved_path = resolved_project.path.clone();
            let path_key = path_comparison_key(Path::new(&resolved_path));
            *imported_path_counts.entry(path_key.clone()).or_default() += 1;
            let path_is_directory = fs::metadata(&resolved_path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false);
            let mut status = if path_is_directory {
                WorkspaceImportProjectStatus::Ready
            } else {
                WorkspaceImportProjectStatus::Missing
            };
            let mut message = if path_is_directory {
                None
            } else {
                Some("目录不存在，请重新定位后再导入。".to_string())
            };
            let mut existing_workspace_id = None;
            let mut existing_workspace_name = None;
            let mut existing_project_id = None;

            if path_is_directory {
                if let Some(matches) = existing_projects.get(&path_key) {
                    if matches.len() == 1 {
                        let matched = &matches[0];
                        status = WorkspaceImportProjectStatus::Existing;
                        existing_workspace_id = Some(matched.workspace_id.clone());
                        existing_workspace_name = Some(matched.workspace_name.clone());
                        existing_project_id = Some(matched.project_id.clone());
                        message = Some("此目录已在 Bexo Studio 中注册。".to_string());
                    } else {
                        status = WorkspaceImportProjectStatus::Invalid;
                        message =
                            Some("同一路径对应多个现有项目，无法安全决定更新目标。".to_string());
                    }
                }
            }

            if matches!(
                status,
                WorkspaceImportProjectStatus::Ready | WorkspaceImportProjectStatus::Existing
            ) {
                if let Err(error) =
                    validate_resolved_project_runtime_paths(&resolved_project, cancelled)
                {
                    status = WorkspaceImportProjectStatus::Invalid;
                    message = Some(format!("启动配置路径无效：{}", error.message));
                }
            }

            projects.push(ProjectAnalysis {
                project: resolved_project,
                preview: WorkspaceImportProjectPreview {
                    project_index,
                    name: project.name.clone(),
                    source_path: source_project_path,
                    resolved_path,
                    status,
                    existing_workspace_id,
                    existing_workspace_name,
                    message,
                },
                path_key,
                existing_project_id,
            });
        }
        analyzed_projects.push(projects);
    }

    for projects in &mut analyzed_projects {
        for project in projects {
            if imported_path_counts
                .get(&project.path_key)
                .copied()
                .unwrap_or_default()
                > 1
            {
                project.preview.status = WorkspaceImportProjectStatus::Invalid;
                project.preview.message = Some("导入文件内存在重复项目路径。".to_string());
                project.existing_project_id = None;
            }
        }
    }

    let mut occupied_workspace_names = existing_state
        .workspaces
        .iter()
        .map(|workspace| workspace.name.to_lowercase())
        .collect::<HashSet<_>>();
    let mut preview_items = Vec::with_capacity(document.workspaces.len());
    let mut plans = Vec::with_capacity(document.workspaces.len());
    let mut summary = WorkspaceImportPreviewSummary {
        total_workspace_count: document.workspaces.len(),
        ..WorkspaceImportPreviewSummary::default()
    };
    let mut global_warnings = Vec::new();
    let mut warned_profiles = HashSet::new();
    let mut warned_editors = HashSet::new();

    for (workspace_index, (workspace, mut projects)) in document
        .workspaces
        .into_iter()
        .zip(analyzed_projects.into_iter())
        .enumerate()
    {
        check_cancelled(cancelled)?;
        let mut warnings = Vec::new();
        if workspace.projects.is_empty() {
            warnings.push("工作区不包含项目目录，不能创建或更新。".to_string());
        }
        for project in &mut projects {
            match project.preview.status {
                WorkspaceImportProjectStatus::Missing => summary.missing_project_count += 1,
                WorkspaceImportProjectStatus::Invalid => summary.invalid_project_count += 1,
                _ => {}
            }
            if let Some(profile_name) = project.project.codex_profile_name.as_ref() {
                if !existing_profile_names.contains(&profile_name.to_lowercase()) {
                    warnings.push(format!(
                        "未找到 Codex Profile“{profile_name}”，导入时将解除绑定。"
                    ));
                    if warned_profiles.insert(profile_name.to_lowercase()) {
                        global_warnings.push(format!(
                            "未找到 Codex Profile“{profile_name}”，相关项目将解除绑定。"
                        ));
                    }
                }
            }
            if let Some(ide_type) = project.project.ide_type.clone() {
                if !matches!(ide_type.as_str(), "vscode" | "jetbrains")
                    && !existing_custom_editor_ids.contains(&ide_type.to_lowercase())
                {
                    warnings.push(format!(
                        "未找到自定义编辑器“{ide_type}”，导入时将解除编辑器绑定。"
                    ));
                    if warned_editors.insert(ide_type.to_lowercase()) {
                        global_warnings.push(format!(
                            "未找到自定义编辑器“{ide_type}”，相关项目将解除编辑器绑定。"
                        ));
                    }
                    project.project.ide_type = None;
                }
            }
        }

        let blocked = workspace.projects.is_empty()
            || projects.iter().any(|project| {
                matches!(
                    project.preview.status,
                    WorkspaceImportProjectStatus::Missing | WorkspaceImportProjectStatus::Invalid
                )
            });
        let matched_workspace_ids = projects
            .iter()
            .filter_map(|project| project.preview.existing_workspace_id.clone())
            .collect::<HashSet<_>>();
        let conflicting_workspaces = matched_workspace_ids.len() > 1;
        if conflicting_workspaces {
            warnings.push("该文件中的项目分别属于多个现有工作区，不能合并更新。".to_string());
        }
        let blocked = blocked || conflicting_workspaces;
        let existing_workspace_id = if matched_workspace_ids.len() == 1 {
            matched_workspace_ids.into_iter().next()
        } else {
            None
        };
        let existing_workspace_name = existing_workspace_id.as_ref().and_then(|workspace_id| {
            existing_state
                .workspaces
                .iter()
                .find(|workspace| &workspace.id == workspace_id)
                .map(|workspace| workspace.name.clone())
        });
        let (status, recommended_action, can_create, can_update, suggested_name) = if blocked {
            summary.blocked_workspace_count += 1;
            (
                WorkspaceImportItemStatus::Blocked,
                WorkspaceImportAction::Skip,
                false,
                false,
                workspace.name.clone(),
            )
        } else if existing_workspace_id.is_some() {
            summary.existing_workspace_count += 1;
            (
                WorkspaceImportItemStatus::Existing,
                WorkspaceImportAction::Skip,
                false,
                true,
                existing_workspace_name
                    .clone()
                    .unwrap_or_else(|| workspace.name.clone()),
            )
        } else {
            summary.ready_workspace_count += 1;
            let suggested_name =
                unique_workspace_name_for_transfer(&workspace.name, &occupied_workspace_names);
            occupied_workspace_names.insert(suggested_name.to_lowercase());
            (
                WorkspaceImportItemStatus::Ready,
                WorkspaceImportAction::Create,
                true,
                false,
                suggested_name,
            )
        };
        let existing_project_ids = projects
            .iter()
            .map(|project| project.existing_project_id.clone())
            .collect::<Vec<_>>();
        let resolved_projects = projects
            .iter()
            .map(|project| project.project.clone())
            .collect::<Vec<_>>();
        let project_previews = projects
            .into_iter()
            .map(|project| project.preview)
            .collect::<Vec<_>>();
        let mut resolved_workspace = workspace;
        resolved_workspace.projects = resolved_projects;
        let preview_name = resolved_workspace.name.clone();
        plans.push(WorkspaceImportPlanEntry {
            workspace_index,
            action: recommended_action,
            workspace: resolved_workspace,
            existing_workspace_id: existing_workspace_id.clone(),
            existing_project_ids,
        });
        preview_items.push(WorkspaceImportItemPreview {
            workspace_index,
            name: preview_name,
            suggested_name,
            status,
            recommended_action,
            can_create,
            can_update,
            existing_workspace_id,
            existing_workspace_name,
            projects: project_previews,
            warnings,
        });
    }

    Ok(WorkspaceImportAnalysis {
        preview: WorkspaceImportPreview {
            source_path,
            file_hash,
            schema_version: WORKSPACE_TRANSFER_SCHEMA_VERSION,
            summary,
            items: preview_items,
            warnings: global_warnings,
        },
        plans,
    })
}

fn apply_selections(
    analysis: &mut WorkspaceImportAnalysis,
    selections: Vec<crate::domain::WorkspaceImportSelection>,
) -> AppResult<()> {
    if selections.len() != analysis.plans.len() {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_SELECTION_INVALID",
            "导入选择数量与预检结果不一致，请重新预检",
        )
        .with_detail("selectionCount", selections.len().to_string())
        .with_detail("workspaceCount", analysis.plans.len().to_string()));
    }
    let mut selection_map = HashMap::with_capacity(selections.len());
    for selection in selections {
        if selection.workspace_index >= analysis.plans.len() {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_SELECTION_INVALID",
                "导入选择包含无效的工作区序号",
            )
            .with_detail("workspaceIndex", selection.workspace_index.to_string()));
        }
        if selection_map
            .insert(selection.workspace_index, selection.action)
            .is_some()
        {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_SELECTION_INVALID",
                "导入选择包含重复的工作区序号",
            )
            .with_detail("workspaceIndex", selection.workspace_index.to_string()));
        }
    }

    for (workspace_index, plan) in analysis.plans.iter_mut().enumerate() {
        let action = selection_map
            .get(&workspace_index)
            .copied()
            .ok_or_else(|| {
                AppError::new(
                    "WORKSPACE_TRANSFER_SELECTION_INVALID",
                    "导入选择缺少工作区动作",
                )
                .with_detail("workspaceIndex", workspace_index.to_string())
            })?;
        let preview = &analysis.preview.items[workspace_index];
        match action {
            WorkspaceImportAction::Create if !preview.can_create => {
                return Err(selection_not_allowed(workspace_index, "新建"));
            }
            WorkspaceImportAction::Update if !preview.can_update => {
                return Err(selection_not_allowed(workspace_index, "更新"));
            }
            _ => plan.action = action,
        }
    }
    Ok(())
}

fn selection_not_allowed(workspace_index: usize, action: &str) -> AppError {
    AppError::new(
        "WORKSPACE_TRANSFER_SELECTION_INVALID",
        format!("当前预检状态不允许{action}此工作区"),
    )
    .with_detail("workspaceIndex", workspace_index.to_string())
}

#[derive(Debug, Clone)]
struct ExistingProjectMatch {
    workspace_id: String,
    workspace_name: String,
    project_id: String,
}

fn build_existing_project_index(
    workspaces: &[WorkspaceRecord],
) -> HashMap<String, Vec<ExistingProjectMatch>> {
    let mut index = HashMap::<String, Vec<ExistingProjectMatch>>::new();
    for workspace in workspaces {
        for project in &workspace.projects {
            index
                .entry(path_comparison_key(Path::new(&project.path)))
                .or_default()
                .push(ExistingProjectMatch {
                    workspace_id: workspace.id.clone(),
                    workspace_name: workspace.name.clone(),
                    project_id: project.id.clone(),
                });
        }
    }
    index
}

fn validate_relocations(
    document: &WorkspaceTransferDocument,
    relocations: Vec<WorkspaceImportRelocation>,
    cancelled: &AtomicBool,
) -> AppResult<HashMap<(usize, usize), String>> {
    let project_count = document
        .workspaces
        .iter()
        .map(|workspace| workspace.projects.len())
        .sum::<usize>();
    if relocations.len() > project_count {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_SELECTION_INVALID",
            "重新定位数量超过导入文件中的项目数量",
        )
        .with_detail("relocationCount", relocations.len().to_string())
        .with_detail("projectCount", project_count.to_string()));
    }
    let mut normalized = HashMap::with_capacity(relocations.len());
    for relocation in relocations {
        check_cancelled(cancelled)?;
        let Some(workspace) = document.workspaces.get(relocation.workspace_index) else {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_SELECTION_INVALID",
                "重新定位包含无效的工作区序号",
            )
            .with_detail("workspaceIndex", relocation.workspace_index.to_string()));
        };
        if relocation.project_index >= workspace.projects.len() {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_SELECTION_INVALID",
                "重新定位包含无效的项目序号",
            )
            .with_detail("workspaceIndex", relocation.workspace_index.to_string())
            .with_detail("projectIndex", relocation.project_index.to_string()));
        }
        let path = relocation.path.trim();
        if !Path::new(path).is_absolute() {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_PATH_INVALID",
                "重新定位的项目目录必须是绝对路径",
            )
            .with_detail("path", path.to_string()));
        }
        let metadata = fs::metadata(path).map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_PATH_MISSING",
                "重新定位的项目目录不存在",
            )
            .with_detail("path", path.to_string())
            .with_detail("reason", error.to_string())
        })?;
        if !metadata.is_dir() {
            return Err(
                AppError::new("WORKSPACE_TRANSFER_PATH_INVALID", "重新定位目标必须是目录")
                    .with_detail("path", path.to_string()),
            );
        }
        if normalized
            .insert(
                (relocation.workspace_index, relocation.project_index),
                path.to_string(),
            )
            .is_some()
        {
            return Err(AppError::new(
                "WORKSPACE_TRANSFER_SELECTION_INVALID",
                "同一项目包含重复的重新定位设置",
            )
            .with_detail("workspaceIndex", relocation.workspace_index.to_string())
            .with_detail("projectIndex", relocation.project_index.to_string()));
        }
    }
    Ok(normalized)
}

fn remap_project_paths(project: &mut WorkspaceTransferProject, old_root: &str, new_root: &str) {
    project.path = new_root.to_string();
    for task in &mut project.launch_tasks {
        if !task.working_dir.trim().is_empty() {
            task.working_dir = remap_child_path(&task.working_dir, old_root, new_root)
                .unwrap_or_else(|| task.working_dir.clone());
        }
        if task.task_type == "open_path" {
            task.command = remap_child_path(&task.command, old_root, new_root)
                .unwrap_or_else(|| task.command.clone());
        }
    }
}

fn remap_child_path(candidate: &str, old_root: &str, new_root: &str) -> Option<String> {
    let candidate_components = comparable_components(Path::new(candidate));
    let root_components = comparable_components(Path::new(old_root));
    if candidate_components.len() < root_components.len()
        || !candidate_components
            .iter()
            .zip(root_components.iter())
            .all(|(candidate, root)| path_component_eq(candidate, root))
    {
        return None;
    }
    let mut remapped = PathBuf::from(new_root);
    for component in Path::new(candidate)
        .components()
        .skip(root_components.len())
    {
        remapped.push(component.as_os_str());
    }
    Some(remapped.display().to_string())
}

fn comparable_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::CurDir => None,
            _ => Some(component.as_os_str().to_string_lossy().to_string()),
        })
        .collect()
}

#[cfg(windows)]
fn path_component_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(not(windows))]
fn path_component_eq(left: &str, right: &str) -> bool {
    left == right
}

fn validate_resolved_project_runtime_paths(
    project: &WorkspaceTransferProject,
    cancelled: &AtomicBool,
) -> AppResult<()> {
    for task in &project.launch_tasks {
        check_cancelled(cancelled)?;
        validate_launch_task_working_dir(Some(task.working_dir.clone()))?;
        validate_launch_task_command(&task.task_type, &task.command)?;
    }
    Ok(())
}

fn path_comparison_key(path: &Path) -> String {
    let normalized = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut value = normalized.display().to_string();
    if let Some(stripped) = value.strip_prefix(r"\\?\") {
        value = stripped.to_string();
    }
    while value.len() > 3 && (value.ends_with('\\') || value.ends_with('/')) {
        value.pop();
    }
    #[cfg(windows)]
    {
        value.replace('/', "\\").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        value
    }
}

fn validate_source_path(value: &str) -> AppResult<PathBuf> {
    validate_absolute_path(value, "WORKSPACE_TRANSFER_PATH_INVALID")
}

fn validate_destination_path(value: &str) -> AppResult<PathBuf> {
    let path = validate_absolute_path(value, "WORKSPACE_TRANSFER_PATH_INVALID")?;
    path.parent().ok_or_else(|| {
        AppError::new("WORKSPACE_TRANSFER_PATH_INVALID", "导出文件路径缺少父目录")
    })?;
    if path.file_name().is_none() {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_PATH_INVALID",
            "导出文件名不能为空",
        ));
    }
    Ok(path)
}

fn validate_absolute_path(value: &str, error_code: &str) -> AppResult<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(error_code, "文件路径不能为空"));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(AppError::new(error_code, "文件路径必须是绝对路径")
            .with_detail("path", trimmed.to_string()));
    }
    Ok(path)
}

fn validate_expected_hash(value: &str) -> AppResult<()> {
    let trimmed = value.trim();
    if trimmed.len() != 64
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_SELECTION_INVALID",
            "预检文件哈希无效，请重新预检",
        ));
    }
    Ok(())
}

async fn read_workspace_transfer_file(path: PathBuf) -> AppResult<Vec<u8>> {
    run_blocking_with_timeout(
        "read_workspace_transfer_file",
        WORKSPACE_TRANSFER_FILE_IO_TIMEOUT,
        "WORKSPACE_TRANSFER_READ_TIMEOUT",
        move |cancelled| {
            check_cancelled(&cancelled)?;
            let metadata = fs::metadata(&path).map_err(|error| {
                AppError::new(
                    "WORKSPACE_TRANSFER_PATH_INVALID",
                    "导入文件不存在或无法读取",
                )
                .with_detail("path", path.display().to_string())
                .with_detail("reason", error.to_string())
            })?;
            if !metadata.is_file() {
                return Err(AppError::new(
                    "WORKSPACE_TRANSFER_PATH_INVALID",
                    "导入路径必须指向文件",
                )
                .with_detail("path", path.display().to_string()));
            }
            if metadata.len() > WORKSPACE_TRANSFER_MAX_FILE_BYTES {
                return Err(file_too_large_error(metadata.len()));
            }
            let mut file = File::open(&path).map_err(|error| {
                AppError::new("WORKSPACE_TRANSFER_PATH_INVALID", "打开工作区列表文件失败")
                    .with_detail("path", path.display().to_string())
                    .with_detail("reason", error.to_string())
            })?;
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                check_cancelled(&cancelled)?;
                let read_count = file.read(&mut buffer).map_err(|error| {
                    AppError::new("WORKSPACE_TRANSFER_PATH_INVALID", "读取工作区列表文件失败")
                        .with_detail("path", path.display().to_string())
                        .with_detail("reason", error.to_string())
                })?;
                if read_count == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read_count]);
                if bytes.len() as u64 > WORKSPACE_TRANSFER_MAX_FILE_BYTES {
                    return Err(file_too_large_error(bytes.len() as u64));
                }
            }
            Ok(bytes)
        },
    )
    .await
}

fn write_workspace_transfer_file(
    destination_path: &Path,
    bytes: &[u8],
    cancelled: &AtomicBool,
) -> AppResult<()> {
    check_cancelled(cancelled)?;
    let parent = destination_path.parent().ok_or_else(|| {
        AppError::new("WORKSPACE_TRANSFER_PATH_INVALID", "导出文件路径缺少父目录")
    })?;
    let parent_metadata = fs::metadata(parent).map_err(|error| {
        AppError::new(
            "WORKSPACE_TRANSFER_PATH_INVALID",
            "导出文件父目录不存在或无法访问",
        )
        .with_detail("path", parent.display().to_string())
        .with_detail("reason", error.to_string())
    })?;
    if !parent_metadata.is_dir() {
        return Err(
            AppError::new("WORKSPACE_TRANSFER_PATH_INVALID", "导出文件父路径不是目录")
                .with_detail("path", parent.display().to_string()),
        );
    }
    if fs::metadata(destination_path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
    {
        return Err(
            AppError::new("WORKSPACE_TRANSFER_PATH_INVALID", "导出路径不能指向目录")
                .with_detail("path", destination_path.display().to_string()),
        );
    }
    let file_name = destination_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bexo-workspaces.json");
    let temp_path = parent.join(format!(".{file_name}.tmp.{}", Uuid::new_v4()));
    let write_result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| {
                AppError::new("WORKSPACE_TRANSFER_WRITE_FAILED", "创建导出临时文件失败")
                    .with_detail("path", temp_path.display().to_string())
                    .with_detail("reason", error.to_string())
            })?;
        for chunk in bytes.chunks(64 * 1024) {
            check_cancelled(cancelled)?;
            file.write_all(chunk).map_err(|error| {
                AppError::new(
                    "WORKSPACE_TRANSFER_WRITE_FAILED",
                    "写入工作区列表临时文件失败",
                )
                .with_detail("path", temp_path.display().to_string())
                .with_detail("reason", error.to_string())
            })?;
        }
        file.flush().map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_WRITE_FAILED",
                "刷新工作区列表临时文件失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        file.sync_all().map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_WRITE_FAILED",
                "持久化工作区列表临时文件失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        drop(file);
        check_cancelled(cancelled)?;
        replace_file_from_temp(&temp_path, destination_path).map_err(|error| {
            AppError::new("WORKSPACE_TRANSFER_WRITE_FAILED", "原子替换导出文件失败")
                .with_detail("path", destination_path.display().to_string())
                .with_detail("reason", error.to_string())
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

#[cfg(windows)]
fn replace_file_from_temp(temp_path: &Path, destination_path: &Path) -> std::io::Result<()> {
    let temp_wide = temp_path
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let destination_wide = destination_path
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVE_FILE_REPLACE_EXISTING | MOVE_FILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file_from_temp(temp_path: &Path, destination_path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, destination_path)
}

async fn run_blocking_with_timeout<T, F>(
    operation_name: &'static str,
    timeout: Duration,
    timeout_code: &'static str,
    operation: F,
) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(Arc<AtomicBool>) -> AppResult<T> + Send + 'static,
{
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let mut handle = tauri::async_runtime::spawn_blocking(move || operation(worker_cancelled));
    match tokio::time::timeout(timeout, &mut handle).await {
        Ok(joined) => joined.map_err(|error| {
            AppError::new(timeout_code, "工作区列表本地操作异常终止")
                .with_detail("operation", operation_name)
                .with_detail("reason", error.to_string())
        })?,
        Err(_) => {
            cancelled.store(true, Ordering::Release);
            match handle.await {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) if error.code != "WORKSPACE_TRANSFER_CANCELLED" => Err(error),
                Ok(Err(_)) => Err(AppError::new(timeout_code, "工作区列表本地操作超时")
                    .with_detail("operation", operation_name)
                    .retryable(true)),
                Err(error) => Err(
                    AppError::new(timeout_code, "工作区列表本地操作超时且回收失败")
                        .with_detail("operation", operation_name)
                        .with_detail("reason", error.to_string()),
                ),
            }
        }
    }
}

fn check_cancelled(cancelled: &AtomicBool) -> AppResult<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_CANCELLED",
            "工作区列表操作已取消",
        ));
    }
    Ok(())
}

fn file_too_large_error(size: u64) -> AppError {
    AppError::new(
        "WORKSPACE_TRANSFER_FILE_TOO_LARGE",
        "工作区列表文件超过 5 MiB 限制",
    )
    .with_detail("sizeBytes", size.to_string())
    .with_detail("limitBytes", WORKSPACE_TRANSFER_MAX_FILE_BYTES.to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        WorkspaceTransferLaunchTask, WorkspaceTransferWorkspace, WORKSPACE_TRANSFER_MAX_FILE_BYTES,
    };

    fn sample_document(path: String) -> WorkspaceTransferDocument {
        WorkspaceTransferDocument {
            format: WORKSPACE_TRANSFER_FORMAT.to_string(),
            schema_version: WORKSPACE_TRANSFER_SCHEMA_VERSION,
            app_version: "0.1.16".to_string(),
            exported_at: "2026-09-02T00:00:00Z".to_string(),
            workspaces: vec![WorkspaceTransferWorkspace {
                name: "Imported".to_string(),
                description: None,
                icon: None,
                color: None,
                sort_order: 0,
                is_default: true,
                is_archived: false,
                pinned: false,
                projects: vec![WorkspaceTransferProject {
                    name: "Imported".to_string(),
                    path,
                    platform: "windows".to_string(),
                    terminal_type: "windows_terminal".to_string(),
                    ide_type: Some("vscode".to_string()),
                    codex_profile_name: Some("Missing Profile".to_string()),
                    open_terminal: true,
                    open_ide: false,
                    auto_resume_codex: false,
                    sort_order: 0,
                    launch_tasks: Vec::<WorkspaceTransferLaunchTask>::new(),
                }],
            }],
        }
    }

    #[test]
    fn preview_marks_missing_directory_as_blocked() {
        let missing_path = std::env::temp_dir()
            .join(format!("bexo-missing-{}", Uuid::new_v4()))
            .display()
            .to_string();
        let analysis = analyze_workspace_import(
            "import.json".to_string(),
            "0".repeat(64),
            sample_document(missing_path),
            Vec::new(),
            WorkspaceTransferExistingState {
                workspaces: Vec::new(),
                codex_profile_names: Vec::new(),
                custom_editor_ids: Vec::new(),
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            analysis.preview.items[0].status,
            WorkspaceImportItemStatus::Blocked
        );
        assert_eq!(analysis.preview.summary.missing_project_count, 1);
        assert!(!analysis.preview.items[0].can_create);
    }

    #[test]
    fn relocation_remaps_project_child_paths() {
        let old_root = if cfg!(windows) {
            r"C:\old\project"
        } else {
            "/old/project"
        };
        let new_root = if cfg!(windows) {
            r"D:\new\project"
        } else {
            "/new/project"
        };
        let child = if cfg!(windows) {
            r"C:\old\project\web"
        } else {
            "/old/project/web"
        };
        assert_eq!(
            remap_child_path(child, old_root, new_root),
            Some(PathBuf::from(new_root).join("web").display().to_string())
        );
    }

    #[test]
    fn unknown_custom_editor_is_removed_with_a_warning() {
        let directory = std::env::temp_dir().join(format!("bexo-custom-editor-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create custom editor test directory");
        let mut document = sample_document(directory.display().to_string());
        document.workspaces[0].projects[0].ide_type = Some("editor-from-other-machine".to_string());

        let analysis = analyze_workspace_import(
            "import.json".to_string(),
            "0".repeat(64),
            document,
            Vec::new(),
            WorkspaceTransferExistingState {
                workspaces: Vec::new(),
                codex_profile_names: Vec::new(),
                custom_editor_ids: Vec::new(),
            },
            &AtomicBool::new(false),
        )
        .unwrap();

        assert!(analysis
            .preview
            .warnings
            .iter()
            .any(|warning| warning.contains("未找到自定义编辑器“editor-from-other-machine”")));
        assert!(analysis.plans[0].workspace.projects[0].ide_type.is_none());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn bounded_reader_rejects_content_over_limit() {
        assert_eq!(
            file_too_large_error(WORKSPACE_TRANSFER_MAX_FILE_BYTES + 1).code,
            "WORKSPACE_TRANSFER_FILE_TOO_LARGE"
        );
    }
}
