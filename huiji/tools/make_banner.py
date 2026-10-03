from PIL import Image, ImageDraw, ImageFont
import os

ASSETS = r"C:\Users\dril\OneDrive\Desktop\灰迹输入法\app\huiji\assets"
OUT = os.path.join(ASSETS, "huiji-banner.png")

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

bg.convert("RGB").save(OUT, quality=95)
print("saved", OUT, bg.size)
