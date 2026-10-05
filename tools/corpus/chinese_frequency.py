# /// script
# requires-python = ">=3.10"
# dependencies = ["wordfreq>=3.1"]
# ///
"""用 wordfreq 的中文词频替换语言模型的一元先验，并把二元计数按新边际重标定（条件概率不变）。

背景：随包 lm 的一元与 <s> 句首统计主要来自中文维基，语域偏书面，「世纪 / 世界 / 时间」常作句首，
「手机 / 信息 / 谢谢」被低估。打声母缩写（sj、xx）发生在日常输入里，排序应按聚合多语域的一元先验。

流程：
  1. 先从已打包模型反导出语料 TSV：
       cargo run -p qingjian-cli --example dump_lm
  2. 本脚本读 data/generated/lm-unigram.tsv、lm-bigram.tsv，写出 *.new.tsv：
       uv run tools/corpus/chinese_frequency.py
  3. 核对后替换并重新打包：
       mv lm-unigram.new.tsv lm-unigram.tsv; mv lm-bigram.new.tsv lm-bigram.tsv
       cargo run --release -p qingjian-dict-convert -- pack lm \
         --name 语言模型 --license ... --attribution ... --source ...

wordfreq 代码 MIT，数据带 CC-BY-SA 等许可，这里只取频率数字做排序。
"""

import math
import sys

from wordfreq import zipf_frequency

UNIGRAM = "data/generated/lm-unigram.tsv"
BIGRAM = "data/generated/lm-bigram.tsv"
START = "<s>"

# 一元计数 = 10^zipf · SCALE；取 10 让最高频词（zipf≈7.6）计数在 4e8 量级、落进 u32 不被截断。
SCALE = 10.0
U32_MAX = 4_294_967_295


def read_tsv(path):
    rows = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line:
                continue
            fields = line.split("\t")
            rows.append((fields[0], fields[1:]))
    return rows


def main():
    unigram_rows = read_tsv(UNIGRAM)
    orig_count = {}
    for word, rest in unigram_rows:
        orig_count[word] = int(rest[0])

    # 已知词的 wordfreq 计数，顺带拟合 语料计数 -> 新词频 的比例，给未收录词回退用
    new_count = {}
    fit_ratios = []
    known = 0
    for word, orig in orig_count.items():
        if word == START:
            continue
        zipf = zipf_frequency(word, "zh")
        if zipf > 0:
            value = min(U32_MAX, round(10.0**zipf * SCALE))
            new_count[word] = max(1, value)
            known += 1
            if orig > 0:
                fit_ratios.append(new_count[word] / orig)
        else:
            new_count[word] = None
    fit_ratios.sort()
    ratio = fit_ratios[len(fit_ratios) // 2] if fit_ratios else 1.0
    unknown = 0
    for word in orig_count:
        if word == START:
            continue
        if new_count[word] is None:
            new_count[word] = max(1, round(orig_count[word] * ratio))
            unknown += 1
    # <s> 保留语料计数（句首条件概率的分母，只作前词、不作目标）
    new_count[START] = orig_count[START]

    # 二元重标定：普通前词按新边际缩放，条件概率不变；<s> 前的行原样保留
    new_bigram = []
    dropped = 0
    for first, rest in read_tsv(BIGRAM):
        second, pair_count = rest[0], int(rest[1])
        if first == START:
            new_bigram.append((first, second, pair_count))
            continue
        denom = orig_count.get(first, 0)
        if denom <= 0:
            dropped += 1
            continue
        rescaled = round(pair_count * new_count[first] / denom)
        if rescaled < 1:
            dropped += 1
            continue
        new_bigram.append((first, second, rescaled))

    with open(UNIGRAM + ".new", "w", encoding="utf-8") as out:
        for word, _ in unigram_rows:
            out.write(f"{word}\t{new_count[word]}\n")
    with open(BIGRAM + ".new", "w", encoding="utf-8") as out:
        for first, second, count in new_bigram:
            out.write(f"{first}\t{second}\t{count}\n")

    max_count = max(v for w, v in new_count.items() if w != START)
    print(
        f"known={known} fallback={unknown} fit_ratio={ratio:.4g} "
        f"max_unigram={max_count} bigrams={len(new_bigram)} dropped={dropped}",
        file=sys.stderr,
    )
    for word in ["手机", "世纪", "时间", "世界", "信息", "谢谢", "学校", "相信"]:
        print(f"{word}\t{new_count[word]}", file=sys.stderr)


if __name__ == "__main__":
    main()
