import { Alert, Form, Input, Modal, Typography } from "antd";
import { useEffect } from "react";

import { buildOssFolderKey, validateOssFolderName } from "./oss-folder-model";

type OssFolderDialogProps = {
  open: boolean;
  parentPrefix: string;
  confirmLoading?: boolean;
  errorMessage?: string | null;
  onCancel: () => void;
  onSubmit: (name: string) => Promise<void>;
};

type FolderFormValues = {
  name: string;
};

export function OssFolderDialog({
  open,
  parentPrefix,
  confirmLoading = false,
  errorMessage,
  onCancel,
  onSubmit,
}: OssFolderDialogProps) {
  const [form] = Form.useForm<FolderFormValues>();
  const name = Form.useWatch("name", form) ?? "";
  const objectKey = name.trim() ? buildOssFolderKey(parentPrefix, name) : "等待输入文件夹名称";

  useEffect(() => {
    if (open) {
      form.setFieldsValue({ name: "" });
    }
  }, [form, open]);

  async function handleOk() {
    const values = await form.validateFields();
    await onSubmit(values.name.trim());
  }

  return (
    <Modal
      cancelText="取消"
      confirmLoading={confirmLoading}
      okText="创建文件夹"
      onCancel={onCancel}
      onOk={() => void handleOk()}
      open={open}
      title="新建文件夹"
      width={500}
    >
      <div className="space-y-4 pt-1">
        <div className="oss-surface-info oss-border-info rounded-[10px] border px-3 py-2.5">
          <Typography.Text className="oss-text-primary block text-[12px] font-medium">
            OSS 文件夹是一个目录占位对象
          </Typography.Text>
          <Typography.Text className="oss-text-secondary mt-1 block text-[11px] leading-5">
            创建后会写入一个以 / 结尾的 0 字节对象，不会修改 Bucket 配置。
          </Typography.Text>
        </div>
        {errorMessage ? <Alert showIcon message={errorMessage} type="error" /> : null}
        <Form form={form} layout="vertical">
          <Form.Item
            extra={
              <span>
                当前目录：<span className="font-mono">{parentPrefix || "/"}</span>
              </span>
            }
            label="文件夹名称"
            name="name"
            rules={[
              {
                validator: async (_, value: string | undefined) => {
                  const error = validateOssFolderName(value ?? "");
                  if (error) {
                    throw new Error(error);
                  }
                },
              },
            ]}
          >
            <Input
              autoFocus
              maxLength={254}
              placeholder="例如：images"
              showCount
            />
          </Form.Item>
        </Form>
        <div className="oss-surface-muted oss-border rounded-[8px] border px-3 py-2">
          <Typography.Text className="oss-text-tertiary block text-[10px] uppercase tracking-[0.12em]">
            目标 Object Key
          </Typography.Text>
          <Typography.Text className="oss-text-secondary mt-1 block break-all font-mono text-[11px]">
            {objectKey}
          </Typography.Text>
        </div>
      </div>
    </Modal>
  );
}
