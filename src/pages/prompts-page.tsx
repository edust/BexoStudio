import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { App } from "antd";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { PromptEditor } from "@/features/prompts/prompt-editor";
import {
  buildEmptyPromptDraft,
  isPromptDraftDirty,
  promptRecordToDraft,
  validatePromptDraft,
  type PromptDraft,
  type PromptDraftErrors,
} from "@/features/prompts/prompt-model";
import { PromptSidebar } from "@/features/prompts/prompt-sidebar";
import { buildPromptQuickPasteBindingsByPromptId } from "@/features/prompts/prompt-quick-paste";
import { usePromptNavigationGuard } from "@/features/prompts/use-prompt-navigation-guard";
import { copyTextToClipboard, getClipboardErrorMessage } from "@/lib/clipboard";
import { getErrorSummary, hasDesktopRuntime } from "@/lib/command-client";
import {
  deletePrompt,
  listPrompts,
  promptsQueryKey,
  reorderPrompts,
  upsertPrompt,
} from "@/queries/prompts";
import { appPreferencesQueryKey, getAppPreferences } from "@/queries/preferences";
import type { PromptRecord } from "@/types/backend";

export default function PromptsPage() {
  const { modal } = App.useApp();
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sourcePrompt, setSourcePrompt] = useState<PromptRecord | null>(null);
  const [draft, setDraft] = useState<PromptDraft | null>(null);
  const [draftErrors, setDraftErrors] = useState<PromptDraftErrors>({});
  const [operationError, setOperationError] = useState<string | null>(null);

  const promptsQuery = useQuery({
    queryKey: promptsQueryKey,
    queryFn: listPrompts,
    enabled: desktopRuntimeAvailable,
    staleTime: 10_000,
  });
  const preferencesQuery = useQuery({
    queryKey: appPreferencesQueryKey,
    queryFn: getAppPreferences,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const prompts = promptsQuery.data ?? [];
  const quickPasteBindingsByPromptId = useMemo(
    () =>
      buildPromptQuickPasteBindingsByPromptId(
        preferencesQuery.data?.hotkey.promptQuickPasteSlots,
      ),
    [preferencesQuery.data?.hotkey.promptQuickPasteSlots],
  );
  const dirty = isPromptDraftDirty(draft, sourcePrompt);

  useEffect(() => {
    if (draft?.isNew || dirty) {
      return;
    }
    const target = prompts.find((prompt) => prompt.id === selectedId) ?? prompts[0] ?? null;
    if (!target) {
      setSelectedId(null);
      setSourcePrompt(null);
      setDraft(null);
      return;
    }
    if (
      target.id !== selectedId ||
      target.updatedAt !== sourcePrompt?.updatedAt ||
      target.title !== sourcePrompt?.title ||
      target.content !== sourcePrompt?.content
    ) {
      openPrompt(target);
    }
  }, [dirty, draft?.isNew, prompts, selectedId, sourcePrompt]);

  usePromptNavigationGuard(dirty);

  const saveMutation = useMutation({
    mutationFn: upsertPrompt,
    onSuccess: (savedPrompt) => {
      queryClient.setQueryData<PromptRecord[]>(promptsQueryKey, (current = []) => {
        const exists = current.some((prompt) => prompt.id === savedPrompt.id);
        const next = exists
          ? current.map((prompt) => (prompt.id === savedPrompt.id ? savedPrompt : prompt))
          : [...current, savedPrompt];
        return [...next].sort(
          (left, right) => left.sortOrder - right.sortOrder || left.id.localeCompare(right.id),
        );
      });
      setSelectedId(savedPrompt.id);
      setSourcePrompt(savedPrompt);
      setDraft(promptRecordToDraft(savedPrompt));
      setDraftErrors({});
      setOperationError(null);
      toast.success("Prompt 已保存");
    },
    onError: (error) => {
      const message = getErrorSummary(error).message;
      setOperationError(message);
      toast.error("保存 Prompt 失败", { description: message });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deletePrompt,
    onSuccess: (result) => {
      const deletedIndex = prompts.findIndex((prompt) => prompt.id === result.id);
      const remaining = prompts
        .filter((prompt) => prompt.id !== result.id)
        .map((prompt, index) => ({ ...prompt, sortOrder: index }));
      queryClient.setQueryData(promptsQueryKey, remaining);
      const nextPrompt = remaining[Math.min(Math.max(0, deletedIndex), remaining.length - 1)] ?? null;
      if (nextPrompt) {
        openPrompt(nextPrompt);
      } else {
        setSelectedId(null);
        setSourcePrompt(null);
        setDraft(null);
      }
      setOperationError(null);
      toast.success("Prompt 已删除");
    },
    onError: (error) => {
      const message = getErrorSummary(error).message;
      setOperationError(message);
      toast.error("删除 Prompt 失败", { description: message });
    },
  });

  const reorderMutation = useMutation({
    mutationFn: (nextPrompts: PromptRecord[]) =>
      reorderPrompts({ promptIds: nextPrompts.map((prompt) => prompt.id) }),
    onMutate: async (nextPrompts) => {
      await queryClient.cancelQueries({ queryKey: promptsQueryKey });
      const previousPrompts = queryClient.getQueryData<PromptRecord[]>(promptsQueryKey);
      queryClient.setQueryData(promptsQueryKey, nextPrompts);
      return { previousPrompts };
    },
    onSuccess: (orderedPrompts) => {
      queryClient.setQueryData(promptsQueryKey, orderedPrompts);
      toast.success("Prompt 顺序已更新");
    },
    onError: (error, _nextPrompts, context) => {
      if (context?.previousPrompts) {
        queryClient.setQueryData(promptsQueryKey, context.previousPrompts);
      }
      toast.error("更新 Prompt 顺序失败", {
        description: getErrorSummary(error).message,
      });
    },
  });

  const busy = saveMutation.isPending || deleteMutation.isPending || reorderMutation.isPending;

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "s") {
        return;
      }
      if (!draft || busy || (!draft.isNew && !dirty)) {
        return;
      }
      event.preventDefault();
      void handleSave();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, dirty, draft]);

  function openPrompt(prompt: PromptRecord) {
    setSelectedId(prompt.id);
    setSourcePrompt(prompt);
    setDraft(promptRecordToDraft(prompt));
    setDraftErrors({});
    setOperationError(null);
  }

  function requestTransition(action: () => void) {
    if (!dirty) {
      action();
      return;
    }
    modal.confirm({
      title: "放弃未保存的修改？",
      content: "切换后，当前 Prompt 的未保存内容将丢失。",
      okText: "放弃修改",
      cancelText: "继续编辑",
      okButtonProps: { danger: true },
      onOk: action,
    });
  }

  function handleCreate() {
    requestTransition(() => {
      try {
        setSelectedId(null);
        setSourcePrompt(null);
        setDraft(buildEmptyPromptDraft());
        setDraftErrors({});
        setOperationError(null);
      } catch (error) {
        toast.error("创建 Prompt 草稿失败", {
          description: error instanceof Error ? error.message : "无法生成 Prompt 标识",
        });
      }
    });
  }

  function handleSelect(prompt: PromptRecord) {
    if (prompt.id === selectedId && !draft?.isNew) {
      return;
    }
    requestTransition(() => openPrompt(prompt));
  }

  function handleDraftChange(nextDraft: PromptDraft) {
    const validation = validatePromptDraft(nextDraft);
    const nextValidationErrors = validation.errors;
    setDraft(nextDraft);
    setDraftErrors((current) => ({
      title:
        nextDraft.title !== draft?.title ? nextValidationErrors.title : current.title,
      content:
        nextDraft.content !== draft?.content ? nextValidationErrors.content : current.content,
    }));
    setOperationError(null);
  }

  function handleSave() {
    if (!draft) {
      return;
    }
    if (!desktopRuntimeAvailable) {
      toast.error("请在 Bexo Studio 桌面应用中保存 Prompt");
      return;
    }
    const validation = validatePromptDraft(draft);
    setDraftErrors(validation.errors);
    if (!validation.ok) {
      toast.error("请修正 Prompt 表单中的错误");
      return;
    }
    setOperationError(null);
    saveMutation.mutate(validation.input);
  }

  function handleDelete() {
    if (!draft || draft.isNew) {
      return;
    }
    deleteMutation.mutate(draft.id);
  }

  async function handleCopy(prompt: Pick<PromptRecord, "title" | "content">) {
    try {
      await copyTextToClipboard(prompt.content, "Prompt 内容为空，无法复制");
      toast.success("Prompt 已复制到剪贴板", { description: prompt.title });
    } catch (error) {
      toast.error("复制 Prompt 失败", {
        description: getClipboardErrorMessage(error),
      });
    }
  }

  const queryErrorMessage = promptsQuery.error
    ? getErrorSummary(promptsQuery.error).message
    : null;
  const pageBusy = busy || promptsQuery.isLoading;
  return (
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] overflow-hidden bg-panel">
      <PromptSidebar
        busy={pageBusy}
        desktopRuntimeAvailable={desktopRuntimeAvailable}
        errorMessage={queryErrorMessage}
        loading={promptsQuery.isLoading}
        onCopy={handleCopy}
        onCreate={handleCreate}
        onRefresh={async () => {
          await promptsQuery.refetch();
        }}
        onReorder={async (nextPrompts) => {
          await reorderMutation.mutateAsync(nextPrompts);
        }}
        onSelect={handleSelect}
        prompts={prompts}
        quickPasteBindingsByPromptId={quickPasteBindingsByPromptId}
        refreshing={promptsQuery.isFetching}
        selectedId={selectedId}
      />
      <PromptEditor
        busy={busy}
        dirty={dirty}
        draft={draft}
        errors={draftErrors}
        onChange={handleDraftChange}
        onCopy={() => (draft ? handleCopy(draft) : Promise.resolve())}
        onDelete={handleDelete}
        onSave={handleSave}
        operationError={operationError}
        saving={saveMutation.isPending}
      />
    </div>
  );
}
