import { Alert, Button, Select, Switch, Tooltip, Typography } from "antd";
import { useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import {
  defaultPromptQuickPasteShortcut,
  normalizePromptQuickPasteSlots,
} from "@/features/prompts/prompt-quick-paste";
import { useHotkeyRecorder } from "@/features/settings/use-hotkey-recorder";
import { getErrorSummary } from "@/lib/command-client";
import type { PromptQuickPasteHotkeySlot, PromptRecord } from "@/types/backend";

type PromptQuickPasteSettingsProps = {
  disabled: boolean;
  loadingPrompts: boolean;
  onSave: (slots: PromptQuickPasteHotkeySlot[]) => Promise<void>;
  promptLoadError?: string | null;
  prompts: PromptRecord[];
  registeredCount: number;
  slots: PromptQuickPasteHotkeySlot[];
};

export function PromptQuickPasteSettings({
  disabled,
  loadingPrompts,
  onSave,
  promptLoadError,
  prompts,
  registeredCount,
  slots,
}: PromptQuickPasteSettingsProps) {
  const normalizedSlots = useMemo(() => normalizePromptQuickPasteSlots(slots), [slots]);
  const [recordingSlot, setRecordingSlot] = useState<number | null>(null);
  const [recorderPreview, setRecorderPreview] = useState("");
  const [recorderError, setRecorderError] = useState<string | null>(null);
  const [savingSlot, setSavingSlot] = useState<number | null>(null);
  const savingRef = useRef(false);
  const promptById = useMemo(
    () => new Map(prompts.map((prompt) => [prompt.id, prompt] as const)),
    [prompts],
  );
  const promptOptions = useMemo(
    () =>
      prompts.map((prompt) => ({
        label: prompt.title,
        value: prompt.id,
      })),
    [prompts],
  );

  useHotkeyRecorder({
    active: recordingSlot !== null,
    onCancel: cancelRecording,
    onComplete: async (shortcut) => {
      if (recordingSlot === null) {
        return;
      }
      const slot = recordingSlot;
      setRecordingSlot(null);
      setRecorderPreview("");
      await updateSlot(
        slot,
        { shortcut },
        `槽位 ${slot} 热键已更新为 ${shortcut}`,
      );
    },
    onError: setRecorderError,
    onPreviewChange: (preview) => {
      setRecorderError(null);
      setRecorderPreview(preview);
    },
  });

  function cancelRecording() {
    setRecordingSlot(null);
    setRecorderPreview("");
    setRecorderError(null);
  }

  async function updateSlot(
    slotNumber: number,
    patch: Partial<PromptQuickPasteHotkeySlot>,
    successMessage: string,
  ) {
    if (disabled || savingRef.current) {
      return false;
    }
    savingRef.current = true;
    setSavingSlot(slotNumber);
    setRecorderError(null);
    const nextSlots = normalizedSlots.map((slot) =>
      slot.slot === slotNumber ? { ...slot, ...patch } : slot,
    );
    try {
      await onSave(nextSlots);
      toast.success(successMessage);
      return true;
    } catch (error) {
      const message = getErrorSummary(error).message;
      setRecorderError(message);
      toast.error("Prompt 快速粘贴设置失败", { description: message });
      return false;
    } finally {
      savingRef.current = false;
      setSavingSlot(null);
    }
  }

  return (
    <section aria-label="Prompt 快速粘贴" className="shrink-0 space-y-1.5">
      <header className="flex items-center gap-2 px-1">
        <Tooltip title="绑定后可在 Bexo Studio 后台或托盘状态下，把 Prompt 正文发送到当前光标处。">
          <h3 className="m-0 text-[12px] font-medium text-fg-3">Prompt 快速粘贴</h3>
        </Tooltip>
        <span className="text-[12px] text-fg-3">· 已注册 {registeredCount}</span>
      </header>
      <div className="overflow-hidden rounded-md border border-line bg-surface">

      {promptLoadError ? (
        <Alert
          banner
          className="!rounded-none"
          message={`Prompt 列表读取失败：${promptLoadError}`}
          showIcon
          type="error"
        />
      ) : null}

      {normalizedSlots.map((slot) => {
        const selectedPrompt = slot.promptId ? promptById.get(slot.promptId) : null;
        const missingPrompt = Boolean(slot.promptId && !selectedPrompt && !loadingPrompts);
        const recording = recordingSlot === slot.slot;
        const saving = savingSlot === slot.slot;
        const displayShortcut = recording
          ? recorderPreview || "请按下组合键，松开后保存"
          : slot.shortcut;
        const options = missingPrompt
          ? [
              {
                label: `已删除的 Prompt（${slot.promptId?.slice(0, 8)}）`,
                value: slot.promptId as string,
                disabled: true,
              },
              ...promptOptions,
            ]
          : promptOptions;

        return (
          <div
            className="grid grid-cols-1 items-center gap-x-3 gap-y-2 border-b border-line px-3 py-2 last:border-b-0 lg:grid-cols-[92px_minmax(0,1fr)] xl:grid-cols-[92px_minmax(0,1fr)_auto]"
            key={slot.slot}
          >
            <div className="flex items-center gap-2">
              <Switch
                aria-label={`启用 Prompt 快速粘贴槽位 ${slot.slot}`}
                checked={slot.enabled}
                disabled={disabled || saving || !slot.promptId || missingPrompt}
                loading={saving}
                onChange={(enabled) =>
                  void updateSlot(
                    slot.slot,
                    { enabled },
                    enabled ? `槽位 ${slot.slot} 已启用` : `槽位 ${slot.slot} 已停用`,
                  )
                }
                size="small"
              />
              <Typography.Text className="text-[13px] text-fg">
                槽位 {slot.slot}
              </Typography.Text>
            </div>

            <div className="grid min-w-0 grid-cols-1 gap-2 xl:grid-cols-[minmax(180px,1fr)_minmax(190px,0.8fr)]">
              <Select
                allowClear
                aria-label={`选择槽位 ${slot.slot} 的 Prompt`}
                disabled={disabled || saving}
                loading={loadingPrompts}
                onChange={(promptId?: string) =>
                  void updateSlot(
                    slot.slot,
                    {
                      promptId: promptId ?? null,
                      enabled: Boolean(promptId),
                    },
                    promptId
                      ? `槽位 ${slot.slot} 已绑定 Prompt`
                      : `槽位 ${slot.slot} 已清除绑定`,
                  )
                }
                optionFilterProp="label"
                options={options}
                placeholder="选择 Prompt"
                showSearch
                status={missingPrompt ? "error" : undefined}
                value={slot.promptId ?? undefined}
              />
              <Tooltip title={recording ? "按 Esc 取消录制" : slot.shortcut}>
                <div
                  className={`flex h-[30px] min-w-0 items-center overflow-hidden rounded-md border px-2.5 font-mono text-[12px] ${
                    recording
                      ? "border-primary bg-selected text-primary"
                      : "border-line bg-surface-2 text-fg-2"
                  }`}
                >
                  <span className="truncate">{displayShortcut}</span>
                </div>
              </Tooltip>
              <span
                className={`block text-[12px] xl:col-span-2 ${
                  missingPrompt ? "text-danger" : "text-fg-3"
                }`}
              >
                {missingPrompt
                  ? "绑定的 Prompt 已删除，请重新选择。"
                  : selectedPrompt
                    ? `已绑定：${selectedPrompt.title}`
                    : `默认 ${defaultPromptQuickPasteShortcut(slot.slot)}，选择 Prompt 后自动启用。`}
              </span>
            </div>

            <div className="flex justify-start gap-2 lg:col-start-2 xl:col-start-auto xl:justify-end">
              <Button
                disabled={disabled || saving || (recordingSlot !== null && !recording)}
                onClick={() => {
                  if (recording) {
                    cancelRecording();
                    return;
                  }
                  setRecorderError(null);
                  setRecorderPreview("");
                  setRecordingSlot(slot.slot);
                }}
                type={recording ? "primary" : "default"}
              >
                {recording ? "取消录制" : "录制热键"}
              </Button>
              <Button
                disabled={
                  disabled ||
                  saving ||
                  recordingSlot !== null ||
                  slot.shortcut === defaultPromptQuickPasteShortcut(slot.slot)
                }
                onClick={() =>
                  void updateSlot(
                    slot.slot,
                    { shortcut: defaultPromptQuickPasteShortcut(slot.slot) },
                    `槽位 ${slot.slot} 已恢复默认热键`,
                  )
                }
              >
                恢复默认
              </Button>
            </div>
          </div>
        );
      })}

      {recorderError ? (
        <Alert
          banner
          className="!rounded-none"
          message={recorderError}
          showIcon
          type="error"
        />
      ) : null}
      </div>
    </section>
  );
}
