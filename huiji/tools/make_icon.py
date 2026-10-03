#!/usr/bin/env python3
# 把"白底黑字"的标志图转成透明背景黑字，并打包多尺寸 .ico。
# 用法：
#   python make_icon.py --input 白底黑字.png --out-png huiji.png --out-ico huiji.ico
import argparse

from PIL import Image


def build_alpha(img: Image.Image, low: int, high: int) -> Image.Image:
    """按亮度生成 alpha：越黑越不透明。low 以下全不透明，high 以上全透明。"""
    gray = img.convert("L")
    px = gray.load()
    w, h = gray.size
    alpha = Image.new("L", (w, h))
    ap = alpha.load()
    span = max(high - low, 1)
    for y in range(h):
        for x in range(w):
            v = px[x, y]
            if v <= low:
                a = 255
            elif v >= high:
                a = 0
            else:
                # 抗锯齿灰阶线性过渡
                a = int(255 * (high - v) / span)
            ap[x, y] = a
    return alpha


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", required=True)
    ap.add_argument("--out-png", required=True)
    ap.add_argument("--out-ico", required=True)
    ap.add_argument("--size", type=int, default=1024, help="透明 PNG 边长")
    ap.add_argument("--pad", type=float, default=0.16, help="字形外留白比例")
    ap.add_argument("--low", type=int, default=60)
    ap.add_argument("--high", type=int, default=238)
    args = ap.parse_args()

    src = Image.open(args.input).convert("RGB")
    alpha = build_alpha(src, args.low, args.high)

    # 字用纯黑，只靠 alpha 表达笔锋与飞白
    black = Image.new("RGBA", src.size, (0, 0, 0, 0))
    black.putalpha(alpha)

    # 裁掉四周空白，再补成带安全距的正方形
    bbox = black.getbbox()
    if not bbox:
        raise SystemExit("图里没有找到字形")
    glyph = black.crop(bbox)
    side = int(max(glyph.size) * (1 + args.pad))
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(glyph, ((side - glyph.width) // 2, (side - glyph.height) // 2), glyph)

    out = canvas.resize((args.size, args.size), Image.LANCZOS)
    out.save(args.out_png)

    icon_sizes = [(16, 16), (20, 20), (24, 24), (32, 32), (40, 40),
                  (48, 48), (64, 64), (128, 128), (256, 256)]
    out.save(args.out_ico, sizes=icon_sizes)
    print(f"png {args.out_png}  ico {args.out_ico}  side={side}")


if __name__ == "__main__":
    main()
