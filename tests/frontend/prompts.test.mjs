import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { resolveClipboardPayload } from "../../src/lib/clipboard.ts";
import {
  filterPrompts,
  isSamePromptOrder,
  isPromptDraftDirty,
  movePromptBeforeTarget,
  orderPromptsByIds,
  promptRecordToDraft,
  validatePromptDraft,
} from "../../src/features/prompts/prompt-model.ts";

const prompts = [
  {
    id: "11111111-1111-4111-8111-111111111111",
    title: "代码 Review",
    content: "检查逻辑错误与边界条件",
    sortOrder: 0,
    createdAt: "2026-07-10T00:00:00Z",
    updatedAt: "2026-07-10T00:00:00Z",
  },
  {
    id: "22222222-2222-4222-8222-222222222222",
    title: "翻译",
    content: "Translate the following content",
    sortOrder: 1,
    createdAt: "2026-07-10T00:00:01Z",
    updatedAt: "2026-07-10T00:00:01Z",
  },
];

test("Prompt validation trims the title but preserves content exactly", () => {
  const result = validatePromptDraft({
    id: prompts[0].id,
    title: "  标题  ",
    content: "\n  保留正文空白  \n",
    isNew: true,
  });
  assert.equal(result.ok, true);
  assert.equal(result.input.title, "标题");
  assert.equal(result.input.content, "\n  保留正文空白  \n");
});

test("Prompt validation reports required fields", () => {
  const result = validatePromptDraft({
    id: prompts[0].id,
    title: " ",
    content: "\n\t",
    isNew: true,
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.title, /标题/);
  assert.match(result.errors.content, /内容/);
});

test("Prompt validation rejects title controls and NUL body content", () => {
  const invalidTitle = validatePromptDraft({
    id: prompts[0].id,
    title: "两行\n标题",
    content: "正文",
    isNew: true,
  });
  assert.equal(invalidTitle.ok, false);
  assert.match(invalidTitle.errors.title, /控制字符/);

  const invalidContent = validatePromptDraft({
    id: prompts[0].id,
    title: "标题",
    content: "正文\0内容",
    isNew: true,
  });
  assert.equal(invalidContent.ok, false);
  assert.match(invalidContent.errors.content, /NUL/);
});

test("Prompt search covers both title and content without changing source order", () => {
  assert.deepEqual(filterPrompts(prompts, "translate").map((prompt) => prompt.id), [prompts[1].id]);
  assert.deepEqual(filterPrompts(prompts, "代码").map((prompt) => prompt.id), [prompts[0].id]);
  assert.deepEqual(filterPrompts(prompts, "").map((prompt) => prompt.id), prompts.map((prompt) => prompt.id));
});

test("Prompt reorder uses stable ids and preserves all records", () => {
  const reordered = movePromptBeforeTarget(prompts, prompts[1].id, prompts[0].id);
  assert.deepEqual(reordered.map((prompt) => prompt.id), [prompts[1].id, prompts[0].id]);
  const reorderedDown = movePromptBeforeTarget(prompts, prompts[0].id, prompts[1].id);
  assert.deepEqual(reorderedDown.map((prompt) => prompt.id), [prompts[1].id, prompts[0].id]);
  assert.equal(movePromptBeforeTarget(prompts, prompts[0].id, prompts[0].id), null);

  const ordered = orderPromptsByIds(prompts, [prompts[1].id, prompts[0].id]);
  assert.deepEqual(ordered.map((prompt) => prompt.id), [prompts[1].id, prompts[0].id]);
  assert.equal(orderPromptsByIds(prompts, [prompts[0].id, prompts[0].id]), null);
  assert.equal(orderPromptsByIds(prompts, [prompts[0].id]), null);
  assert.equal(isSamePromptOrder(prompts.map((prompt) => prompt.id), prompts), true);
  assert.equal(isSamePromptOrder([prompts[1].id, prompts[0].id], prompts), false);
});

test("Prompt dirty state compares the exact persisted body", () => {
  const draft = promptRecordToDraft(prompts[0]);
  assert.equal(isPromptDraftDirty(draft, prompts[0]), false);
  assert.equal(isPromptDraftDirty({ ...draft, content: `${draft.content}\n` }, prompts[0]), true);
});

test("Clipboard validation keeps leading and trailing Prompt whitespace", () => {
  const content = "\n  exact prompt  \n";
  assert.equal(resolveClipboardPayload(content), content);
  assert.throws(() => resolveClipboardPayload(" \n "), /复制内容为空/);
});

test("Prompts stays reachable from the primary menu and owns its two-pane route", () => {
  const navigation = readFileSync("src/lib/navigation.ts", "utf8");
  const promptIcon = readFileSync("src/components/shell/prompt-letter-icon.tsx", "utf8");
  const router = readFileSync("src/routes/app-router.tsx", "utf8");
  const shell = readFileSync("src/layouts/app-shell.tsx", "utf8");
  assert.match(navigation, /key: "prompts"[\s\S]*?href: "\/prompts"/);
  assert.match(navigation, /key: "prompts"[\s\S]*?icon: PromptLetterIcon/);
  assert.doesNotMatch(navigation, /SnippetsOutlined/);
  assert.match(promptIcon, />\s*P\s*<\/span>/);
  assert.match(router, /path: "prompts"[\s\S]*?<PromptsPage/);
  assert.match(shell, /routeKey !== "prompts"/);
});

test("Prompt list rows clip long previews inside the fixed virtual row", () => {
  const item = readFileSync("src/features/prompts/prompt-list-item.tsx", "utf8");
  const sidebar = readFileSync("src/features/prompts/prompt-sidebar.tsx", "utf8");

  assert.match(item, /h-\[94px\][^"\n]*overflow-hidden/);
  assert.match(item, /h-full[^"\n]*overflow-hidden[^"\n]*text-left/);
  assert.match(item, /line-clamp-2[^"\n]*max-h-\[40px\][^"\n]*break-words/);
  assert.doesNotMatch(item, /line-clamp-2[^"\n]*\bblock\b|\bblock\b[^"\n]*line-clamp-2/);
  assert.match(sidebar, /const PROMPT_ROW_HEIGHT = 99;/);
  assert.match(sidebar, /layoutScroll/);
  assert.match(sidebar, /values=\{orderedPrompts\.map\(\(prompt\) => prompt\.id\)\}/);
  assert.match(sidebar, /至少需要两条 Prompt 才能排序/);
  assert.match(item, /layout="position"/);
  assert.match(item, /layout=\{pointerReorderEnabled \? "position" : false\}/);
  assert.match(item, /value=\{prompt\.id\}/);
  assert.match(item, /transition=\{reorderLayoutTransition\}/);
  assert.match(
    item,
    /themeMode === "dark"[\s\S]*?hover:border-\[#3c3c3c\] hover:bg-\[#2a2d2e\][\s\S]*?hover:border-\[#d9e2ec\] hover:bg-\[#f8fafc\]/,
  );
});

test("Prompt editor uses large readable text for title and body", () => {
  const editor = readFileSync("src/features/prompts/prompt-editor.tsx", "utf8");

  assert.match(editor, /!h-\[44px\][^"\n]*!text-\[20px\]/);
  assert.match(editor, /!text-\[20px\][^"\n]*!leading-8/);
  assert.doesNotMatch(editor, /font-mono !text-\[12px\]/);
});
