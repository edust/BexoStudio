import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");

test("OSS components use semantic theme classes for custom surfaces and states", () => {
  const css = read("src/styles/globals.css");
  const accountDialog = read("src/features/oss/oss-account-dialog.tsx");
  const accountSidebar = read("src/features/oss/oss-account-sidebar.tsx");
  const targetDialog = read("src/features/oss/oss-target-dialog.tsx");
  const transferQueue = read("src/features/oss/oss-transfer-queue.tsx");
  const page = read("src/features/oss/oss-page.tsx");

  for (const token of [
    "--oss-surface-info",
    "--oss-surface-hover",
    "--oss-border-subtle",
    "--oss-text-danger",
    "--oss-focus-ring",
  ]) {
    assert.match(css, new RegExp(token.replaceAll("-", "\\-")));
  }
  assert.match(css, /\.oss-object-row:hover\s*\{/);
  assert.match(css, /\.oss-focusable:focus-visible\s*\{/);
  assert.match(css, /:root\[data-theme="dark"\][\s\S]*--oss-surface-info:\s*#20364a/);
  assert.match(css, /:root\[data-theme="dark"\][\s\S]*--oss-text-danger:\s*#f38b86/);

  assert.match(accountDialog, /oss-surface-info[\s\S]*oss-border-info/);
  assert.match(accountDialog, /oss-text-info/);
  assert.match(accountSidebar, /oss-card-hover/);
  assert.match(accountSidebar, /oss-status-danger/);
  assert.match(targetDialog, /oss-surface-muted[\s\S]*oss-border/);
  assert.match(transferQueue, /oss-text-danger/);
  assert.match(page, /oss-object-row/);
  assert.match(page, /oss-border-subtle/);

  for (const source of [accountDialog, accountSidebar, targetDialog, transferQueue, page]) {
    assert.doesNotMatch(source, /\b(?:bg|border|text)-\[\#/);
    assert.doesNotMatch(source, /\bhover:(?:bg|border|text)-/);
  }
});
