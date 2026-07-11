import { useEffect, useRef } from "react";

type UseHotkeyRecorderOptions = {
  active: boolean;
  onCancel: () => void;
  onComplete: (shortcut: string) => void | Promise<void>;
  onError: (message: string) => void;
  onPreviewChange: (shortcut: string) => void;
};

const HOTKEY_MODIFIER_TOKENS = new Set([
  "Ctrl",
  "Alt",
  "Shift",
  "Super",
  "LCtrl",
  "RCtrl",
  "LAlt",
  "RAlt",
  "LWin",
  "RWin",
  "LShift",
  "RShift",
]);

export function useHotkeyRecorder(options: UseHotkeyRecorderOptions) {
  const handlersRef = useRef(options);
  handlersRef.current = options;

  useEffect(() => {
    if (!options.active) {
      return;
    }

    const activeTokens = new Set<string>();
    const seenTokens = new Set<string>();
    let unsupportedKey: string | null = null;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) {
        event.preventDefault();
        return;
      }
      if (event.code === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        handlersRef.current.onCancel();
        return;
      }

      const token = keyboardEventToHotkeyToken(event);
      event.preventDefault();
      event.stopPropagation();
      if (!token) {
        unsupportedKey = event.code;
        handlersRef.current.onError(
          `不支持的按键：${event.code}（支持 A-Z、0-9、F1-F24、Space/Tab/Enter/Backspace/Delete 与左右修饰键）。`,
        );
        return;
      }

      unsupportedKey = null;
      activeTokens.add(token);
      seenTokens.add(token);
      handlersRef.current.onPreviewChange(formatHotkeyTokens(activeTokens));
    };

    const handleKeyUp = (event: KeyboardEvent) => {
      const token = keyboardEventToHotkeyToken(event);
      if (!token) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      activeTokens.delete(token);
      handlersRef.current.onPreviewChange(formatHotkeyTokens(activeTokens));

      if (activeTokens.size !== 0 || seenTokens.size === 0) {
        return;
      }
      if (unsupportedKey) {
        handlersRef.current.onError(`包含不支持的按键：${unsupportedKey}，请重新录制。`);
        return;
      }

      const shortcut = formatHotkeyTokens(seenTokens);
      if (!shortcut.split("+").some((item) => !isHotkeyModifierToken(item))) {
        handlersRef.current.onError("快捷键至少需要包含一个字母、数字或功能键。");
        return;
      }
      void handlersRef.current.onComplete(shortcut);
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
    };
  }, [options.active]);
}

function keyboardEventToHotkeyToken(event: KeyboardEvent): string | null {
  switch (event.code) {
    case "ControlLeft":
      return "LCtrl";
    case "ControlRight":
      return "RCtrl";
    case "AltLeft":
      return "LAlt";
    case "AltRight":
      return "RAlt";
    case "ShiftLeft":
      return "LShift";
    case "ShiftRight":
      return "RShift";
    case "MetaLeft":
      return "LWin";
    case "MetaRight":
      return "RWin";
    case "Space":
      return "Space";
    case "Tab":
      return "Tab";
    case "Enter":
      return "Enter";
    case "Backspace":
      return "Backspace";
    case "Delete":
      return "Delete";
    default:
      break;
  }

  if (event.code.startsWith("Key") && event.code.length === 4) {
    return event.code.slice(3);
  }
  if (event.code.startsWith("Digit") && event.code.length === 6) {
    return event.code.slice(5);
  }
  return /^F([1-9]|1[0-9]|2[0-4])$/.test(event.code) ? event.code : null;
}

function formatHotkeyTokens(tokens: Iterable<string>) {
  return Array.from(new Set(tokens))
    .sort((left, right) => hotkeyTokenOrder(left) - hotkeyTokenOrder(right))
    .join("+");
}

function hotkeyTokenOrder(token: string) {
  const fixedOrder: Record<string, number> = {
    Ctrl: 10,
    LCtrl: 11,
    RCtrl: 12,
    Alt: 20,
    LAlt: 21,
    RAlt: 22,
    Shift: 30,
    LShift: 31,
    RShift: 32,
    Super: 40,
    LWin: 41,
    RWin: 42,
    Space: 100,
    Tab: 101,
    Enter: 102,
    Backspace: 103,
    Delete: 104,
  };
  const fixed = fixedOrder[token];
  if (typeof fixed === "number") {
    return fixed;
  }
  if (/^[A-Z]$/.test(token)) {
    return 100 + token.charCodeAt(0);
  }
  if (/^[0-9]$/.test(token)) {
    return 200 + token.charCodeAt(0);
  }
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(token)) {
    return 300 + Number.parseInt(token.slice(1), 10);
  }
  return 999;
}

function isHotkeyModifierToken(token: string) {
  return HOTKEY_MODIFIER_TOKENS.has(token);
}
