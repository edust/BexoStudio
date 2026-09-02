import {
  CloudOutlined,
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  ReloadOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Empty, Input, Popconfirm, Spin, Tag, Tooltip, Typography } from "antd";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { cn } from "@/lib/cn";
import {
  deleteOssAccount,
  getErrorSummary,
  hasDesktopRuntime,
  listOssAccounts,
  upsertOssAccount,
} from "@/lib/command-client";
import { ossAccountsQueryKey } from "@/queries/oss";
import { useShellStore } from "@/stores/shell-store";
import type { OssAccountSummary, UpsertOssAccountPayload } from "@/types/backend";

import { OssAccountDialog } from "./oss-account-dialog";

export function OssAccountSidebar() {
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const queryClient = useQueryClient();
  const selectedAccountId = useShellStore((state) => state.selectedOssAccountId);
  const setSelectedAccountId = useShellStore((state) => state.setSelectedOssAccountId);
  const [query, setQuery] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingAccount, setEditingAccount] = useState<OssAccountSummary | null>(null);
  const accountsQuery = useQuery({
    queryKey: ossAccountsQueryKey,
    queryFn: listOssAccounts,
    enabled: desktopRuntimeAvailable,
    staleTime: 20_000,
  });
  const saveMutation = useMutation({
    mutationFn: upsertOssAccount,
    onSuccess: async (account) => {
      queryClient.setQueryData<OssAccountSummary[]>(ossAccountsQueryKey, (current) => {
        const next = current?.filter((item) => item.id !== account.id) ?? [];
        return [...next, account].sort((left, right) => left.displayName.localeCompare(right.displayName));
      });
      await queryClient.invalidateQueries({ queryKey: ossAccountsQueryKey });
      setSelectedAccountId(account.id);
      setDialogOpen(false);
      setEditingAccount(null);
      toast.success("OSS 账号已保存");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const deleteMutation = useMutation({
    mutationFn: deleteOssAccount,
    onSuccess: async (result) => {
      await queryClient.invalidateQueries({ queryKey: ossAccountsQueryKey });
      if (selectedAccountId === result.id) {
        setSelectedAccountId(null);
      }
      toast.success("OSS 账号已移除");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });

  const filteredAccounts = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) {
      return accountsQuery.data ?? [];
    }
    return (accountsQuery.data ?? []).filter((account) =>
      [account.displayName, account.accessKeyIdHint].join(" ").toLocaleLowerCase().includes(normalized),
    );
  }, [accountsQuery.data, query]);

  useEffect(() => {
    const accounts = accountsQuery.data ?? [];
    if (!accounts.length) {
      if (selectedAccountId) {
        setSelectedAccountId(null);
      }
      return;
    }
    if (!selectedAccountId || !accounts.some((account) => account.id === selectedAccountId)) {
      setSelectedAccountId(accounts[0].id);
    }
  }, [accountsQuery.data, selectedAccountId, setSelectedAccountId]);

  function openCreateDialog() {
    setEditingAccount(null);
    setDialogOpen(true);
  }

  function openEditDialog(account: OssAccountSummary) {
    setEditingAccount(account);
    setDialogOpen(true);
  }

  async function handleSubmit(input: UpsertOssAccountPayload) {
    await saveMutation.mutateAsync(input);
  }

  if (!desktopRuntimeAvailable) {
    return (
      <aside className="bexo-shell-surface flex h-full min-h-0 flex-col overflow-hidden rounded-[16px]">
        <SidebarHeader onAdd={openCreateDialog} />
        <div className="flex min-h-0 flex-1 items-center px-5">
          <Alert
            className="w-full"
            description="请在 Bexo Studio 桌面窗口中配置 OSS 账号。"
            message="需要桌面 runtime"
            showIcon
            type="info"
          />
        </div>
        <OssAccountDialog
          account={editingAccount}
          confirmLoading={saveMutation.isPending}
          onCancel={() => setDialogOpen(false)}
          onSubmit={handleSubmit}
          open={dialogOpen}
        />
      </aside>
    );
  }

  return (
    <aside className="bexo-shell-surface flex h-full min-h-0 flex-col overflow-hidden rounded-[16px]">
      <SidebarHeader
        loading={accountsQuery.isFetching}
        onAdd={openCreateDialog}
        onRefresh={() => void accountsQuery.refetch()}
      />
      <div className="oss-border border-b px-3 pb-3">
        <Input
          allowClear
          onChange={(event) => setQuery(event.target.value)}
          placeholder="搜索 OSS 账号..."
          prefix={<CloudOutlined className="oss-text-tertiary" />}
          value={query}
        />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {accountsQuery.error ? (
          <Alert
            action={
              <Button onClick={() => void accountsQuery.refetch()} size="small" type="link">
                重试
              </Button>
            }
            className="mb-2"
            description={getErrorSummary(accountsQuery.error).message}
            message="读取 OSS 账号失败"
            showIcon
            type="error"
          />
        ) : null}
        {accountsQuery.isLoading ? (
          <div className="flex min-h-[180px] items-center justify-center">
            <Spin size="small" />
          </div>
        ) : filteredAccounts.length ? (
          <div className="space-y-1">
            {filteredAccounts.map((account) => (
              <AccountItem
                account={account}
                deleting={deleteMutation.isPending && deleteMutation.variables === account.id}
                key={account.id}
                onDelete={() => deleteMutation.mutate(account.id)}
                onEdit={() => openEditDialog(account)}
                onSelect={() => setSelectedAccountId(account.id)}
                selected={selectedAccountId === account.id}
              />
            ))}
          </div>
        ) : (
          <Empty
            className="!my-12"
            description={query ? "没有匹配的 OSS 账号" : "还没有 OSS 账号"}
            image={Empty.PRESENTED_IMAGE_SIMPLE}
          />
        )}
      </div>

      <div className="oss-border border-t px-3 py-3">
        <div className="oss-surface-muted flex items-start gap-2 rounded-[10px] px-2.5 py-2">
          <SafetyCertificateOutlined className="oss-text-accent mt-0.5" />
          <Typography.Text className="oss-text-secondary text-[11px] leading-4">
            Secret 使用 Windows Credential Manager 保存。这里仅管理账号引用和 Bucket 绑定。
          </Typography.Text>
        </div>
      </div>

      <OssAccountDialog
        account={editingAccount}
        confirmLoading={saveMutation.isPending}
        onCancel={() => {
          setDialogOpen(false);
          setEditingAccount(null);
        }}
        onSubmit={handleSubmit}
        open={dialogOpen}
      />
    </aside>
  );
}

