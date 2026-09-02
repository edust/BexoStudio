import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildOssUploadObjectKey,
  enqueueOssUploadPaths,
  prepareOssUploadCandidates,
} from "../../src/features/oss/oss-upload-model.ts";

test("OSS upload candidates preserve the current prefix and reject duplicate targets", () => {
  assert.equal(buildOssUploadObjectKey("assets", "photo.png"), "assets/photo.png");
  assert.equal(buildOssUploadObjectKey("assets/", "photo.png"), "assets/photo.png");
  assert.equal(buildOssUploadObjectKey("", "photo.png"), "photo.png");

  const result = prepareOssUploadCandidates(
    ["D:\\drop\\photo.png", "C:\\other\\photo.png", "D:\\drop\\notes.txt", "bad\u0000path"],
    "assets/",
  );
  assert.deepEqual(result.accepted.map((item) => item.objectKey), ["assets/photo.png", "assets/notes.txt"]);
  assert.equal(result.rejected.length, 2);
  assert.match(result.rejected[0].reason, /相同目标文件名/);
  assert.match(result.rejected[1].reason, /路径无效/);
});

test("OSS batch enqueue is bounded and keeps partial failures visible", async () => {
  let active = 0;
  let maximumActive = 0;
  const started = [];
  const result = await enqueueOssUploadPaths(
    ["D:\\drop\\a.bin", "D:\\drop\\b.bin", "D:\\drop\\c.bin", "D:\\drop\\d.bin", "D:\\drop\\e.bin"],
    "target-1",
    "uploads/",
    async (input) => {
      active += 1;
      maximumActive = Math.max(maximumActive, active);
      started.push(input);
      await new Promise((resolve) => setTimeout(resolve, input.objectKey.endsWith("a.bin") ? 8 : 2));
      active -= 1;
      if (input.objectKey.endsWith("c.bin")) {
        throw new Error("模拟入队失败");
      }
    },
    2,
  );

  assert.equal(maximumActive, 2);
  assert.equal(started.length, 5);
  assert.deepEqual(result.accepted.map((item) => item.name).sort(), ["a.bin", "b.bin", "d.bin", "e.bin"]);
  assert.equal(result.rejected.length, 1);
  assert.equal(result.rejected[0].name, "c.bin");
  assert.match(result.rejected[0].reason, /模拟入队失败/);
});

test("OSS page uses native Tauri multi-file drop and completion refresh wiring", () => {
  const page = readFileSync("src/features/oss/oss-page.tsx", "utf8");
  const queue = readFileSync("src/features/oss/oss-transfer-queue.tsx", "utf8");
  const tauriConfig = readFileSync("src-tauri/tauri.conf.json", "utf8");

  assert.match(page, /getCurrentWebview\(\)\s*\.onDragDropEvent/);
  assert.match(page, /multiple: true/);
  assert.match(page, /onDropPaths=\{/);
  assert.match(page, /onTaskStateChanged=\{handleTransferTaskStateChanged\}/);
  assert.match(page, /invalidateObjectsForTarget/);
  assert.match(queue, /speedBytesPerSecond/);
  assert.match(queue, /needs_confirmation/);
  assert.match(queue, /onTaskStateChanged\?\./);
  assert.match(tauriConfig, /"dragDropEnabled": true/);
});
