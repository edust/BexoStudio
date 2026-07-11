import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { AsyncDisposerGroup } from "../../src/features/screenshot-overlay/async-disposer-group.ts";
import {
  buildToolHotkeyMap,
  normalizeOverlayShortcut,
  resolveToolHotkeyFromKeyboardEvent,
} from "../../src/features/screenshot-overlay/overlay-hotkeys.ts";
import {
  areNativeInteractionStatesEqual,
  buildNativeInteractionRuntimeRequestKey,
} from "../../src/features/screenshot-overlay/native-runtime-state.ts";

test("overlay tool hotkeys preserve the production mapping", () => {
  const hotkeys = buildToolHotkeyMap();
  assert.equal(hotkeys.get("1"), "select");
  assert.equal(hotkeys.get("8"), "fill");
  assert.equal(hotkeys.get("9"), "mosaic");
  assert.equal(hotkeys.get("0"), "blur");
  assert.equal(hotkeys.get("n"), "number");
});

test("overlay shortcut normalization rejects ambiguous keys", () => {
  assert.equal(normalizeOverlayShortcut("LCtrl + Shift + X"), "ctrl+shift+x");
  assert.equal(normalizeOverlayShortcut("Ctrl+X+Y"), null);
  assert.equal(normalizeOverlayShortcut("Ctrl+Unknown"), null);
});

test("keyboard events resolve against normalized tool hotkeys", () => {
  const resolved = resolveToolHotkeyFromKeyboardEvent(
    { key: "N", ctrlKey: false, altKey: false, shiftKey: false, metaKey: false },
    buildToolHotkeyMap(),
  );
  assert.equal(resolved, "number");
  assert.equal(
    resolveToolHotkeyFromKeyboardEvent(
      { key: " ", ctrlKey: false, altKey: false, shiftKey: false, metaKey: false },
      new Map([["space", "select"]]),
    ),
    "select",
  );
});

test("disposer group cleans successful partial setup in reverse order", async () => {
  const calls = [];
  const group = new AsyncDisposerGroup();
  await group.attach(async () => () => calls.push("first"));
  await group.attach(async () => () => calls.push("second"));

  group.dispose();
  group.dispose();
  assert.deepEqual(calls, ["second", "first"]);
});

test("disposer group immediately cleans a registration resolved after disposal", async () => {
  let resolveRegistration;
  let disposedCount = 0;
  const registration = new Promise((resolve) => {
    resolveRegistration = resolve;
  });
  const group = new AsyncDisposerGroup();
  const attached = group.attach(() => registration);

  group.dispose();
  resolveRegistration(() => {
    disposedCount += 1;
  });

  assert.equal(await attached, false);
  assert.equal(disposedCount, 1);
});

test("caller can dispose partial registrations after a later attach fails", async () => {
  let disposedCount = 0;
  const group = new AsyncDisposerGroup();
  await group.attach(async () => () => {
    disposedCount += 1;
  });

  await assert.rejects(
    group.attach(async () => {
      throw new Error("listener registration failed");
    }),
    /listener registration failed/,
  );
  group.dispose();
  assert.equal(disposedCount, 1);
});

test("native state equality tolerates sub-millipixel geometry noise", () => {
  const base = {
    backendKind: "windows_layered_selection_mvp",
    lifecycleState: "visible",
    hasActiveSession: true,
    selection: { x: 10, y: 20, width: 300, height: 200 },
    activeShape: null,
    activeShapeDraft: null,
    hoveredHitRegion: "none",
    dragMode: null,
    selectionRevision: 4,
    activeShapeRevision: 2,
    interactionMode: "selection",
    rectDraft: null,
  };
  const noisy = {
    ...base,
    selection: { ...base.selection, x: base.selection.x + 0.0004 },
  };

  assert.equal(areNativeInteractionStatesEqual(base, noisy), true);
  assert.equal(
    areNativeInteractionStatesEqual(base, {
      ...noisy,
      selectionRevision: base.selectionRevision + 1,
    }),
    false,
  );
});

test("runtime request key deduplicates geometry noise but never crosses sessions", () => {
  const request = {
    sessionId: "session-a",
    visible: true,
    exclusionRects: [{ x: 1, y: 2, width: 3, height: 4 }],
    mode: "selection",
    selection: { x: 10.1234, y: 20, width: 300, height: 200 },
    activeShape: null,
    shapeCandidates: [],
    color: "#ffffff",
    strokeWidth: 2,
  };
  const baseKey = buildNativeInteractionRuntimeRequestKey(request);
  const noisyKey = buildNativeInteractionRuntimeRequestKey({
    ...request,
    selection: { ...request.selection, x: 10.12341 },
  });
  const nextSessionKey = buildNativeInteractionRuntimeRequestKey({
    ...request,
    sessionId: "session-b",
  });

  assert.equal(baseKey, noisyKey);
  assert.notEqual(baseKey, nextSessionKey);
});

