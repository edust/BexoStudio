import { Form, Input, Modal, Typography } from "antd";
import { useEffect } from "react";

import type { OssAccountSummary, UpsertOssAccountPayload } from "@/types/backend";

type OssAccountDialogProps = {
  open: boolean;
  account?: OssAccountSummary | null;
  confirmLoading?: boolean;
  onCancel: () => void;
  onSubmit: (input: UpsertOssAccountPayload) => Promise<void>;
};

type AccountFormValues = {
  displayName: string;
  accessKeyId: string;
  accessKeySecret?: string;
};

export function OssAccountDialog({
  open,
  account,
  confirmLoading = false,
  onCancel,
  onSubmit,
}: OssAccountDialogProps) {
  const [form] = Form.useForm<AccountFormValues>();

  useEffect(() => {
    if (!open) {
      return;
    }
    form.setFieldsValue({
      displayName: account?.displayName ?? "",
      accessKeyId: "",
      accessKeySecret: "",
    });
  }, [account, form, open]);

  async function handleOk() {
    const values = await form.validateFields();
    await onSubmit({
      id: account?.id,
      displayName: values.displayName.trim(),
      accessKeyId: values.accessKeyId.trim(),
      accessKeySecret: values.accessKeySecret?.trim() || undefined,
    });
  }

  return (
    <Modal
      cancelText="取消"
      confirmLoading={confirmLoading}
      okText={account ? "保存账号" : "添加账号"}
      onCancel={onCancel}
      onOk={() => void handleOk()}
      open={open}
      title={account ? "编辑 OSS 账号" : "添加 OSS 账号"}
      width={520}
    >
      <div className="space-y-4 pt-1">
        <div className="oss-surface-info oss-border-info rounded-[10px] border px-3 py-2.5">
          <Typography.Text className="oss-text-info block text-[12px] font-medium">
            仅保存 RAM AccessKey
          </Typography.Text>
          <Typography.Text className="oss-text-secondary mt-1 block text-[11px] leading-5">
            AccessKey Secret 会写入 Windows Credential Manager，不会进入 SQLite、日志或前端状态。
          </Typography.Text>
        </div>

        <Form form={form} layout="vertical">
          <Form.Item
            label="账号备注"
            name="displayName"
            rules={[
              { required: true, message: "请输入账号备注" },
              { max: 80, message: "账号备注不能超过 80 个字符" },
            ]}
          >
            <Input autoFocus placeholder="例如：生产 OSS、活动素材账号" />
          </Form.Item>
          <Form.Item
            extra={account ? "编辑时请重新输入完整 AccessKey ID；Secret 留空则沿用已保存的 Secret。" : undefined}
            label="AccessKey ID"
            name="accessKeyId"
            rules={[{ required: true, message: "请输入 AccessKey ID" }]}
          >
            <Input className="font-mono" placeholder="LTAI..." />
          </Form.Item>
          <Form.Item
            extra={account ? "留空表示不更换已有 Secret。" : "Secret 只在本次保存时提交给 Rust。"}
            label="AccessKey Secret"
            name="accessKeySecret"
            rules={[
              {
                validator: async (_, value: string | undefined) => {
                  if (account && !value) {
                    return;
                  }
                  if (!value || value.length > 256) {
                    throw new Error("请输入有效的 AccessKey Secret");
                  }
                },
              },
            ]}
          >
            <Input.Password className="font-mono" placeholder={account ? "留空保留当前 Secret" : "请输入 Secret"} />
          </Form.Item>
        </Form>
      </div>
    </Modal>
  );
}
