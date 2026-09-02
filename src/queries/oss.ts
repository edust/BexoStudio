import {
  cancelOssTransfer,
  copyOssObject,
  confirmOssTransfer,
  deleteOssAccount,
  deleteOssObjects,
  deleteOssTarget,
  listOssAccounts,
  listOssObjects,
  listOssTargets,
  listOssTransferTasks,
  resumeOssTransfer,
  startOssDownload,
  startOssUpload,
  testOssTarget,
  upsertOssAccount,
  upsertOssTarget,
} from "@/lib/command-client";

export const ossAccountsQueryKey = ["oss", "accounts"] as const;
export const ossTargetsQueryKey = (accountId: string | null) => ["oss", "targets", accountId] as const;
export const ossObjectsQueryKey = (
  targetId: string | null,
  prefix: string,
  continuationToken: string | null,
) => ["oss", "objects", targetId, prefix, continuationToken] as const;
export const ossTransferTasksQueryKey = ["oss", "transfer-tasks"] as const;

export {
  cancelOssTransfer,
  copyOssObject,
  confirmOssTransfer,
  deleteOssAccount,
  deleteOssObjects,
  deleteOssTarget,
  listOssAccounts,
  listOssObjects,
  listOssTargets,
  listOssTransferTasks,
  resumeOssTransfer,
  startOssDownload,
  startOssUpload,
  testOssTarget,
  upsertOssAccount,
  upsertOssTarget,
};