test("auxiliary window capabilities exclude Codex Auth secrets and system control", () => {
  const mainCapability = JSON.parse(
    readFileSync("src-tauri/capabilities/default.json", "utf8"),
  );
  const overlayCapability = JSON.parse(
    readFileSync("src-tauri/capabilities/screenshot-overlay.json", "utf8"),
  );
  const historyCapability = JSON.parse(
    readFileSync("src-tauri/capabilities/codex-history.json", "utf8"),
  );
  const permissionSets = readFileSync(
    "src-tauri/permissions/window-commands.toml",
    "utf8",
  );

  assert.deepEqual(mainCapability.windows, ["main"]);
  assert.deepEqual(overlayCapability.windows, ["screenshot_overlay"]);
  assert.deepEqual(historyCapability.windows, ["codex_history_*"]);

  const overlaySet = extractPermissionSet(permissionSets, "screenshot-overlay-commands");
  const historySet = extractPermissionSet(permissionSets, "codex-history-window-commands");
  for (const restrictedSet of [overlaySet, historySet]) {
    assert.doesNotMatch(restrictedSet, /codex-auth|restore-run|workspace-terminal/);
  }
  assert.match(overlaySet, /allow-get-screenshot-session/);
  assert.match(historySet, /allow-get-codex-history-messages/);
});

test("Tauri command manifest, invoke handler, and main permission set stay identical", () => {
  const buildScript = readFileSync("src-tauri/build.rs", "utf8");
  const appModule = readFileSync("src-tauri/src/app/mod.rs", "utf8");
  const permissionSets = readFileSync(
    "src-tauri/permissions/window-commands.toml",
    "utf8",
  );

  const buildBlock = buildScript.match(/APP_COMMANDS:[\s\S]*?= &\[([\s\S]*?)\];/);
  const handlerBlock = appModule.match(
    /\.invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\]\)/,
  );
  assert.ok(buildBlock, "missing APP_COMMANDS manifest");
  assert.ok(handlerBlock, "missing Tauri invoke handler");

  const manifestCommands = [...buildBlock[1].matchAll(/"([a-z0-9_]+)"/g)].map(
    (match) => match[1],
  );
  const handlerCommands = [
    ...handlerBlock[1].matchAll(/commands::[a-z0-9_]+::([a-z0-9_]+)/g),
  ].map((match) => match[1]);
  const mainSet = extractPermissionSet(permissionSets, "main-window-commands");
  const permittedCommands = [...mainSet.matchAll(/"allow-([a-z0-9-]+)"/g)].map(
    (match) => match[1].replaceAll("-", "_"),
  );

  assert.deepEqual([...manifestCommands].sort(), [...handlerCommands].sort());
  assert.deepEqual([...manifestCommands].sort(), [...permittedCommands].sort());
  assert.equal(new Set(manifestCommands).size, manifestCommands.length);
});

test("Tauri CSP is enabled and permits only the owned preview image protocol", () => {
  const tauriConfig = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  const appModule = readFileSync("src-tauri/src/app/mod.rs", "utf8");
  const csp = tauriConfig.app.security.csp;
  const devCsp = tauriConfig.app.security.devCsp;
  assert.equal(typeof csp, "object");
  assert.equal(csp["object-src"], "'none'");
  assert.match(csp["img-src"], /bexo-preview:/);
  assert.doesNotMatch(csp["script-src"], /unsafe-eval|unsafe-inline/);
  assert.doesNotMatch(csp["connect-src"], /localhost:1420/);
  assert.match(devCsp["connect-src"], /localhost:1420/);
  assert.match(
    appModule,
    /context\.webview_label\(\) != SCREENSHOT_OVERLAY_WINDOW_LABEL/,
  );
});

function extractPermissionSet(source, identifier) {
  const escapedIdentifier = identifier.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(
    new RegExp(
      `identifier = "${escapedIdentifier}"([\\s\\S]*?)(?=\\n\\[\\[set\\]\\]|$)`,
    ),
  );
  assert.ok(match, `missing permission set ${identifier}`);
  return match[0];
}
