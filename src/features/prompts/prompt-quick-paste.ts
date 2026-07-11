import type { PromptQuickPasteHotkeySlot } from "@/types/backend";

export const PROMPT_QUICK_PASTE_SLOT_COUNT = 5;

export function defaultPromptQuickPasteShortcut(slot: number) {
  return `Ctrl+Alt+Shift+${slot}`;
}

export function normalizePromptQuickPasteSlots(
  slots: PromptQuickPasteHotkeySlot[] | null | undefined,
) {
  const candidates = Array.isArray(slots) ? slots : [];
  return Array.from({ length: PROMPT_QUICK_PASTE_SLOT_COUNT }, (_, index) => {
    const slot = index + 1;
    const candidate = candidates.find((item) => item.slot === slot);
    const promptId = candidate?.promptId?.trim() || null;
    return {
      slot,
      enabled: Boolean(candidate?.enabled && promptId),
      promptId,
      shortcut: candidate?.shortcut?.trim() || defaultPromptQuickPasteShortcut(slot),
    } satisfies PromptQuickPasteHotkeySlot;
  });
}

export type PromptQuickPasteBinding = {
  slot: number;
  shortcut: string;
};

export function buildPromptQuickPasteBindingsByPromptId(
  slots: PromptQuickPasteHotkeySlot[] | null | undefined,
) {
  const bindings = new Map<string, PromptQuickPasteBinding[]>();
  for (const slot of normalizePromptQuickPasteSlots(slots)) {
    if (!slot.enabled || !slot.promptId) {
      continue;
    }
    const current = bindings.get(slot.promptId) ?? [];
    current.push({ slot: slot.slot, shortcut: slot.shortcut });
    bindings.set(slot.promptId, current);
  }
  return bindings;
}
