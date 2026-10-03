#!/usr/bin/env python3
# 把透明图标分别贴到浅色、深色背景上，预览实际显示效果。
import argparse

from PIL import Image


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--icon", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    icon = Image.open(args.icon).convert("RGBA")
    tile = 360
    icon = icon.resize((tile, tile), Image.LANCZOS)

    w, h = 900, 520
    canvas = Image.new("RGBA", (w, h), (255, 255, 255, 255))
    left = Image.new("RGBA", (w // 2, h), (238, 238, 238, 255))
    right = Image.new("RGBA", (w // 2, h), (38, 38, 42, 255))
    canvas.paste(left, (0, 0))
    canvas.paste(right, (w // 2, 0))

    cy = (h - tile) // 2
    canvas.alpha_composite(icon, ((w // 2 - tile) // 2, cy))
    canvas.alpha_composite(icon, (w // 2 + (w // 2 - tile) // 2, cy))
    canvas.convert("RGB").save(args.out)
    print(args.out)


if __name__ == "__main__":
    main()
