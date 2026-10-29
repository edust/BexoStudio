import { CopyOutlined, DeleteOutlined, SaveOutlined } from "@ant-design/icons";
import { Alert, Button, Empty, Input, Popconfirm, Tooltip } from "antd";

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
      <main className="flex h-full min-h-0 items-center justify-center bg-surface px-6">
        <Empty description="从左侧选择一个 Prompt，或点击 + 新建" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      </main>
    );
  }

  const titleCount = countPromptCharacters(draft.title.trim());
  const contentCount = countPromptCharacters(draft.content);
  const status = draft.isNew ? "新草稿" : dirty ? "未保存" : "已保存";

  return (
    <main className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-surface">
      <header className="flex h-10 shrink-0 items-center gap-2 border-b border-line px-3">
        <div className="flex min-w-0 flex-1 items-baseline gap-2">
          <h2 className="m-0 truncate text-[13px] font-semibold text-fg">
            {draft.isNew ? "新增 Prompt" : draft.title || "未命名 Prompt"}
          </h2>
          <span className={dirty || draft.isNew ? "shrink-0 text-[12px] text-warning" : "shrink-0 text-[12px] text-fg-3"}>
            {status}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Tooltip title="按原样复制正文（含首尾空白与换行）">
            <Button
              disabled={busy || !draft.content.trim()}
              icon={<CopyOutlined />}
              onClick={() => void onCopy()}
              size="small"
            >
              复制
            </Button>
          </Tooltip>
          <Tooltip title="Ctrl+S">
            <Button
              disabled={busy || (!draft.isNew && !dirty)}
              icon={<SaveOutlined />}
              loading={saving}
              onClick={() => void onSave()}
              size="small"
              type="primary"
            >
              保存
            </Button>
          </Tooltip>
          {!draft.isNew ? (
            <Popconfirm
              cancelText="取消"
              description="删除后无法恢复，已复制到其他位置的文本不受影响。"
              okButtonProps={{ danger: true }}
              okText="删除"
              onConfirm={() => void onDelete()}
              title={`删除“${draft.title}”？`}
            >
              <Tooltip title="删除">
                <Button aria-label="删除 Prompt" danger disabled={busy} icon={<DeleteOutlined />} size="small" type="text" />
              </Tooltip>
            </Popconfirm>
          ) : null}
        </div>
      </header>

      {operationError ? (
        <Alert banner description={operationError} message="Prompt 操作失败" showIcon type="error" />
      ) : null}

      <section className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden p-3">
        <div className="shrink-0">
          <div className="mb-1 flex items-center justify-between text-[12px]">
            <label className="text-fg-2" htmlFor="prompt-title">标题</label>
            <span className="text-fg-3">{titleCount.toLocaleString()} / {MAX_PROMPT_TITLE_CHARS}</span>
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
          {errors.title ? <div className="mt-1 text-[12px] text-danger">{errors.title}</div> : null}
        </div>

        <div className="flex min-h-0 flex-1 flex-col">
          <div className="mb-1 flex items-center justify-between text-[12px]">
            <label className="text-fg-2" htmlFor="prompt-content">内容</label>
            <span className="text-fg-3">
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
          {errors.content ? <div className="mt-1 text-[12px] text-danger">{errors.content}</div> : null}
        </div>
      </section>
    </main>
  );
}
