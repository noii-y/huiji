#!/usr/bin/env python3
"""灰迹输入法打字浸泡测试。

假定前台已打开一个全新的记事本、且灰迹处于中文模式。脚本逐轮打字、上屏，
每隔若干轮截图留证，最后全选复制回每条上屏结果，写出 JSON 供分析。

轮次类型：
- word   打拼音后空格上首选，再回车换行（一行一条结果）
- page    打拼音后用 PageDown / PageUp 翻页再空格
- raw     打拼音后回车（按设计保留拼音原文）
- esc     打拼音后 Esc（应清空、不留内容）
- proper  拉丁专有名词
- toggle  Ctrl+空格 中英切换后打字再切回

注：cu.press 无法注入符号 OEM 键，句内标点与 -= 翻页不在本脚本内，由 Rust 单测 + 人工覆盖。
"""

import json
import os
import time

import seed_computer_use as cu

# --- 输入小工具 -------------------------------------------------------------

def type_pinyin(text, settle=0.12):
    # cu.press 只认字母 / 数字 / 命名功能键，符号 OEM 键（, . - = 等）一律不支持，
    # 故浸泡脚本只打纯字母串；标点与 -= 翻页由 Rust 单测覆盖、人工补测。
    for ch in text:
        cu.press(ch)
        time.sleep(0.045)
    time.sleep(settle)


def shot(tag):
    png = cu.screenshot(tag=tag)
    path = os.path.join(os.environ["HUII_SOAK_DIR"], f"{tag}.png")
    with open(path, "wb") as handle:
        handle.write(bytes(png))
    return path


def read_all_lines():
    cu.hotkey("ctrl", "a")
    time.sleep(0.15)
    cu.hotkey("ctrl", "c")
    time.sleep(0.2)
    text = cu.get_clipboard()
    cu.hotkey("ctrl", "end")
    time.sleep(0.15)
    return [line.rstrip("\r") for line in text.split("\n")]


# --- 轮次集合 ---------------------------------------------------------------

WORDS = [
    "nihao", "xiexie", "zaijian", "woaini", "zhongguo", "beijing", "shanghai",
    "xuexi", "gongzuo", "diannao", "shouji", "shijian", "shenghuo", "pengyou",
    "jiating", "wenti", "fanyi", "shuru", "kaifa", "fangzi", "qiche", "shudian",
    "kafei", "chifan", "shuijiao", "shangban", "xiaban", "mingtian", "jintian",
    "zuotian", "xianzai", "dianying", "yinyue", "tushu", "laoshi", "xuesheng",
    "tongxue", "huiyi", "jihua", "xiangfa", "zhidao", "juede", "renwei",
    "xiwang", "xihuan", "gaosu", "jieshi", "lijie", "shiyan", "ceshi",
    "jieguo", "rizhi", "wending", "kuaisu", "jiandan", "rongyi", "kunnan",
    "zhongyao", "guanjian", "putong", "teshu", "zhuanye", "mingci", "jianyi",
    "caozuo", "xitong", "shezhi", "ruanjian", "yingjian", "wangluo", "fuwu",
    "kehu", "yonghu", "shuju", "zidian", "ciyu", "juzi", "biaodian",
    "waiguoren", "youyong", "paobu", "lvxing", "gouwu", "youxi",
]

PROPER = ["linux", "google", "github", "iphone", "nvidia"]


def build_plan():
    plan = []
    # 88 个普通词
    for index in range(88):
        plan.append(("word", WORDS[index % len(WORDS)]))
    # 10 个翻页（用 PageDown / PageUp，与 -= 走同一套候选翻页逻辑）
    for index in range(10):
        plan.append(("page", WORDS[(index * 3) % len(WORDS)]))
    # 8 个回车保留原文
    for index in range(8):
        plan.append(("raw", WORDS[(index * 5) % len(WORDS)]))
    # 6 个 Esc（不留行）
    for index in range(6):
        plan.append(("esc", WORDS[(index * 7) % len(WORDS)]))
    # 5 个专有名词
    for text in PROPER:
        plan.append(("proper", text))
    # 3 次中英切换
    for index in range(3):
        plan.append(("toggle", WORDS[(index * 11) % len(WORDS)]))
    return plan


def main():
    out_dir = os.environ["HUII_SOAK_DIR"]
    os.makedirs(out_dir, exist_ok=True)
    plan = build_plan()

    records = []
    line_no = 0
    for round_index, (kind, payload) in enumerate(plan, start=1):
        rec = {"round": round_index, "kind": kind, "typed": payload, "line": None}
        type_pinyin(payload)

        if kind == "word":
            cu.press("space")
            time.sleep(0.08)
            cu.press("enter")
            time.sleep(0.08)
            rec["line"] = line_no
            line_no += 1
        elif kind == "page":
            cu.press("pagedown")   # 往后翻一页
            time.sleep(0.18)
            cu.press("pageup")     # 翻回首页
            time.sleep(0.18)
            cu.press("space")
            time.sleep(0.08)
            cu.press("enter")
            time.sleep(0.08)
            rec["line"] = line_no
            line_no += 1
        elif kind == "raw":
            cu.press("enter")
            time.sleep(0.08)
            rec["line"] = line_no
            line_no += 1
        elif kind == "esc":
            cu.press("esc")
            time.sleep(0.08)
        elif kind == "proper":
            cu.press("space")
            time.sleep(0.08)
            cu.press("enter")
            time.sleep(0.08)
            rec["line"] = line_no
            line_no += 1
        elif kind == "toggle":
            cu.hotkey("ctrl", "space")  # 切英文
            time.sleep(0.2)
            type_pinyin("abc")          # 英文模式打 abc
            cu.press("space")           # 上 abc
            time.sleep(0.05)
            cu.hotkey("ctrl", "space")  # 切回中文
            time.sleep(0.2)
            cu.press("enter")
            time.sleep(0.08)
            rec["line"] = line_no
            line_no += 1

        # 每 10 轮截图：在下一轮打字之后、上屏之前更能拍到候选窗，
        # 这里在动作间隙补拍当前记事本状态。
        if round_index % 10 == 0:
            rec["screenshot"] = shot(f"soak-{round_index:03d}")
        records.append(rec)

    # 中途自愈专项：杀掉本会话 server，立刻打字，截图看候选窗是否随重拉回来
    heal = []
    type_pinyin("nihao")
    heal.append(("before_kill", shot("heal-1-before")))
    os.system('taskkill /f /im qingjian-server.exe /fi "SESSION eq 5"')
    time.sleep(0.4)
    cu.press("esc")
    time.sleep(0.2)
    type_pinyin("xiexie")
    heal.append(("after_relaunch", shot("heal-2-after")))
    cu.press("space")
    cu.press("enter")

    lines = read_all_lines()
    result = {"records": records, "lines": lines, "expected_lines": line_no + 1,
              "heal": heal}
    with open(os.path.join(out_dir, "soak-result.json"), "w", encoding="utf-8") as handle:
        json.dump(result, handle, ensure_ascii=False, indent=2)
    print("rounds:", len(plan), "lines captured:", len(lines),
          "expected ~", line_no + 1)


if __name__ == "__main__":
    main()
