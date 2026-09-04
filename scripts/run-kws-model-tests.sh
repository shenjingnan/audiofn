#!/usr/bin/env bash
# 下载模型并运行依赖模型文件的 KWS 测试（#[ignore] 门禁）。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
"$SCRIPT_DIR/download-kws-model.sh"
cd "$SCRIPT_DIR/.."
cargo test -- --ignored --test-threads=1
