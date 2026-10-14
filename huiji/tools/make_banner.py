from PIL import Image, ImageDraw, ImageFont
import os

# 脚本在 huiji/tools/，素材在 huiji/assets/，用相对路径避免写死电脑用户名。
ASSETS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
OUT = os.path.join(ASSETS, "huiji-banner.webp")

bg = Image.open(os.path.join(ASSETS, "banner-bg-raw.png")).convert("RGBA")
W, H = bg.size

# 灰迹色值
LIGHT = (232, 232, 234, 255)
AMBER = (205, 158, 104, 255)
MUTED = (138, 138, 144, 255)


def tint(img, color):
    """保留 alpha，把黑色书法字着成指定颜色。"""
    base = Image.new("RGBA", img.size, color)
    base.putalpha(img.split()[3])
    return base


# 反白「灰」图形标
logo = Image.open(os.path.join(ASSETS, "logo", "huiji-logo.png")).convert("RGBA")
LH = 440
logo = logo.resize((int(logo.width * LH / logo.height), LH), Image.LANCZOS)
logo = tint(logo, LIGHT)

FONTS = r"C:\Windows\Fonts"


def font(name, size):
    return ImageFont.truetype(os.path.join(FONTS, name), size)


f_cn = font("msyhbd.ttc", 168)
try:
    f_en = font("segoeuil.ttf", 168)
except OSError:
    f_en = font("segoeui.ttf", 168)
f_slogan = font("msyh.ttc", 66)
f_sub = font("msyh.ttc", 52)
f_tag = font("msyh.ttc", 44)

draw = ImageDraw.Draw(bg)

# 图形标位置
lx = 190
ly = (H - LH) // 2 - 30
bg.alpha_composite(logo, (lx, ly))

# 文字块
tx = lx + logo.width + 100
title_y = ly + 20

# 标题：灰迹（浅色） Huiji（琥珀）
w_cn = draw.textlength("灰迹", font=f_cn)
draw.text((tx, title_y), "灰迹", font=f_cn, fill=LIGHT)
gap = 36
draw.text((tx + w_cn + gap, title_y + 18), "Huiji", font=f_en, fill=AMBER)

# slogan
slogan_y = title_y + 250
draw.text((tx, slogan_y), "工作打字之余，顺便学一学外语", font=f_slogan, fill=LIGHT)
draw.text((tx, slogan_y + 100), "不用专门腾时间，敲字就是教材", font=f_sub, fill=LIGHT)

# 标签（浅色 + 深色描边，避免压在琥珀笔触上看不清）
tags = "边打边学  ·  整句译文  ·  全在本机"
draw.text(
    (tx, slogan_y + 205),
    tags,
    font=f_tag,
    fill=LIGHT,
    stroke_width=3,
    stroke_fill=(20, 20, 23, 255),
)

final = bg.convert("RGB")
# README 主视觉在页面上最多约 900px 宽，1800 宽够 2x 视网膜屏；
# 深色噪点 + 锐利文字用 WebP q86，原图 3.7MB 可压到约 250KB 且肉眼无损。
target_w = 1800
final = final.resize((target_w, round(final.height * target_w / final.width)), Image.LANCZOS)
final.save(OUT, "WEBP", quality=86, method=6)
print("saved", OUT, final.size)
