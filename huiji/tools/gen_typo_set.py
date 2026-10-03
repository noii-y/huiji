# -*- coding: utf-8 -*-
"""
手误对照集生成器（灰迹输入法 · 阶段 2B）。

读 assets/lexicon/dict.tsv（词 / 音节 / 词频），挑常用的 2-4 字词，
按真实手误的几类注入错误，输出 eval 子系统能直接读的三列 TSV：正确词 \\t 错误拼音 \\t 上文（空）。

分类型出文件，方便看清每一类现在的命中/召回，以及改完之后各涨了多少：
  手误-换位        相邻字母敲反（真实数据里占大头，青简已能纠，做不回归）
  手误-漏1字母      少按一个键
  手误-多1字母      多按一个键
  手误-相邻键替换   想按的键被旁边的键替了
  手误-声母缩写     多音节词部分音节只敲了首字母（如 xinrenwu -> xinrw，当前召回弱）
  手误-模糊音       近音/口音（平翘舌、前后鼻韵、l/n、h/f）
  防误纠           正确拼音，期望首选仍是它，用来盯过度纠错。

固定随机种子，结果可复现。
"""

import os
import random
import sys

SEED = 20261003
PER_TYPE = 160          # 每类手误条数
NEGATIVE = 400          # 防误纠条数
MIN_FREQ = 40           # 词频门槛，取常用词
random.seed(SEED)

HERE = os.path.dirname(os.path.abspath(sys.argv[0]))
APP = os.path.abspath(os.path.join(HERE, "..", ".."))
DICT = os.path.join(APP, "assets", "lexicon", "dict.tsv")
OUTDIR = os.path.abspath(os.path.join(HERE, "..", "评测", "手误"))

# QWERTY 同行相邻键（“按错旁边的键”最常见）
KEYBOARD_ROWS = ["qwertyuiop", "asdfghjkl", "zxcvbnm"]
NEIGHBORS = {}
for row in KEYBOARD_ROWS:
    for i, ch in enumerate(row):
        near = []
        if i > 0:
            near.append(row[i - 1])
        if i + 1 < len(row):
            near.append(row[i + 1])
        NEIGHBORS[ch] = near


def is_han(ch):
    return "一" <= ch <= "鿿"


def load_words():
    words = []
    with open(DICT, encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("#") or not line.strip():
                continue
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 3:
                continue
            text, syllables, freq = parts[0], parts[1], parts[2]
            try:
                freq_i = int(freq)
            except ValueError:
                continue
            syl = syllables.split()
            joined = "".join(syl)
            if not all(is_han(ch) for ch in text):
                continue
            if not (2 <= len(syl) <= 4):
                continue
            if not (5 <= len(joined) <= 16):
                continue
            if freq_i < MIN_FREQ:
                continue
            words.append((text, syl, freq_i))
    # 高频优先，同时打乱同档顺序，避免总被最前面的词占满
    words.sort(key=lambda w: -w[2])
    words = words[:1200]
    random.shuffle(words)
    return words


def transpose(syl):
    s = "".join(syl)
    i = random.randrange(len(s) - 1)
    return s[:i] + s[i + 1] + s[i] + s[i + 2:]


def missing_one(syl):
    s = "".join(syl)
    i = random.randrange(len(s))
    return s[:i] + s[i + 1:]


def extra_one(syl):
    s = "".join(syl)
    i = random.randrange(len(s) + 1)
    return s[:i] + random.choice("abcdefghijklmnopqrstuvwxyz") + s[i:]


def adjacent_sub(syl):
    s = "".join(syl)
    candidates = [i for i, ch in enumerate(s) if NEIGHBORS.get(ch)]
    if not candidates:
        return None
    i = random.choice(candidates)
    return s[:i] + random.choice(NEIGHBORS[s[i]]) + s[i + 1:]


def initial_abbrev(syl):
    # 至少保留一个完整音节，其余缩成首字母
    if len(syl) < 2:
        return None
    keep_full = random.randrange(len(syl))
    out = []
    for i, s in enumerate(syl):
        out.append(s if i == keep_full else s[0])
    return "".join(out)


# 模糊音：在音节层面改一处。双字母声母 zh/ch/sh 与单字母 z/c/s 要按实际声母最长匹配，
# 否则 cha 会被既当 ch 又当 c，造出 chha 这种不存在的形态。
INITIAL_TO_SINGLE = {"zh": "z", "ch": "c", "sh": "s"}
SINGLE_TO_INITIAL = {"z": "zh", "c": "ch", "s": "sh"}
SINGLE_SWAP = {"l": "n", "n": "l", "h": "f", "f": "h"}
FINAL_LONG_TO_SHORT = {"ing": "in", "eng": "en"}
FINAL_SHORT_TO_LONG = {"in": "ing", "en": "eng"}


def initial_variants(s):
    for two, one in INITIAL_TO_SINGLE.items():
        if s.startswith(two):
            return [one + s[2:]]
    first = s[0]
    if first in SINGLE_TO_INITIAL:
        return [SINGLE_TO_INITIAL[first] + s[1:]]
    if first in SINGLE_SWAP:
        return [SINGLE_SWAP[first] + s[1:]]
    return []


def final_variants(s):
    for long, short in FINAL_LONG_TO_SHORT.items():
        if s.endswith(long):
            return [s[: -len(long)] + short]
    for short, long in FINAL_SHORT_TO_LONG.items():
        if s.endswith(short):
            return [s[: -len(short)] + long]
    return []


def fuzzy_sound(syl):
    candidates = []
    for idx, s in enumerate(syl):
        for v in initial_variants(s):
            candidates.append((idx, v))
        for v in final_variants(s):
            candidates.append((idx, v))
    if not candidates:
        return None
    idx, v = random.choice(candidates)
    syl = list(syl)
    syl[idx] = v
    return "".join(syl)


GENERATORS = {
    "手误-换位": transpose,
    "手误-漏1字母": missing_one,
    "手误-多1字母": extra_one,
    "手误-相邻键替换": adjacent_sub,
    "手误-声母缩写": initial_abbrev,
    "手误-模糊音": fuzzy_sound,
}


def main():
    os.makedirs(OUTDIR, exist_ok=True)
    words = load_words()
    if not words:
        print("没选到词，检查词库路径", file=sys.stderr)
        return 1

    for name, fn in GENERATORS.items():
        rows = []
        seen = set()
        for text, syl, _ in words:
            if len(rows) >= PER_TYPE:
                break
            typo = fn(syl)
            if not typo or typo == "".join(syl):
                continue
            key = (text, typo)
            if key in seen:
                continue
            seen.add(key)
            rows.append(f"{text}\t{typo}\t")
        path = os.path.join(OUTDIR, name + ".tsv")
        with open(path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write("\n".join(rows) + "\n")
        print(f"{name}: {len(rows)} 条")

    # 防误纠：正确拼音，期望首选 = 正确词
    neg = []
    seen = set()
    for text, syl, _ in words:
        if len(neg) >= NEGATIVE:
            break
        joined = "".join(syl)
        if (text, joined) in seen:
            continue
        seen.add((text, joined))
        neg.append(f"{text}\t{joined}\t")
    path = os.path.join(OUTDIR, "防误纠.tsv")
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(neg) + "\n")
    print(f"防误纠: {len(neg)} 条")
    return 0


if __name__ == "__main__":
    sys.exit(main())
