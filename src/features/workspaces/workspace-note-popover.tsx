import { EditOutlined, FileTextOutlined } from "@ant-design/icons";
import { Button, Input, Modal, Popover, Typography } from "antd";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";

import { getErrorSummary } from "@/lib/command-client";
import {
  countUnicodeCharacters,
  MAX_WORKSPACE_DESCRIPTION_CHARS,
  validateWorkspaceDescription,
} from "@/features/workspaces/workspace-model";

type WorkspaceNotePopoverProps = {
  value?: string | null;
  onSave: (value: string) => Promise<void>;
  loading?: boolean;
  trigger?: ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
};

export function WorkspaceNotePopover({
  value,
  onSave,
  loading = false,
  trigger,
  open: controlledOpen,
  onOpenChange,
}: WorkspaceNotePopoverProps) {
  const normalizedValue = useMemo(() => value?.trim() ?? "", [value]);
  const [internalOpen, setInternalOpen] = useState(false);
  const [draft, setDraft] = useState(normalizedValue);
  const [error, setError] = useState<string | null>(null);
  const previousOpenRef = useRef(false);
  const isControlled = controlledOpen !== undefined;
  const open = controlledOpen ?? internalOpen;
  const characterCount = countUnicodeCharacters(draft);
  const hasUnsavedChanges = draft !== normalizedValue;
  const validationError = validateWorkspaceDescription(draft);
  const canSave = !validationError && !loading;

  function setPopoverOpen(nextOpen: boolean) {
    if (!isControlled) {
      setInternalOpen(nextOpen);
    }
    onOpenChange?.(nextOpen);
  }

  useEffect(() => {
    if (open && !previousOpenRef.current) {
      setDraft(normalizedValue);
      setError(null);
    } else if (!open) {
      setDraft(normalizedValue);
      setError(null);
    }
    previousOpenRef.current = open;
  }, [normalizedValue, open]);

  function openEditor() {
    setDraft(normalizedValue);
    setError(null);
    setPopoverOpen(true);
  }

  function discardDraft() {
    setDraft(normalizedValue);
    setError(null);
    setPopoverOpen(false);
  }

  function requestClose() {
    if (loading) {
      return;
    }

    if (!hasUnsavedChanges) {
      setPopoverOpen(false);
      return;
    }

    Modal.confirm({
      centered: true,
      cancelText: "继续编辑",
      content: "当前备注还没有保存，确定放弃这次修改吗？",
      okButtonProps: { danger: true },
      okText: "放弃修改",
      title: "放弃未保存备注？",
      onOk: discardDraft,
    });
  }

  async function handleSave() {
    if (validationError) {
      setError(validationError);
      return;
    }
    if (loading) {
      return;
    }

    setError(null);
    try {
      await onSave(draft);
      setPopoverOpen(false);
    } catch (saveError) {
      setError(getErrorSummary(saveError).message);
    }
  }

  const editor = (
    <div className="w-[280px] space-y-2.5">
      <div>
        <Typography.Text className="block text-[12px] font-semibold text-[#1f2937]">
          项目备注
        </Typography.Text>
        <Typography.Text className="mt-0.5 block text-[11px] leading-4 text-[#667085]">
          写一句话说明它是做什么的，方便以后找回工作上下文。
        </Typography.Text>
      </div>
      <Input.TextArea
        autoFocus
        aria-label="项目备注"
        autoSize={{ minRows: 3, maxRows: 5 }}
        onChange={(event) => {
          setDraft(event.target.value);
          setError(null);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            requestClose();
          }
          if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
            event.preventDefault();
            void handleSave();
          }
        }}
        placeholder="例如：活动报名系统，负责后端 API 和管理端"
        value={draft}
      />
      <div className="flex items-center justify-between gap-3">
        <Typography.Text
          className={
            characterCount > MAX_WORKSPACE_DESCRIPTION_CHARS
              ? "text-[11px] text-[#cf5a4a]"
              : "text-[11px] text-[#98a2b3]"
          }
        >
          {characterCount}/{MAX_WORKSPACE_DESCRIPTION_CHARS}
        </Typography.Text>
        <div className="flex items-center gap-1.5">
          <Button disabled={loading} onClick={requestClose} size="small">
            取消
          </Button>
          <Button disabled={!canSave} loading={loading} onClick={() => void handleSave()} size="small" type="primary">
            保存
          </Button>
        </div>
      </div>
      {error ? <Typography.Text type="danger" className="block text-[11px] leading-4">{error}</Typography.Text> : null}
    </div>
  );

  return (
    <Popover
      arrow={false}
      content={editor}
      onOpenChange={(nextOpen) => {
        if (nextOpen) {
          openEditor();
        } else {
          requestClose();
        }
      }}
      open={open}
      placement="rightTop"
      trigger="click"
    >
      <span
        onClick={(event) => event.stopPropagation()}
        onPointerDown={(event) => event.stopPropagation()}
      >
        {trigger ?? (
          <Button
            aria-label={normalizedValue ? "编辑项目备注" : "添加项目备注"}
            className={normalizedValue ? "!text-[#1283ab]" : ""}
            icon={normalizedValue ? <FileTextOutlined /> : <EditOutlined />}
            size="small"
            type="text"
          />
        )}
      </span>
    </Popover>
  );
}
