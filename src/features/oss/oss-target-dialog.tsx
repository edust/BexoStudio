import { Form, Input, Modal, Switch, Typography } from "antd";
import { useEffect } from "react";

import type { OssTargetRecord, UpsertOssTargetPayload } from "@/types/backend";

type OssTargetDialogProps = {
  open: boolean;
  accountId: string;
  target?: OssTargetRecord | null;
  confirmLoading?: boolean;
  onCancel: () => void;
  onSubmit: (input: UpsertOssTargetPayload) => Promise<void>;
};

type TargetFormValues = {
  displayName: string;
  bucket: string;
  region: string;
  endpoint: string;
  prefix: string;
  isDefault: boolean;
};

export function OssTargetDialog({
  open,
  accountId,
  target,
  confirmLoading = false,
  onCancel,
  onSubmit,
}: OssTargetDialogProps) {
  const [form] = Form.useForm<TargetFormValues>();

  useEffect(() => {
    if (!open) {
      return;
    }
    form.setFieldsValue({
      displayName: target?.displayName ?? "",
      bucket: target?.bucket ?? "",
      region: target?.region ?? "cn-hangzhou",
      endpoint: target?.endpoint ?? "oss-cn-hangzhou.aliyuncs.com",
      prefix: target?.prefix ?? "",
      isDefault: target?.isDefault ?? false,
    });
  }, [form, open, target]);

  async function handleOk() {
    const values = await form.validateFields();
    await onSubmit({
      id: target?.id,
      accountId,
      displayName: values.displayName.trim(),
      bucket: values.bucket.trim(),
      region: values.region.trim(),
      endpoint: values.endpoint.trim(),
      prefix: values.prefix.trim() || undefined,
      isDefault: values.isDefault,
      sortOrder: target?.sortOrder ?? 0,
    });
  }

  return (
    <Modal
      cancelText="取消"
      confirmLoading={confirmLoading}
      okText={target ? "保存绑定" : "添加 Bucket"}
      onCancel={onCancel}
      onOk={() => void handleOk()}
      open={open}
      title={target ? "编辑 Bucket 绑定" : "添加 Bucket 绑定"}
      width={560}
    >
      <div className="space-y-4 pt-1">
        <div className="oss-surface-muted oss-border rounded-[10px] border px-3 py-2.5">
          <Typography.Text className="oss-text-primary block text-[12px] font-medium">
            只保存本地绑定
          </Typography.Text>
          <Typography.Text className="oss-text-secondary mt-1 block text-[11px] leading-5">
            Bexo Studio 不会创建、删除或修改 Bucket 配置；这里的删除只会移除本地绑定。
          </Typography.Text>
        </div>
        <Form form={form} layout="vertical">
          <Form.Item
            label="绑定名称"
            name="displayName"
            rules={[
              { required: true, message: "请输入绑定名称" },
              { max: 80, message: "绑定名称不能超过 80 个字符" },
            ]}
          >
            <Input autoFocus placeholder="例如：生产素材、活动上传目录" />
          </Form.Item>
          <div className="grid grid-cols-2 gap-3">
            <Form.Item
              label="Bucket"
              name="bucket"
              rules={[{ required: true, message: "请输入 Bucket 名称" }]}
            >
              <Input className="font-mono" placeholder="example-bucket" />
            </Form.Item>
            <Form.Item
              extra="填写 cn-shanghai；oss-cn-shanghai 是 Endpoint，不是 Region。"
              label="Region"
              name="region"
              rules={[{ required: true, message: "请输入 Region" }]}
            >
              <Input className="font-mono" placeholder="cn-hangzhou" />
            </Form.Item>
          </div>
          <Form.Item
            extra="可填写 oss-cn-hangzhou.aliyuncs.com，也可以填写带 https:// 的完整 Endpoint。"
            label="Endpoint"
            name="endpoint"
            rules={[{ required: true, message: "请输入 Endpoint" }]}
          >
            <Input className="font-mono" placeholder="oss-cn-hangzhou.aliyuncs.com" />
          </Form.Item>
          <Form.Item
            extra="可选。填写后只浏览这个前缀及其子目录，例如 assets/。"
            label="根前缀"
            name="prefix"
          >
            <Input className="font-mono" placeholder="assets/" />
          </Form.Item>
          <Form.Item label="设为默认绑定" name="isDefault" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </div>
    </Modal>
  );
}
