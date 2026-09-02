import {
  ArrowLeftOutlined,
  CopyOutlined,
  DeleteOutlined,
  DownloadOutlined,
  ExperimentOutlined,
  FileOutlined,
  FolderAddOutlined,
  FolderOpenOutlined,
  LinkOutlined,
  MoreOutlined,
  PlusOutlined,
  ReloadOutlined,
  UploadOutlined,
} from "@ant-design/icons";
import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { Button, Dropdown, Empty, Input, Modal, Spin, Tag, Tooltip, Typography } from "antd";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { toast } from "sonner";

import { OssTargetDialog } from "@/features/oss/oss-target-dialog";
import { cn } from "@/lib/cn";
import { copyTextToClipboard, getClipboardErrorMessage } from "@/lib/clipboard";
import {
  copyOssObject,
  createOssFolder,
  deleteOssObjects,
  deleteOssTarget,
  getErrorSummary,
  getOssObjectDownloadUrl,
  hasDesktopRuntime,
  listOssObjects,
  listOssTargets,
  startOssDownload,
  startOssUpload,
  testOssTarget,
  upsertOssTarget,
} from "@/lib/command-client";
import { ossAccountsQueryKey, ossObjectsQueryKey, ossTargetsQueryKey, ossTransferTasksQueryKey } from "@/queries/oss";
import { useShellStore } from "@/stores/shell-store";
import type {
  OssObjectEntry,
  OssObjectPage,
  OssTargetRecord,
  OssTransferTaskView,
  UpsertOssTargetPayload,
} from "@/types/backend";

import { OssFolderDialog } from "./oss-folder-dialog";
import { OssCopyDialog } from "./oss-copy-dialog";
import { OssTransferQueue } from "./oss-transfer-queue";
import { buildOssCopyTargetKey } from "./oss-copy-model";
import { enqueueOssUploadPaths, type OssUploadStartInput } from "./oss-upload-model";

const ROW_HEIGHT = 46;

