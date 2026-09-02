import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildOssCopyTargetKey,
  validateOssCopyTargetKey,
} from "../../src/features/oss/oss-copy-model.ts";

const read = (path) => readFileSync(path, "utf8");

test("OSS copy target keys preserve folders and add a safe suffix", () => {
  assert.equal(buildOssCopyTargetKey("assets/photo.png"), "assets/photo-copy.png");
  assert.equal(buildOssCopyTargetKey("assets/archive"), "assets/archive-copy");
  assert.equal(buildOssCopyTargetKey(".env"), ".env-copy");
  assert.equal(validateOssCopyTargetKey("assets/photo-copy.png", "assets/photo.png"), null);
  assert.match(validateOssCopyTargetKey("", "assets/photo.png"), /请输入/);
  assert.match(validateOssCopyTargetKey("/photo.png", "photo.png"), /开头/);
  assert.match(validateOssCopyTargetKey("photo.png/", "photo.png"), /结尾/);
  assert.match(validateOssCopyTargetKey("photo.png", "photo.png"), /相同/);
  assert.match(validateOssCopyTargetKey("bad\u0000name", "photo.png"), /控制字符/);
});

test("OSS object context menu wires all four actions and keeps deletion confirmed", () => {
  const page = read("src/features/oss/oss-page.tsx");
  const dialog = read("src/features/oss/oss-copy-dialog.tsx");
  const commandClient = read("src/lib/command-client.ts");
  const buildScript = read("src-tauri/build.rs");
  const handler = read("src-tauri/src/app/mod.rs");
  const permissions = read("src-tauri/permissions/window-commands.toml");
  const command = read("src-tauri/src/commands/oss.rs");
  const service = read("src-tauri/src/services/oss_service.rs");
  const signer = read("src-tauri/src/adapters/oss/v4_signer.rs");

  assert.match(page, /trigger=\{\["contextMenu"\]\}/);
  for (const label of ["下载文件", "复制文件", "复制下载 URL", "删除"]) {
    assert.match(page, new RegExp(label));
  }
  assert.match(page, /Modal\.confirm/);
  assert.match(page, /copyTextToClipboard\(result\.url\)/);
  assert.match(page, /getClipboardErrorMessage/);
  assert.match(dialog, /目标 Object Key/);
  assert.match(dialog, /不会覆盖/);
  assert.match(commandClient, /"get_oss_object_download_url"/);
  assert.match(buildScript, /"get_oss_object_download_url"/);
  assert.match(handler, /commands::oss::get_oss_object_download_url/);
  assert.match(permissions, /"allow-get-oss-object-download-url"/);
  assert.match(command, /get_oss_object_download_url/);
  assert.match(service, /normalize_download_url_expires/);
  assert.match(service, /presign_get_url/);
  assert.match(signer, /x-oss-signature-version/);
  assert.match(signer, /x-oss-signature/);
  assert.doesNotMatch(page, /localStorage\.(?:setItem|getItem).*url/i);
});
