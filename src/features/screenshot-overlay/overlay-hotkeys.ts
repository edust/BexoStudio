export type ScreenshotToolKind =
  | "select"
  | "line"
  | "rect"
  | "ellipse"
  | "arrow"
  | "pen"
  | "text"
  | "number"
  | "fill"
  | "mosaic"
  | "blur";

export type OverlayKeyboardEventLike = Pick<
  KeyboardEvent,
  "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey"
>;

export const FIXED_SCREENSHOT_TOOL_HOTKEYS: ReadonlyArray<
  readonly [string, ScreenshotToolKind]
> = [
  ["1", "select"],
  ["2", "line"],
  ["3", "rect"],
  ["4", "ellipse"],
  ["5", "arrow"],
  ["6", "pen"],
  ["7", "text"],
  ["8", "fill"],
  ["9", "mosaic"],
  ["0", "blur"],
  ["n", "number"],
];

export function buildToolHotkeyMap(): ReadonlyMap<string, ScreenshotToolKind> {
  const map = new Map<string, ScreenshotToolKind>();
  for (const [shortcut, tool] of FIXED_SCREENSHOT_TOOL_HOTKEYS) {
    const normalized = normalizeOverlayShortcut(shortcut);
    if (normalized) {
      map.set(normalized, tool);
    }
  }
  return map;
}

export function resolveToolHotkeyFromKeyboardEvent(
  event: OverlayKeyboardEventLike,
  toolHotkeyMap: ReadonlyMap<string, ScreenshotToolKind>,
): ScreenshotToolKind | null {
  const normalized = normalizeOverlayKeyboardEvent(event);
  return normalized ? (toolHotkeyMap.get(normalized) ?? null) : null;
}

export function normalizeOverlayShortcut(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const modifiers = new Set<string>();
  let key: string | null = null;
  for (const part of trimmed.split("+")) {
    const token = normalizeOverlayShortcutToken(part);
    if (!token) {
      return null;
    }
    if (isOverlayModifierToken(token)) {
      modifiers.add(token);
      continue;
    }
    if (key) {
      return null;
    }
    key = token;
  }

  return key || modifiers.size > 0 ? formatOverlayHotkey(modifiers, key) : null;
}

export function normalizeOverlayKeyboardEvent(
  event: OverlayKeyboardEventLike,
): string | null {
  const modifiers = new Set<string>();
  if (event.ctrlKey) modifiers.add("ctrl");
  if (event.altKey) modifiers.add("alt");
  if (event.shiftKey) modifiers.add("shift");
  if (event.metaKey) modifiers.add("super");

  const keyToken = normalizeOverlayKeyboardKey(event.key);
  if (!keyToken) {
    return null;
  }
  if (isOverlayModifierToken(keyToken)) {
    modifiers.add(keyToken);
    return formatOverlayHotkey(modifiers, null);
  }
  return formatOverlayHotkey(modifiers, keyToken);
}

function formatOverlayHotkey(modifiers: ReadonlySet<string>, key: string | null): string {
  const parts: string[] = [];
  for (const token of ["ctrl", "alt", "shift", "super"]) {
    if (modifiers.has(token)) {
      parts.push(token);
    }
  }
  if (key) {
    parts.push(key);
  }
  return parts.join("+");
}

function normalizeOverlayKeyboardKey(input: string): string | null {
  const raw = input.toLowerCase();
  const normalized = raw === " " ? raw : raw.trim();
  if (!normalized) return null;
  if (/^[a-z0-9]$/.test(normalized)) return normalized;
  if (/^f([1-9]|1[0-9]|2[0-4])$/.test(normalized)) return normalized;

  switch (normalized) {
    case "control":
    case "ctrl":
      return "ctrl";
    case "alt":
    case "altgraph":
      return "alt";
    case "shift":
      return "shift";
    case "meta":
    case "super":
    case "os":
      return "super";
    case " ":
    case "spacebar":
      return "space";
    case "tab":
    case "enter":
    case "backspace":
    case "delete":
    case "escape":
    case "arrowup":
    case "arrowdown":
    case "arrowleft":
    case "arrowright":
      return normalized;
    case "return":
      return "enter";
    case "del":
      return "delete";
    case "esc":
      return "escape";
    default:
      return null;
  }
}

function normalizeOverlayShortcutToken(input: string): string | null {
  const normalized = input.trim().toLowerCase();
  if (!normalized) return null;
  if (/^[a-z0-9]$/.test(normalized)) return normalized;
  if (/^f([1-9]|1[0-9]|2[0-4])$/.test(normalized)) return normalized;

  if (["ctrl", "control", "lctrl", "leftctrl", "leftcontrol", "rctrl", "rightctrl", "rightcontrol"].includes(normalized)) {
    return "ctrl";
  }
  if (["alt", "lalt", "leftalt", "ralt", "rightalt", "altgraph"].includes(normalized)) {
    return "alt";
  }
  if (["shift", "lshift", "leftshift", "rshift", "rightshift"].includes(normalized)) {
    return "shift";
  }
  if (["super", "meta", "win", "windows", "lwin", "leftwin", "leftwindows", "rwin", "rightwin", "rightwindows"].includes(normalized)) {
    return "super";
  }

  switch (normalized) {
    case "space":
    case "tab":
    case "enter":
    case "backspace":
    case "delete":
    case "escape":
    case "arrowup":
    case "arrowdown":
    case "arrowleft":
    case "arrowright":
      return normalized;
    case "return":
      return "enter";
    case "del":
      return "delete";
    case "esc":
      return "escape";
    default:
      return null;
  }
}

function isOverlayModifierToken(token: string): boolean {
  return token === "ctrl" || token === "alt" || token === "shift" || token === "super";
}
