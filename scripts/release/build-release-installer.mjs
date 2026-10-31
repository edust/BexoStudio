import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = fileURLToPath(new URL("../../", import.meta.url));
const platform = process.platform;
const target = platform === "win32" ? "x86_64-pc-windows-msvc" : "aarch64-apple-darwin";
if (!((platform === "win32" && process.arch === "x64") || (platform === "darwin" && process.arch === "arm64"))) {
  throw new Error("发行构建需要 Windows x64 或 macOS arm64 宿主。");
}
// Build the checked-in version. Versioning stays explicit; builds never mutate manifests or locks.
const result = spawnSync(process.execPath, [
  path.join(root, "node_modules/@tauri-apps/cli/tauri.js"),
  "build", "--target", target, "--bundles", platform === "win32" ? "nsis" : "app,dmg", "--", "--locked",
], { cwd: root, stdio: "inherit", timeout: 60 * 60 * 1000 });
if (result.error) throw result.error;
if (result.signal) throw new Error(`Build terminated: ${result.signal}`);
process.exitCode = result.status ?? 1;