function SidebarHeader({
  loading = false,
  onAdd,
  onRefresh,
}: {
  loading?: boolean;
  onAdd: () => void;
  onRefresh?: () => void;
}) {
  return (
    <div className="flex items-center justify-between px-4 py-3">
      <div>
        <Typography.Text className="oss-text-tertiary block text-[11px] font-semibold uppercase tracking-[0.2em]">
          OSS FILES
        </Typography.Text>
        <Typography.Text className="oss-text-primary mt-1 block text-[14px] font-semibold">
          账号
        </Typography.Text>
      </div>
      <div className="flex items-center gap-1">
        {onRefresh ? (
          <Tooltip title="刷新账号">
            <Button
              aria-label="刷新 OSS 账号"
              className="!h-7 !w-7 !min-w-7 !p-0"
              icon={<ReloadOutlined spin={loading} />}
              onClick={onRefresh}
              size="small"
              type="text"
            />
          </Tooltip>
        ) : null}
        <Tooltip title="添加 RAM AccessKey">
          <Button
            aria-label="添加 OSS 账号"
            className="!h-7 !w-7 !min-w-7 !p-0"
            icon={<PlusOutlined />}
            onClick={onAdd}
            size="small"
            type="text"
          />
        </Tooltip>
      </div>
    </div>
  );
}

function AccountItem({
  account,
  deleting,
  onDelete,
  onEdit,
  onSelect,
  selected,
}: {
  account: OssAccountSummary;
  deleting: boolean;
  onDelete: () => void;
  onEdit: () => void;
  onSelect: () => void;
  selected: boolean;
}) {
  const statusClassName =
    account.lastProbeStatus === "success"
      ? "oss-status-success"
      : account.lastProbeStatus === "failed"
        ? "oss-status-danger"
        : "oss-status-unknown";
  return (
    <div
      className={cn(
        "group oss-focusable cursor-pointer rounded-[10px] border px-3 py-2.5 transition-colors",
        selected
          ? "oss-border-selected oss-surface-selected"
          : "oss-card-hover border-transparent bg-transparent",
      )}
      onClick={onSelect}
      role="button"
      tabIndex={0}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect();
        }
      }}
    >
      <div className="flex items-start gap-2">
        <span
          className={cn("mt-1.5 h-2 w-2 shrink-0 rounded-full", statusClassName)}
          title={account.lastProbeStatus}
        />
        <div className="min-w-0 flex-1">
          <Typography.Text className="oss-text-primary block truncate text-[13px] font-medium">
            {account.displayName}
          </Typography.Text>
          <Typography.Text className="oss-text-tertiary mt-0.5 block truncate font-mono text-[11px]">
            {account.accessKeyIdHint}
          </Typography.Text>
        </div>
        <div className="flex shrink-0 items-center gap-0 opacity-0 transition-opacity group-hover:opacity-100">
          <Button
            aria-label={"编辑 " + account.displayName}
            className="!h-6 !w-6 !min-w-6 !p-0"
            icon={<EditOutlined />}
            onClick={(event) => {
              event.stopPropagation();
              onEdit();
            }}
            size="small"
            type="text"
          />
          <Popconfirm
            description="只会移除本地账号和凭据引用，不会删除云端 Bucket 或文件。"
            okButtonProps={{ danger: true }}
            okText="移除"
            onConfirm={(event) => {
              event?.stopPropagation();
              onDelete();
            }}
            onCancel={(event) => event?.stopPropagation()}
            title={"移除 " + account.displayName + "？"}
          >
            <Button
              aria-label={"移除 " + account.displayName}
              className="!h-6 !w-6 !min-w-6 !p-0"
              danger
              disabled={deleting}
              icon={<DeleteOutlined />}
              loading={deleting}
              onClick={(event) => event.stopPropagation()}
              size="small"
              type="text"
            />
          </Popconfirm>
        </div>
      </div>
      {account.lastProbeStatus === "failed" && account.lastProbeError ? (
        <Typography.Text className="oss-text-danger mt-1 block truncate text-[10px]">
          {account.lastProbeError}
        </Typography.Text>
      ) : null}
      {account.isDisabled ? (
        <Tag className="mt-1 !m-0 text-[10px]" color="default">
          已停用
        </Tag>
      ) : null}
    </div>
  );
}