export default function OssPage() {
  const desktop = hasDesktopRuntime();
  const accountId = useShellStore((state) => state.selectedOssAccountId);
  const queryClient = useQueryClient();
  const [targetId, setTargetId] = useState<string | null>(null);
  const [prefix, setPrefix] = useState("");
  const [continuationToken, setContinuationToken] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [targetDialogOpen, setTargetDialogOpen] = useState(false);
  const [folderDialogOpen, setFolderDialogOpen] = useState(false);
  const [editingTarget, setEditingTarget] = useState<OssTargetRecord | null>(null);
  const [renameObject, setRenameObject] = useState<OssObjectEntry | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [copySourceObject, setCopySourceObject] = useState<OssObjectEntry | null>(null);
  const [copyDraft, setCopyDraft] = useState("");
  const [uploadBatchPending, setUploadBatchPending] = useState(false);
  const completedUploadIds = useRef(new Set<string>());

  const targetsQuery = useQuery({
    queryKey: ossTargetsQueryKey(accountId),
    queryFn: () => listOssTargets(accountId as string),
    enabled: desktop && Boolean(accountId),
    staleTime: 20_000,
  });
  const target = targetsQuery.data?.find((item) => item.id === targetId) ?? null;
  const objectsQuery = useQuery({
    queryKey: ossObjectsQueryKey(targetId, prefix, continuationToken),
    queryFn: () =>
      listOssObjects({
        targetId: targetId as string,
        prefix,
        continuationToken,
        pageSize: 200,
      }),
    enabled: desktop && Boolean(targetId && target),
    staleTime: 10_000,
  });
  const saveTarget = useMutation({
    mutationFn: upsertOssTarget,
    onSuccess: async (saved) => {
      await queryClient.invalidateQueries({ queryKey: ossTargetsQueryKey(accountId) });
      await queryClient.invalidateQueries({ queryKey: ["oss", "objects", saved.id] });
      setTargetId(saved.id);
      setPrefix(saved.prefix);
      setTargetDialogOpen(false);
      setEditingTarget(null);
      toast.success("Bucket 绑定已保存");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const removeTarget = useMutation({
    mutationFn: deleteOssTarget,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ossTargetsQueryKey(accountId) });
      setTargetId(null);
      toast.success("本地 Bucket 绑定已移除");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const testTarget = useMutation({
    mutationFn: testOssTarget,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ossAccountsQueryKey });
      toast.success("OSS 连接测试成功");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const upload = useMutation({
    mutationFn: startOssUpload,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ossTransferTasksQueryKey });
    },
  });
  const download = useMutation({
    mutationFn: startOssDownload,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ossTransferTasksQueryKey });
      toast.success("文件已加入下载队列");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const removeObjects = useMutation({
    mutationFn: deleteOssObjects,
    onSuccess: async (result) => {
      await invalidateObjects(queryClient, targetId, prefix);
      toast.success("已删除 " + result.deletedKeys.length + " 个对象");
    },
    onError: (error) => {
      const summary = getErrorSummary(error);
      toast.error(summary.message, {
        description: summary.details?.deletedCount
          ? "已完成 " + summary.details.deletedCount + " 个对象；失败对象：" + (summary.details.failedObjectKey ?? "未知")
          : undefined,
      });
      void invalidateObjects(queryClient, targetId, prefix);
    },
  });
  const copy = useMutation({
    mutationFn: copyOssObject,
    onSuccess: () => {
      void invalidateObjects(queryClient, targetId, prefix);
      toast.success("对象已复制");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const copyDownloadUrl = useMutation({
    mutationFn: getOssObjectDownloadUrl,
    onSuccess: async (result) => {
      try {
        await copyTextToClipboard(result.url);
        toast.success("下载 URL 已复制", {
          description: `有效期至 ${formatDate(result.expiresAt)}`,
        });
      } catch (error) {
        toast.error("下载 URL 已生成，但复制失败", {
          description: getClipboardErrorMessage(error),
        });
      }
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const createFolder = useMutation({
    mutationFn: createOssFolder,
    onSuccess: async () => {
      await invalidateObjects(queryClient, targetId, prefix);
      setFolderDialogOpen(false);
      toast.success("文件夹已创建");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });

  const handleTransferTaskStateChanged = useCallback((task: OssTransferTaskView) => {
    if (task.status !== "completed" || task.operation !== "upload") {
      return;
    }
    void invalidateObjectsForTarget(queryClient, task.targetId);
    if (
      task.targetId === targetId
      && task.objectKey.startsWith(prefix)
      && !completedUploadIds.current.has(task.id)
    ) {
      completedUploadIds.current.add(task.id);
      toast.success("文件上传完成，目录已刷新", { description: task.objectKey });
    }
  }, [prefix, queryClient, targetId]);

  const enqueueUploads = useCallback(async (paths: string[]) => {
    if (!target || !paths.length) {
      return;
    }
    setUploadBatchPending(true);
    try {
      const result = await enqueueOssUploadPaths(
        paths,
        target.id,
        prefix,
        async (input: OssUploadStartInput) => {
          try {
            return await upload.mutateAsync(input);
          } catch (error) {
            const summary = getErrorSummary(error);
            if (summary.code === "OSS_LOCAL_FILE_INVALID" && summary.message === "upload source must be a file") {
              throw new Error("仅支持文件，不支持文件夹");
            }
            throw new Error(summary.message);
          }
        },
      );
      const rejectionSummary = result.rejected
        .slice(0, 3)
        .map((item) => `${item.name}：${item.reason}`)
        .join("；");
      if (result.accepted.length && result.rejected.length) {
        toast.warning(`已加入 ${result.accepted.length} 个文件`, {
          description: `${result.rejected.length} 个文件未入队${rejectionSummary ? `：${rejectionSummary}` : ""}`,
        });
      } else if (result.accepted.length) {
        toast.success(`已加入 ${result.accepted.length} 个文件到上传队列`);
      } else {
        toast.error("没有文件加入上传队列", { description: rejectionSummary || "请选择有效文件" });
      }
    } finally {
      setUploadBatchPending(false);
    }
  }, [prefix, target, upload.mutateAsync]);

  useEffect(() => {
    setTargetId(null);
    setPrefix("");
    setContinuationToken(null);
    setSearch("");
    setFolderDialogOpen(false);
  }, [accountId]);

  useEffect(() => {
    const targets = targetsQuery.data ?? [];
    if (!targets.length) {
      setTargetId(null);
      setPrefix("");
      setContinuationToken(null);
      return;
    }
    const selected = targets.find((item) => item.id === targetId);
    if (!selected) {
      const next = targets.find((item) => item.isDefault) ?? targets[0];
      setTargetId(next.id);
      setPrefix(next.prefix);
      setContinuationToken(null);
    }
  }, [targetId, targetsQuery.data]);

  useEffect(() => {
    if (target) {
      setPrefix(target.prefix);
      setContinuationToken(null);
      setSearch("");
    }
  }, [targetId]);

  const rows = useMemo(() => {
    const folders: BrowserRow[] = (objectsQuery.data?.commonPrefixes ?? []).map((item) => ({
      kind: "folder",
      key: item,
      label: relativeName(item, prefix),
      prefix: item,
    }));
    const objects: BrowserRow[] = (objectsQuery.data?.objects ?? []).map((item) => ({
      kind: "object",
      key: item.key,
      label: relativeName(item.key, prefix),
      object: item,
    }));
    const normalized = search.trim().toLocaleLowerCase();
    return normalized
      ? [...folders, ...objects].filter((item) => item.label.toLocaleLowerCase().includes(normalized))
      : [...folders, ...objects];
  }, [objectsQuery.data, prefix, search]);

  if (!desktop) {
    return (
      <div className="oss-surface flex h-full items-center justify-center p-6">
        <Empty description="请在 Bexo Studio 桌面窗口中使用 OSS 文件管理" />
      </div>
    );
  }

  return (
    <div className="oss-surface flex h-full min-h-0 flex-col">
      <header className="oss-border flex shrink-0 items-center justify-between border-b px-5 py-3">
        <div>
          <Typography.Text className="oss-text-primary block text-[13px] font-semibold">OSS 文件管理</Typography.Text>
          <Typography.Text className="oss-text-tertiary mt-0.5 block text-[11px]">
            多账号、Bucket 绑定、目录浏览与可恢复传输
          </Typography.Text>
        </div>
        <Button
          disabled={!accountId}
          icon={<PlusOutlined />}
          onClick={() => {
            setEditingTarget(null);
            setTargetDialogOpen(true);
          }}
          size="small"
        >
          添加 Bucket
        </Button>
      </header>

      {!accountId ? (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <Empty description="请先在左侧添加或选择 OSS 账号" />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1">
          <TargetPane
            deleting={removeTarget.isPending}
            loading={targetsQuery.isLoading}
            onAdd={() => {
              setEditingTarget(null);
              setTargetDialogOpen(true);
            }}
            onDelete={(item) =>
              Modal.confirm({
                cancelText: "取消",
                content: "只会移除本地绑定，不会删除云端 Bucket 或文件。",
                okButtonProps: { danger: true },
                okText: "移除绑定",
                title: "移除这个 Bucket 绑定？",
                onOk: () => removeTarget.mutateAsync(item.id),
              })
            }
            onEdit={(item) => {
              setEditingTarget(item);
              setTargetDialogOpen(true);
            }}
            onSelect={(item) => {
              setTargetId(item.id);
              setPrefix(item.prefix);
              setContinuationToken(null);
            }}
            onTest={(item) => testTarget.mutate({ targetId: item.id })}
            selectedId={targetId}
            targets={targetsQuery.data ?? []}
            testingId={testTarget.isPending ? testTarget.variables?.targetId ?? null : null}
          />
          <div className="flex min-w-0 flex-1 flex-col">
            {target ? (
              <ObjectBrowser
                creatingFolder={createFolder.isPending}
                currentPrefix={prefix}
                downloadLoading={download.isPending}
                objectSearch={search}
                objectsQuery={objectsQuery}
                nextToken={objectsQuery.data?.nextContinuationToken ?? null}
                onBack={() => {
                  setPrefix(parentPrefix(prefix, target.prefix));
                  setContinuationToken(null);
                }}
                onDelete={(item) =>
                  Modal.confirm({
                    cancelText: "取消",
                    content: "删除后无法通过 Bexo Studio 恢复，请确认云端对象已经不再需要。",
                    okButtonProps: { danger: true },
                    okText: "删除对象",
                    title: "删除这个对象？",
                    onOk: () => removeObjects.mutateAsync({ targetId: target.id, objectKeys: [item.key] }),
                  })
                }
                onDownload={(item) => void pickDownload(item, target, download.mutateAsync)}
                onCopy={(item) => {
                  copy.reset();
                  setCopySourceObject(item);
                  setCopyDraft(buildOssCopyTargetKey(item.key));
                }}
                onCopyDownloadUrl={(item) => {
                  void copyDownloadUrl.mutateAsync({ targetId: target.id, objectKey: item.key }).then(
                    () => copyDownloadUrl.reset(),
                    (error) => {
                      copyDownloadUrl.reset();
                      const summary = getErrorSummary(error);
                      console.error("OSS download URL generation failed", {
                        code: summary.code,
                        message: summary.message,
                      });
                    },
                  );
                }}
                onCreateFolder={() => {
                  createFolder.reset();
                  setFolderDialogOpen(true);
                }}
                onFolderOpen={(nextPrefix) => {
                  setPrefix(nextPrefix);
                  setContinuationToken(null);
                }}
                onNextPage={(nextToken) => setContinuationToken(nextToken)}
                onRefresh={() => void objectsQuery.refetch()}
                onRename={(item) => {
                  setRenameObject(item);
                  setRenameDraft(item.key);
                }}
                onSearch={setSearch}
                onDropPaths={(paths) => void enqueueUploads(paths)}
                onUpload={() => void pickUpload(enqueueUploads)}
                rows={rows}
                uploading={upload.isPending || uploadBatchPending}
              />
            ) : (
              <div className="flex min-h-0 flex-1 items-center justify-center">
                <Empty description="请先添加或选择一个 Bucket 绑定" />
              </div>
            )}
            <OssTransferQueue onTaskStateChanged={handleTransferTaskStateChanged} />
          </div>
        </div>
      )}

      {accountId ? (
        <OssTargetDialog
          accountId={accountId}
          confirmLoading={saveTarget.isPending}
          onCancel={() => {
            setTargetDialogOpen(false);
            setEditingTarget(null);
          }}
          onSubmit={async (input: UpsertOssTargetPayload) => {
            await saveTarget.mutateAsync(input);
          }}
          open={targetDialogOpen}
          target={editingTarget}
        />
      ) : null}
      {target ? (
        <OssFolderDialog
          confirmLoading={createFolder.isPending}
          errorMessage={createFolder.error ? getErrorSummary(createFolder.error).message : null}
          onCancel={() => {
            setFolderDialogOpen(false);
            createFolder.reset();
          }}
          onSubmit={async (name) => {
            await createFolder.mutateAsync({
              name,
              parentPrefix: prefix,
              targetId: target.id,
            });
          }}
          open={folderDialogOpen}
          parentPrefix={prefix}
        />
      ) : null}
      {copySourceObject ? (
        <OssCopyDialog
          confirmLoading={copy.isPending}
          errorMessage={copy.error ? getErrorSummary(copy.error).message : null}
          onCancel={() => {
            setCopySourceObject(null);
            setCopyDraft("");
            copy.reset();
          }}
          onChange={setCopyDraft}
          onSubmit={async (targetKey) => {
            if (!target) {
              return;
            }
            try {
              await copy.mutateAsync({
                overwrite: false,
                sourceKey: copySourceObject.key,
                targetId: target.id,
                targetKey,
              });
            } catch (error) {
              const summary = getErrorSummary(error);
              console.error("OSS object copy failed", {
                code: summary.code,
                message: summary.message,
              });
              return;
            }
            setCopySourceObject(null);
            setCopyDraft("");
          }}
          open
          sourceKey={copySourceObject.key}
          targetKey={copyDraft}
        />
      ) : null}
      <Modal
        cancelText="取消"
        confirmLoading={copy.isPending || removeObjects.isPending}
        okText="保存重命名"
        onCancel={() => setRenameObject(null)}
        onOk={() => {
          if (renameObject && target) {
            void rename(
              renameObject,
              renameDraft,
              target,
              copy.mutateAsync,
              removeObjects.mutateAsync,
              () => setRenameObject(null),
            );
          }
        }}
        open={Boolean(renameObject)}
        title="重命名对象"
      >
        <Typography.Text className="oss-text-secondary mb-2 block text-[11px]">
          使用 CopyObject + DeleteObject 完成同 Bucket 重命名。
        </Typography.Text>
        <Input className="font-mono" onChange={(event) => setRenameDraft(event.target.value)} value={renameDraft} />
      </Modal>
    </div>
  );
}

type BrowserRow =
  | { kind: "folder"; key: string; label: string; prefix: string }
  | { kind: "object"; key: string; label: string; object: OssObjectEntry };

function TargetPane({
  targets,
  selectedId,
  loading,
  deleting,
  testingId,
  onSelect,
  onAdd,
  onEdit,
  onDelete,
  onTest,
}: {
  targets: OssTargetRecord[];
  selectedId: string | null;
  loading: boolean;
  deleting: boolean;
  testingId: string | null;
  onSelect: (target: OssTargetRecord) => void;
  onAdd: () => void;
  onEdit: (target: OssTargetRecord) => void;
  onDelete: (target: OssTargetRecord) => void;
  onTest: (target: OssTargetRecord) => void;
}) {
  return (
    <aside className="oss-surface-muted oss-border flex w-[236px] shrink-0 flex-col border-r">
      <div className="oss-border flex items-center justify-between border-b px-3 py-3">
        <div>
          <Typography.Text className="oss-text-tertiary block text-[11px] font-semibold uppercase tracking-[0.16em]">TARGETS</Typography.Text>
          <Typography.Text className="oss-text-primary mt-0.5 block text-[12px] font-medium">Bucket 绑定</Typography.Text>
        </div>
        <Button aria-label="添加 Bucket 绑定" className="!h-7 !w-7 !min-w-7 !p-0" icon={<PlusOutlined />} onClick={onAdd} size="small" type="text" />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {loading ? (
          <div className="flex min-h-[180px] items-center justify-center"><Spin size="small" /></div>
        ) : targets.length ? (
          <div className="space-y-1">
            {targets.map((item) => (
              <div
                className={cn(
                  "group oss-focusable cursor-pointer rounded-[10px] border px-3 py-2.5",
                  item.id === selectedId
                    ? "oss-border-selected oss-surface-selected"
                    : "oss-card-hover border-transparent bg-transparent",
                )}
                key={item.id}
                onClick={() => onSelect(item)}
                role="button"
                tabIndex={0}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onSelect(item);
                  }
                }}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <Typography.Text className="oss-text-primary block truncate text-[12px] font-medium">{item.displayName}</Typography.Text>
                    <Typography.Text className="oss-text-tertiary mt-0.5 block truncate font-mono text-[10px]">{item.bucket}</Typography.Text>
                    <Typography.Text className="oss-text-tertiary mt-1 block truncate text-[10px]">{item.prefix || "根目录"}</Typography.Text>
                  </div>
                  <Dropdown
                    menu={{
                      items: [
                        { key: "test", label: "测试连接", icon: <ExperimentOutlined /> },
                        { key: "edit", label: "编辑绑定" },
                        { key: "delete", label: "移除绑定", danger: true },
                      ],
                      onClick: ({ key, domEvent }) => {
                        domEvent.stopPropagation();
                        if (key === "test") onTest(item);
                        if (key === "edit") onEdit(item);
                        if (key === "delete") onDelete(item);
                      },
                    }}
                    trigger={["click"]}
                  >
                    <Button
                      aria-label={"操作 " + item.displayName}
                      className="!h-6 !w-6 !min-w-6 !p-0 opacity-0 group-hover:opacity-100"
                      disabled={deleting || Boolean(testingId)}
                      icon={<MoreOutlined />}
                      onClick={(event) => event.stopPropagation()}
                      size="small"
                      type="text"
                    />
                  </Dropdown>
                </div>
                {item.isDefault ? <Tag className="!m-0 mt-1 text-[10px]" color="blue">默认</Tag> : null}
              </div>
            ))}
          </div>
        ) : <Empty className="!my-12" description="还没有 Bucket 绑定" image={Empty.PRESENTED_IMAGE_SIMPLE} />}
      </div>
    </aside>
  );
}

function ObjectBrowser({
  currentPrefix,
  creatingFolder,
  nextToken,
  objectSearch,
  rows,
  objectsQuery,
  uploading,
  downloadLoading,
  onBack,
  onCreateFolder,
  onFolderOpen,
  onNextPage,
  onDropPaths,
  onUpload,
  onRefresh,
  onSearch,
  onDownload,
  onDelete,
  onRename,
  onCopy,
  onCopyDownloadUrl,
}: {
  currentPrefix: string;
  creatingFolder: boolean;
  nextToken: string | null;
  objectSearch: string;
  rows: BrowserRow[];
  objectsQuery: UseQueryResult<OssObjectPage, unknown>;
  uploading: boolean;
  downloadLoading: boolean;
  onBack: () => void;
  onFolderOpen: (prefix: string) => void;
  onNextPage: (token: string) => void;
  onDropPaths: (paths: string[]) => void;
  onUpload: () => void;
  onCreateFolder: () => void;
  onRefresh: () => void;
  onSearch: (value: string) => void;
  onDownload: (object: OssObjectEntry) => void;
  onDelete: (object: OssObjectEntry) => void;
  onRename: (object: OssObjectEntry) => void;
  onCopy: (object: OssObjectEntry) => void;
  onCopyDownloadUrl: (object: OssObjectEntry) => void;
}) {
  const [scrollTop, setScrollTop] = useState(0);
  const sectionRef = useRef<HTMLElement | null>(null);
  const [dropPreview, setDropPreview] = useState<{ paths: string[]; inside: boolean } | null>(null);
  const virtual = rows.length >= 200;
  const start = virtual ? Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - 5) : 0;
  const visible = rows.slice(start, virtual ? Math.min(rows.length, start + 32) : rows.length);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      return;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const isInside = (position: { x: number; y: number }) => {
      const rect = sectionRef.current?.getBoundingClientRect();
      if (!rect) return false;
      const scale = window.devicePixelRatio || 1;
      const x = position.x / scale;
      const y = position.y / scale;
      return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
    };
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter") {
          setDropPreview({ paths: payload.paths, inside: isInside(payload.position) });
          return;
        }
        if (payload.type === "over") {
          setDropPreview((current) => current ? { ...current, inside: isInside(payload.position) } : current);
          return;
        }
        if (payload.type === "leave") {
          setDropPreview(null);
          return;
        }
        const inside = isInside(payload.position);
        setDropPreview(null);
        if (inside && payload.paths.length) {
          onDropPaths(payload.paths);
        }
      })
      .then((removeListener) => {
        if (disposed) {
          removeListener();
        } else {
          unlisten = removeListener;
        }
      })
      .catch((error) => {
        console.error("Failed to listen OSS file drop events", error);
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onDropPaths]);

  return (
    <section className="relative flex min-h-0 flex-1 flex-col" ref={sectionRef}>
      {dropPreview?.inside ? (
        <div className="pointer-events-none absolute inset-3 z-20 flex items-center justify-center rounded-[14px] border-2 border-dashed border-sky-400/80 bg-sky-950/45 backdrop-blur-[2px]">
          <div className="rounded-[10px] border border-sky-300/50 bg-slate-950/75 px-5 py-4 text-center shadow-lg">
            <UploadOutlined className="mb-2 text-2xl text-sky-300" />
            <Typography.Text className="block text-[13px] font-medium text-sky-100">
              释放 {dropPreview.paths.length} 个文件到 {currentPrefix || "/"}
            </Typography.Text>
            <Typography.Text className="mt-1 block text-[11px] text-sky-200/75">
              文件夹不支持递归上传
            </Typography.Text>
          </div>
        </div>
      ) : null}
      <div className="oss-border flex shrink-0 items-center gap-2 border-b px-4 py-3">
        <Button aria-label="返回上一级目录" className="!h-7 !w-7 !min-w-7 !p-0" disabled={!currentPrefix} icon={<ArrowLeftOutlined />} onClick={onBack} size="small" type="text" />
        <Typography.Text className="oss-text-primary min-w-0 flex-1 truncate font-mono text-[12px] font-medium">{currentPrefix || "/"}</Typography.Text>
        <Input allowClear className="!w-[180px]" onChange={(event) => onSearch(event.target.value)} placeholder="筛选当前页..." value={objectSearch} />
        <Tooltip title="刷新目录">
          <Button aria-label="刷新目录" className="!h-7 !w-7 !min-w-7 !p-0" icon={<ReloadOutlined spin={objectsQuery.isFetching} />} onClick={onRefresh} size="small" type="text" />
        </Tooltip>
        <Button disabled={uploading} icon={<FolderAddOutlined />} loading={creatingFolder} onClick={onCreateFolder} size="small">新建文件夹</Button>
        <Button icon={<UploadOutlined />} loading={uploading} onClick={onUpload} size="small" type="primary">上传文件</Button>
      </div>
      {objectsQuery.error ? <div className="px-4 pt-3"><Typography.Text className="oss-text-danger" type="danger">读取对象失败：{getErrorSummary(objectsQuery.error).message}</Typography.Text></div> : null}
      <div className="min-h-0 flex-1 overflow-auto" onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}>
        {objectsQuery.isLoading ? (
          <div className="flex min-h-[260px] items-center justify-center"><Spin size="small" /></div>
        ) : rows.length ? (
          <div style={virtual ? { height: rows.length * ROW_HEIGHT } : undefined}>
            <div style={virtual ? { transform: "translateY(" + start * ROW_HEIGHT + "px)" } : undefined}>
              <div className="oss-border grid grid-cols-[minmax(0,1fr)_110px_180px_92px] border-b px-4 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] oss-text-tertiary"><span>名称</span><span>大小</span><span>修改时间</span><span className="text-right">操作</span></div>
              {visible.map((row) =>
                row.kind === "folder" ? (
                  <button className="oss-object-row cursor-pointer grid h-[46px] w-full grid-cols-[minmax(0,1fr)_110px_180px_92px] items-center border-b oss-border-subtle px-4 text-left" key={row.key} onClick={() => onFolderOpen(row.prefix)} type="button">
                    <span className="flex min-w-0 items-center gap-2"><FolderOpenOutlined className="oss-text-folder" /><span className="oss-text-primary truncate text-[12px] font-medium">{row.label}</span></span><span className="oss-text-tertiary text-[11px]">目录</span><span className="oss-text-tertiary text-[11px]">—</span><span />
                  </button>
                ) : <ObjectRow downloadLoading={downloadLoading} key={row.key} label={row.label} object={row.object} onCopy={onCopy} onCopyDownloadUrl={onCopyDownloadUrl} onDelete={onDelete} onDownload={onDownload} onRename={onRename} />,
              )}
            </div>
          </div>
        ) : <Empty className="!my-20" description={objectSearch ? "当前页没有匹配对象" : "这个目录还是空的"} image={Empty.PRESENTED_IMAGE_SIMPLE} />}
      </div>
      {objectsQuery.data?.isTruncated && nextToken ? (
        <div className="oss-border flex shrink-0 justify-center border-t px-4 py-2">
          <Button onClick={() => onNextPage(nextToken)} size="small">下一页</Button>
        </div>
      ) : null}
    </section>
  );
}

