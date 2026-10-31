#!/bin/bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
trap 'echo "打包失败，请查看上方错误信息。" >&2' ERR

if [[ "$(uname -s)" != Darwin ]]; then
  echo "此脚本需要在 macOS arm64 上运行。" >&2
  exit 1
fi

# 优先使用已安装的工具；兼容本机验收时准备的临时工具链。
export PATH="${PATH}:${HOME}/.cargo/bin"
if ! command -v node >/dev/null 2>&1; then
  export PATH="${PATH}:${HOME}/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin"
fi
if ! command -v npm >/dev/null 2>&1; then
  export PATH="${PATH}:/tmp/bexo-build-tools/bin"
fi
if ! command -v cargo >/dev/null 2>&1 && [[ -x /tmp/bexo-build-tools/cargo/bin/cargo ]]; then
  export CARGO_HOME="${CARGO_HOME:-/tmp/bexo-build-tools/cargo}"
  export RUSTUP_HOME="${RUSTUP_HOME:-/tmp/bexo-build-tools/rustup}"
  export PATH="${PATH}:/tmp/bexo-build-tools/cargo/bin"
fi

for tool in node npm cargo; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "缺少 $tool，请先安装 Node.js 24/npm 和 Rust stable。" >&2
    exit 1
  fi
done
node -e 'if (process.platform !== "darwin" || process.arch !== "arm64") { console.error("请使用 macOS arm64 版 Node.js 运行此脚本。"); process.exit(1); }'
xcode-select -p >/dev/null

# 可选的本机签名身份；环境变量优先，未配置时沿用 Tauri 的临时签名。
# 文件只保存证书名称或指纹，不保存私钥或密码，也不提交到仓库。
if [[ -z "${APPLE_SIGNING_IDENTITY:-}" && -f .macos-signing-identity ]]; then
  APPLE_SIGNING_IDENTITY="$(cat .macos-signing-identity)"
  if [[ -z "$APPLE_SIGNING_IDENTITY" ]]; then
    echo ".macos-signing-identity 为空，请填写签名证书名称或指纹。" >&2
    exit 1
  fi
  export APPLE_SIGNING_IDENTITY
fi
printf 'macOS 签名身份：%s\n' "${APPLE_SIGNING_IDENTITY:-- (ad-hoc)}"

if [[ ! -f node_modules/@tauri-apps/cli/tauri.js ]]; then
  npm ci
fi

npm run release:build
printf '\n打包完成：\n%s\n%s\n' \
  "$PWD/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/" \
  "$PWD/src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/"
