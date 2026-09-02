import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildOssFolderKey,
  validateOssFolderName,
} from "../../src/features/oss/oss-folder-model.ts";

test("OSS folder names are single safe path segments", () => {
  assert.equal(validateOssFolderName("  images  "), null);
  assert.match(validateOssFolderName(""), /请输入文件夹名称/);
  assert.match(validateOssFolderName("images/photos"), /斜杠/);
  assert.match(validateOssFolderName("images\\photos"), /斜杠/);
  assert.match(validateOssFolderName("."), /\. 或 \.\./);
  assert.match(validateOssFolderName(".."), /\. 或 \.\./);
  assert.match(validateOssFolderName("bad\u0000name"), /控制字符/);
  assert.match(validateOssFolderName("a".repeat(255)), /254/);
});

test("OSS folder key preview preserves the current prefix and trailing slash", () => {
  assert.equal(buildOssFolderKey("assets/", "images"), "assets/images/");
  assert.equal(buildOssFolderKey("assets", "images"), "assets/images/");
  assert.equal(buildOssFolderKey("", "images"), "images/");
});

test("OSS folder UI and command wiring expose refresh and permission paths", () => {
  const page = readFileSync("src/features/oss/oss-page.tsx", "utf8");
  const dialog = readFileSync("src/features/oss/oss-folder-dialog.tsx", "utf8");
  const commandClient = readFileSync("src/lib/command-client.ts", "utf8");
  const buildScript = readFileSync("src-tauri/build.rs", "utf8");
  const handler = readFileSync("src-tauri/src/app/mod.rs", "utf8");
  const permissions = readFileSync("src-tauri/permissions/window-commands.toml", "utf8");

  assert.match(page, /createOssFolder/);
  assert.match(page, /invalidateObjects\(queryClient, targetId, prefix\)/);
  assert.match(page, /新建文件夹/);
  assert.match(dialog, /目标 Object Key/);
  assert.match(commandClient, /"create_oss_folder"/);
  assert.match(buildScript, /"create_oss_folder"/);
  assert.match(handler, /commands::oss::create_oss_folder/);
  assert.match(permissions, /"allow-create-oss-folder"/);
});