function ObjectRow({
  object,
  label,
  downloadLoading,
  onCopy,
  onCopyDownloadUrl,
  onDownload,
  onDelete,
  onRename,
}: {
  object: OssObjectEntry;
  label: string;
  downloadLoading: boolean;
  onCopy: (object: OssObjectEntry) => void;
  onCopyDownloadUrl: (object: OssObjectEntry) => void;
  onDownload: (object: OssObjectEntry) => void;
  onDelete: (object: OssObjectEntry) => void;
  onRename: (object: OssObjectEntry) => void;
}) {
  return (
    <Dropdown
      menu={{
        items: [
          { key: "download", label: "下载文件", icon: <DownloadOutlined /> },
          { key: "copy", label: "复制文件", icon: <CopyOutlined /> },
          { key: "url", label: "复制下载 URL", icon: <LinkOutlined /> },
          { key: "delete", label: "删除", danger: true, icon: <DeleteOutlined /> },
        ],
        onClick: ({ key, domEvent }) => {
          domEvent.stopPropagation();
          if (key === "download") onDownload(object);
          if (key === "copy") onCopy(object);
          if (key === "url") onCopyDownloadUrl(object);
          if (key === "delete") onDelete(object);
        },
      }}
      trigger={["contextMenu"]}
    >
      <div className="oss-object-row grid h-[46px] grid-cols-[minmax(0,1fr)_110px_180px_92px] items-center border-b oss-border-subtle px-4">
        <span className="flex min-w-0 items-center gap-2"><FileOutlined className="oss-text-accent" /><span className="oss-text-primary truncate font-mono text-[12px]" title={object.key}>{label}</span></span>
        <span className="oss-text-secondary text-[11px]">{formatBytes(object.size)}</span>
        <span className="oss-text-tertiary truncate text-[11px]">{formatDate(object.lastModified)}</span>
        <Dropdown
          menu={{
            items: [
              { key: "download", label: "下载", icon: <DownloadOutlined /> },
              { key: "copy", label: "复制文件", icon: <CopyOutlined /> },
              { key: "url", label: "复制下载 URL", icon: <LinkOutlined /> },
              { key: "rename", label: "重命名" },
              { key: "delete", label: "删除对象", danger: true, icon: <DeleteOutlined /> },
            ],
            onClick: ({ key }) => {
              if (key === "download") onDownload(object);
              if (key === "copy") onCopy(object);
              if (key === "url") onCopyDownloadUrl(object);
              if (key === "rename") onRename(object);
              if (key === "delete") onDelete(object);
            },
          }}
          trigger={["click"]}
        >
          <Button aria-label={"操作 " + object.key} className="!h-7 !w-7 !min-w-7 !justify-self-end !p-0" disabled={downloadLoading} icon={<MoreOutlined />} size="small" type="text" />
        </Dropdown>
      </div>
    </Dropdown>
  );
}

