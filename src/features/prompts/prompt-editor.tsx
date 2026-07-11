import { CopyOutlined, DeleteOutlined, SaveOutlined } from "@ant-design/icons";
import { Alert, Button, Empty, Input, Popconfirm, Tag, Typography } from "antd";

import {
  countPromptCharacters,
  MAX_PROMPT_CONTENT_CHARS,
  MAX_PROMPT_TITLE_CHARS,
  type PromptDraft,
  type PromptDraftErrors,
} from "@/features/prompts/prompt-model";

type PromptEditorProps = {
  busy: boolean;
  dirty: boolean;
  draft: PromptDraft | null;
  errors: PromptDraftErrors;
  onChange: (draft: PromptDraft) => void;
  onCopy: () => void | Promise<void>;
  onDelete: () => void | Promise<void>;
  onSave: () => void | Promise<void>;
  operationError?: string | null;
  saving: boolean;
};

export function PromptEditor({
  busy,
  dirty,
  draft,
  errors,
  onChange,
  onCopy,
  onDelete,
  onSave,
  operationError,
  saving,
}: PromptEditorProps) {
  if (!draft) {
    return (
      <main className="flex h-full min-h-0 items-center justify-center bg-panel px-6">
        <Empty
          description="从左侧选择一个 Prompt，或创建新的 Prompt"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      </main>
    );
  }

  const titleCount = countPromptCharacters(draft.title.trim());
  const contentCount = countPromptCharacters(draft.content);

  return (
    <main className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-panel">
      <header className="flex min-h-[70px] shrink-0 items-center justify-between gap-4 border-b border-[color:var(--border)] px-5">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Typography.Text className="truncate text-[14px] font-semibold text-foreground">
              {draft.isNew ? "新增 Prompt" : draft.title || "未命名 Prompt"}
            </Typography.Text>
            {draft.isNew ? (
              <Tag color="blue">新草稿</Tag>
            ) : dirty ? (
              <Tag color="gold">未保存</Tag>
            ) : (
              <Tag>已保存</Tag>
            )}
          </div>
          <Typography.Text className="block text-[11px] text-muted-foreground">
            正文将按原样写入剪贴板，包括首尾空白和换行。
          </Typography.Text>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            disabled={busy || !draft.content.trim()}
            icon={<CopyOutlined />}
            onClick={() => void onCopy()}
          >
            一键复制
          </Button>
          <Button
            disabled={busy || (!draft.isNew && !dirty)}
            icon={<SaveOutlined />}
            loading={saving}
            onClick={() => void onSave()}
            type="primary"
          >
            保存
          </Button>
          {!draft.isNew ? (
            <Popconfirm
              cancelText="取消"
              description="删除后无法从 Bexo Studio 恢复，但不会影响已经复制到其他位置的文本。"
              okButtonProps={{ danger: true }}
              okText="删除"
              onConfirm={() => void onDelete()}
              title={`删除“${draft.title}”？`}
            >
              <Button danger disabled={busy} icon={<DeleteOutlined />}>
                删除
              </Button>
            </Popconfirm>
          ) : null}
        </div>
      </header>

      {operationError ? (
        <Alert
          className="mx-5 mt-4 shrink-0"
          description={operationError}
          message="Prompt 操作失败"
          showIcon
          type="error"
        />
      ) : null}

      <section className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden px-5 py-4">
        <div className="shrink-0">
          <div className="mb-1.5 flex items-center justify-between">
            <label className="text-[12px] font-semibold text-foreground" htmlFor="prompt-title">
              标题
            </label>
            <span className="text-[11px] text-muted-foreground">
              {titleCount.toLocaleString()} / {MAX_PROMPT_TITLE_CHARS}
            </span>
          </div>
          <Input
            allowClear
            aria-invalid={Boolean(errors.title)}
            className="!h-[44px] !px-3 !text-[20px]"
            disabled={busy}
            id="prompt-title"
            onChange={(event) => onChange({ ...draft, title: event.target.value })}
            placeholder="例如：生产级代码 Review"
            status={errors.title ? "error" : ""}
            value={draft.title}
          />
          {errors.title ? (
            <Typography.Text className="mt-1 block text-[11px] text-danger">
              {errors.title}
            </Typography.Text>
          ) : null}
        </div>

        <div className="flex min-h-0 flex-1 flex-col">
          <div className="mb-1.5 flex items-center justify-between">
            <label className="text-[12px] font-semibold text-foreground" htmlFor="prompt-content">
              Prompt 内容
            </label>
            <span className="text-[11px] text-muted-foreground">
              {contentCount.toLocaleString()} / {MAX_PROMPT_CONTENT_CHARS.toLocaleString()}
            </span>
          </div>
          <Input.TextArea
            aria-invalid={Boolean(errors.content)}
            autoSize={false}
            className="allow-text-selection !h-full !min-h-[220px] !resize-none font-mono !text-[20px] !leading-8"
            disabled={busy}
            id="prompt-content"
            onChange={(event) => onChange({ ...draft, content: event.target.value })}
            placeholder="输入需要长期复用的完整 Prompt…"
            spellCheck={false}
            status={errors.content ? "error" : ""}
            value={draft.content}
          />
          <div className="mt-1 flex min-h-[18px] items-center justify-between gap-3">
            {errors.content ? (
              <Typography.Text className="text-[11px] text-danger">
                {errors.content}
              </Typography.Text>
            ) : (
              <Typography.Text className="text-[11px] text-muted-foreground">
                支持多行纯文本；使用 Ctrl+S 快速保存。
              </Typography.Text>
            )}
            <Typography.Text className="shrink-0 text-[11px] text-muted-foreground">
              {draft.isNew ? "保存后加入左侧列表" : `ID ${draft.id.slice(0, 8)}`}
            </Typography.Text>
          </div>
        </div>
      </section>
    </main>
  );
}
