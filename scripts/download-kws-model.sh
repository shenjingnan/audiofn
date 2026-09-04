#!/usr/bin/env bash
# 按 models/manifest.json 下载并校验模型资产（幂等）。
#
# 用法:
#   ./scripts/download-kws-model.sh             # 下载清单中全部模型
#   ./scripts/download-kws-model.sh <name>      # 只下载指定模型
#
# 流程: 下载 tar 包 -> sha256 校验 -> 原子解压（先解到临时目录再移动）。
# 已存在且校验文件齐全则跳过；sha256 不匹配则删除损坏文件并报错。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$ROOT/models/manifest.json"
MODEL_DIR="$ROOT/models"

if [ ! -f "$MANIFEST" ]; then
  echo "错误: 找不到 $MANIFEST" >&2
  exit 1
fi

# 用 python3 从 manifest 提取资产元数据（制表符分隔，避免字段内空格干扰）。
# 输出每行: name<TAB>archive<TAB>source<TAB>sha256<TAB>size_bytes<TAB>license
extract_assets() {
  python3 - "$MANIFEST" "$1" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
want = sys.argv[2]
for a in manifest["assets"]:
    if not want or a["name"] == want:
        print(f"{a['name']}\t{a['archive']}\t{a['source']}\t{a['sha256']}\t{a['size_bytes']}\t{a.get('license', '')}")
PY
}

want="${1:-}"
found=0

while IFS=$'\t' read -r name archive source sha256 size license; do
  found=1
  target="$MODEL_DIR/$name"
  tmp_archive="$MODEL_DIR/.${archive}.tmp"
  tmp_extract="$MODEL_DIR/.${name}.extract"

  echo "== 模型: $name (${license:-license-未知}) =="

  # 幂等: 目录存在且核心文件齐全则跳过
  if [ -d "$target" ] && [ -f "$target/tokens.txt" ]; then
    echo "已存在: $target"
    continue
  fi

  echo "下载 $source ..."
  curl -fSL --retry 3 -o "$tmp_archive" "$source"

  echo "校验 sha256 ..."
  got="$(python3 -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$tmp_archive")"
  if [ "$got" != "$sha256" ]; then
    rm -f "$tmp_archive"
    echo "错误: sha256 不匹配（期望 ${sha256}，实际 ${got}），已删除损坏文件，请重试" >&2
    exit 1
  fi

  echo "解压到 $target ..."
  rm -rf "$tmp_extract"
  mkdir -p "$tmp_extract"
  tar xjf "$tmp_archive" -C "$tmp_extract"
  rm -f "$tmp_archive"
  # 原子移动: 解压完整后再落位，避免中断留下半截目录
  mv "$tmp_extract/$name" "$target"
  rm -rf "$tmp_extract"
  echo "完成: $target"
done < <(extract_assets "$want")

if [ "$found" -eq 0 ]; then
  echo "错误: 清单中不存在模型 '$want'（查看 ${MANIFEST}）" >&2
  exit 1
fi