async function pickUpload(enqueue: (paths: string[]) => Promise<void>) {
  try {
    const selected = await open({ directory: false, multiple: true, title: "选择要上传的文件" });
    if (!selected) return;
    await enqueue(Array.isArray(selected) ? selected : [selected]);
  } catch (error) {
    toast.error(getErrorSummary(error).message);
  }
}

async function pickDownload(
  object: OssObjectEntry,
  target: OssTargetRecord,
  start: (input: { targetId: string; objectKey: string; localPath: string; overwrite: boolean }) => Promise<unknown>,
) {
  try {
    const name = object.key.split("/").filter(Boolean).at(-1) ?? "download";
    const selected = await save({ defaultPath: name, title: "选择下载位置" });
    if (typeof selected !== "string" || !selected.trim()) return;
    await start({ targetId: target.id, objectKey: object.key, localPath: selected, overwrite: false });
  } catch (error) {
    toast.error(getErrorSummary(error).message);
  }
}

async function rename(
  object: OssObjectEntry,
  nextKey: string,
  target: OssTargetRecord,
  copy: (input: { targetId: string; sourceKey: string; targetKey: string; overwrite: boolean }) => Promise<unknown>,
  remove: (input: { targetId: string; objectKeys: string[] }) => Promise<unknown>,
  done: () => void,
) {
  const normalized = nextKey.trim();
  if (!normalized || normalized === object.key || normalized.startsWith("/")) {
    toast.error("目标对象名无效，且不能与原对象相同");
    return;
  }
  try {
    await copy({ targetId: target.id, sourceKey: object.key, targetKey: normalized, overwrite: false });
    try {
      await remove({ targetId: target.id, objectKeys: [object.key] });
      toast.success("对象已重命名");
      done();
    } catch (error) {
      toast.warning("新对象已复制，但旧对象删除失败", { description: getErrorSummary(error).message });
    }
  } catch (error) {
    toast.error(getErrorSummary(error).message);
  }
}

