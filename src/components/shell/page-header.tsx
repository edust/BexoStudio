import { Button, Space, Tag, Typography } from "antd";
import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

type PageHeaderAction = {
  id: string;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  variant?: "default" | "secondary" | "ghost" | "danger";
  icon?: ReactNode;
};

type PageHeaderProps = {
  eyebrow: string;
  title: string;
  description: string;
  status?: {
    label: string;
    tone?: "default" | "success" | "warning" | "danger";
  };
  actions?: PageHeaderAction[];
  className?: string;
};

export function PageHeader({
  actions,
  className,
  description,
  eyebrow,
  status,
  title,
}: PageHeaderProps) {
  return (
    <header className={cn("flex flex-col gap-3 border-b border-[#e6edf5] pb-3 lg:flex-row lg:items-start lg:justify-between", className)}>
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Typography.Text className="text-[11px] font-semibold uppercase tracking-[0.22em] text-[#1697c5]">
            {eyebrow}
          </Typography.Text>
          {status ? (
            <Tag bordered={false} className={cn("m-0 rounded-full px-2 py-0 text-[10px] font-semibold uppercase tracking-[0.12em]", toneClassName(status.tone))}>
              {status.label}
            </Tag>
          ) : null}
        </div>
        <div>
          <Typography.Title className="!mb-1 !text-[30px] !font-semibold !tracking-[-0.04em] !text-[#1f2937]" level={1}>
            {title}
          </Typography.Title>
          <Typography.Paragraph className="!mb-0 !max-w-[920px] !text-[13px] !leading-6 !text-[#667085]">
            {description}
          </Typography.Paragraph>
        </div>
      </div>

      {actions?.length ? (
        <Space size={8} wrap>
          {actions.map((action) => (
            <Button
              danger={action.variant === "danger"}
              disabled={action.disabled}
              icon={action.icon}
              key={action.id}
              onClick={action.onClick}
              type={buttonType(action.variant)}
            >
              {action.label}
            </Button>
          ))}
        </Space>
      ) : null}
    </header>
  );
}

function buttonType(variant?: PageHeaderAction["variant"]) {
  if (variant === "ghost") return "text";
  if (variant === "secondary") return "default";
  if (variant === "danger") return "primary";
  return "primary";
}

function toneClassName(tone?: "default" | "success" | "warning" | "danger") {
  if (tone === "success") return "bg-[#eaf8ef] text-[#238657]";
  if (tone === "warning") return "bg-[#fff5e8] text-[#c27a0a]";
  if (tone === "danger") return "bg-[#fdeeed] text-[#c45245]";
  return "bg-[#eef6fb] text-[#1283ab]";
}
