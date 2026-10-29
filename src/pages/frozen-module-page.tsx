import { Empty } from "antd";

import { PaneHeader } from "@/components/shell/pane-header";

type FrozenModulePageProps = {
  title: string;
  description: string;
};

export function FrozenModulePage({ description, title }: FrozenModulePageProps) {
  return (
    <div className="flex h-full min-h-0 flex-col bg-surface">
      <PaneHeader meta="暂未开放" title={title} />
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <Empty description={description} image={Empty.PRESENTED_IMAGE_SIMPLE} />
      </div>
    </div>
  );
}
