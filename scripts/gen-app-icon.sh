#!/bin/sh
# 生成 Tauri 应用图标。
#
# 1. 用 Python stdlib（zlib + struct）画一张 1024x1024 RGBA 占位图（蓝底 + 白色声波圆环，
#    暗示「唤醒词/语音」）；
# 2. 用 @tauri-apps/cli 的 `tauri icon` 从占位图生成全套图标（32x32 / 128x128 / icns / ico / …）
#    输出到 src-tauri/icons/（需提交，CI 构建依赖）。
set -e

cd "$(dirname "$0")/.."
SRC="src-tauri/icon-source.png"
OUT="src-tauri/icons"

python3 - "$SRC" <<'PY'
import struct
import sys
import zlib

W = H = 1024
r_out = sys.argv[1]


def write_png(path, width, height, pixel_at):
    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(
            ">I", zlib.crc32(body) & 0xFFFFFFFF
        )

    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter: none
        for x in range(width):
            raw.extend(pixel_at(x, y))
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def pixel_at(x, y):
    # 对角渐变：深蓝 (#1e3a8a) -> 亮蓝 (#2563eb)
    t = (x + y) / (2 * W)
    bg = (30 + int((37 - 30) * t), 58 + int((99 - 58) * t), 138 + int((235 - 138) * t))

    # 中心白色圆环（麦克风意象）
    cx, cy = W / 2, H / 2
    dx, dy = x - cx, y - cy
    dist = (dx * dx + dy * dy) ** 0.5
    if 300 <= dist <= 330:
        return (255, 255, 255, 255)
    # 圆环内部的小圆点（声波点）
    if dist <= 40:
        return (255, 255, 255, 255)
    # 圆环外圈一圈淡白点阵（声波扩散）
    if 380 <= dist <= 384 and int(dist) % 4 < 2:
        return (255, 255, 255, 220)
    return (*bg, 255)


write_png(r_out, W, H, pixel_at)
print(f"wrote {r_out}")
PY

mkdir -p "$OUT"
npx tauri icon "$SRC" -o "$OUT"
echo "icons generated in $OUT"
