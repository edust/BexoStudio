import { LockOutlined } from "@ant-design/icons";
import { Card, Empty, List, Tag, Typography } from "antd";

import { PageHeader } from "@/components/shell/page-header";

type FrozenModulePageProps = {
  title: string;
  description: string;
};

const parkedNotes = [
  "路由保留，主导航移除",
  "旧表单与业务数据暂时退出主流程",
  "等待新的紧凑框架稳定后再决定是否恢复",
];

export function FrozenModulePage({ description, title }: FrozenModulePageProps) {
  return (
    <div className="space-y-4">
      <PageHeader
        description={description}
        eyebrow="MODULE PARKED"
        status={{ label: "FRAME ONLY", tone: "warning" }}
        title={`${title} 已冻结`}
      />

      <div className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_300px]">
        <Card
          className="bexo-panel-muted rounded-[14px]"
          styles={{ body: { padding: 16 } }}
          title={<span className="text-[13px] font-semibold text-[#1f2937]">空白工作页</span>}
        >
          <div className="flex min-h-[460px] items-center justify-center rounded-[12px] border border-dashed border-[#d9e2ec] bg-white">
            <Empty
              description="当前阶段只保留壳层结构，不继续承载真实业务内容。"
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          </div>
        </Card>

        <Card
          className="bexo-panel-muted rounded-[14px]"
          styles={{ body: { padding: 14 } }}
          title={<span className="text-[13px] font-semibold text-[#1f2937]">冻结说明</span>}
        >
          <Tag bordered={false} className="mb-3 rounded-full bg-[#fff5e8] px-2 py-0 text-[10px] font-semibold uppercase tracking-[0.12em] text-[#c27a0a]">
            parked
          </Tag>
          <List
            dataSource={parkedNotes}
            renderItem={(item) => (
              <List.Item className="border-b-[#eef2f6] px-0 py-3">
                <div className="flex items-start gap-3">
                  <LockOutlined className="mt-0.5 text-[#98a2b3]" />
                  <Typography.Text className="text-[12px] leading-5 text-[#667085]">{item}</Typography.Text>
                </div>
              </List.Item>
            )}
          />
        </Card>
      </div>
    </div>
  );
}
