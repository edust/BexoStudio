import assert from "node:assert/strict";
import test from "node:test";
import { parseTerminalCommandLine, buildTerminalCommandLine } from "../../src/lib/terminal-command.ts";

test("macOS preserves POSIX expansion, quoted empty arguments and pipelines", () => {
  for (const line of [
    'printf "%s\\n" "$HOME" | head -1',
    "cd '/tmp/a b' && printf '%s' ''",
    'echo "$(pwd)" > /tmp/bexo-output',
  ]) {
    const parsed = parseTerminalCommandLine(line, "MacIntel");
    assert.deepEqual(parsed, { command: line, args: [] });
    assert.equal(buildTerminalCommandLine(parsed.command, parsed.args), line);
  }
});

test("Windows command parsing keeps existing paths and arguments", () => {
  assert.deepEqual(parseTerminalCommandLine('python "D:\\My App\\main.py" --port 3000', "Win32"), {
    command: "python", args: ["D:\\My App\\main.py", "--port", "3000"],
  });
});

test("resource browser preserves POSIX root and finds its parent correctly", async () => {
  const { normalizePath, resolveParentPath, isSameOrChildPath } = await import("../../src/features/resource-browser/resource-browser-utils.ts");
  assert.equal(normalizePath("/"), "/");
  assert.equal(resolveParentPath("/Applications"), "/");
  assert.equal(isSameOrChildPath("/Applications/Example.app", "/"), true);
  assert.equal(isSameOrChildPath("/tmp/Project/a", "/tmp/project"), false);
  assert.equal(isSameOrChildPath("C:/PROJECT/a", "c:/project"), true);
});
