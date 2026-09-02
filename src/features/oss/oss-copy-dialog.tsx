import { Alert, Input, Modal, Typography } from "antd";

import { validateOssCopyTargetKey } from "./oss-copy-model";

type OssCopyDialogProps = {
  open: boolean;
  sourceKey: string;
  targetKey: string;
  confirmLoading?: boolean;
  errorMessage?: string | null;
  onCancel: () => void;
  onChange: (value: string) => void;
  onSubmit: (targetKey: string) => Promise<void>;
};

export function OssCopyDialog({
  open,
  sourceKey,
  targetKey,
  confirmLoading = false,
  errorMessage,
  onCancel,
  onChange,
  onSubmit,
}: OssCopyDialogProps) {
  const validationError = validateOssCopyTargetKey(targetKey, sourceKey);

  return (
    <Modal
      cancelText="取消"
      confirmLoading={confirmLoading}
      okButtonProps={{ disabled: Boolean(validationError) }}
      okText="复制文件"
      onCancel={onCancel}
      onOk={() => {
        if (!validationError) {
          void onSubmit(targetKey.trim());
        }
      }}
      open={open}
      title="复制文件"
      width={520}
    >
      <div className="space-y-4 pt-1">
        <div className="oss-surface-info oss-border-info rounded-[10px] border px-3 py-2.5">
          <Typography.Text className="oss-text-primary block text-[12px] font-medium">
            在当前 Bucket 内复制对象
          </Typography.Text>
          <Typography.Text className="oss-text-secondary mt-1 block text-[11px] leading-5">
            源文件不会被删除。目标已存在时不会覆盖，避免误操作覆盖云端文件。
          </Typography.Text>
        </div>
        {errorMessage ? <Alert showIcon message={errorMessage} type="error" /> : null}
        <div>
          <Typography.Text className="oss-text-tertiary mb-1.5 block text-[11px]">
            源 Object Key
          </Typography.Text>
          <Typography.Text className="oss-text-secondary block break-all rounded-[8px] border px-3 py-2 font-mono text-[11px]">
            {sourceKey}
          </Typography.Text>
        </div>
        <div>
          <Typography.Text className="oss-text-primary mb-1.5 block text-[11px]">
            目标 Object Key
          </Typography.Text>
          <Input
            autoFocus
            className="font-mono"
            maxLength={1024}
            onChange={(event) => onChange(event.target.value)}
            status={validationError ? "error" : undefined}
            value={targetKey}
          />
          <Typography.Text className={validationError ? "oss-text-danger mt-1 block text-[11px]" : "oss-text-tertiary mt-1 block text-[11px]"}>
            {validationError ?? "复制只发生在当前 Bucket，且目标必须在当前绑定的目录范围内。"}
          </Typography.Text>
        </div>
      </div>
    </Modal>
  );
}
