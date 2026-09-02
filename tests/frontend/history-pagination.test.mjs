import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("Codex history uses cursor pagination and keeps message pagination separate", () => {
  const page = readFileSync("src/pages/history-page.tsx", "utf8");
  const commandClient = readFileSync("src/lib/command-client.ts", "utf8");
  const backendTypes = readFileSync("src/types/backend.ts", "utf8");

  assert.match(page, /useInfiniteQuery/);
  assert.match(page, /SESSION_PAGE_SIZE = 10/);
  assert.match(page, /fetchNextPage/);
  assert.match(page, /SESSION_LOAD_THRESHOLD = 120/);
  assert.match(page, /继续加载 10 条/);
  assert.match(page, /getCodexHistoryMessages\(/);
  assert.match(commandClient, /listAllCodexHistorySessions\(input: ListCodexHistorySessionsPagePayload\)/);
  assert.match(commandClient, /list_all_codex_history_sessions/);
  assert.match(backendTypes, /nextCursor\?: string \| null/);
  assert.match(backendTypes, /hasMore: boolean/);
});