async function invalidateObjects(
  queryClient: ReturnType<typeof useQueryClient>,
  targetId: string | null,
  prefix: string,
) {
  if (targetId) {
    await queryClient.invalidateQueries({
      queryKey: ["oss", "objects", targetId, prefix],
    });
  }
}

async function invalidateObjectsForTarget(
  queryClient: ReturnType<typeof useQueryClient>,
  targetId: string,
) {
  await queryClient.invalidateQueries({
    queryKey: ["oss", "objects", targetId],
  });
}

function relativeName(value: string, prefix: string) {
  const result = value.startsWith(prefix) ? value.slice(prefix.length) : value;
  return result.replace(/\/$/, "") || value;
}

function parentPrefix(prefix: string, root: string) {
  if (!prefix || prefix === root) return root;
  const value = prefix.replace(/\/$/, "");
  const index = value.lastIndexOf("/");
  const parent = index >= 0 ? value.slice(0, index + 1) : "";
  return parent.length >= root.length ? parent : root;
}

function formatBytes(value?: number | null) {
  if (value === null || value === undefined) return "—";
  if (value < 1024) return String(value) + " B";
  if (value < 1024 * 1024) return (value / 1024).toFixed(1) + " KB";
  if (value < 1024 * 1024 * 1024) return (value / (1024 * 1024)).toFixed(1) + " MB";
  return (value / (1024 * 1024 * 1024)).toFixed(1) + " GB";
}

function formatDate(value?: string | null) {
  if (!value) return "—";
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? value : new Date(timestamp).toLocaleString();
}
