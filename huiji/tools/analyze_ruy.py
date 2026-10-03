"""统计 Rust 端侧（Ruy）批量翻译结果：延迟分布 + 译文重复退化检测。

用法：python analyze_ruy.py <结果.tsv>
结果 TSV 由 cli --translate-file --translate-save 产出，列含「云端译文」「云端延迟(ms)」。
"""

import csv
import re
import statistics
import sys


def percentile(sorted_values, p):
    """线性插值百分位。"""
    if not sorted_values:
        return 0
    k = (len(sorted_values) - 1) * p
    low = int(k)
    high = min(low + 1, len(sorted_values) - 1)
    if low == high:
        return sorted_values[low]
    return sorted_values[low] + (sorted_values[high] - sorted_values[low]) * (k - low)


def repetition_flags(text):
    """检测相邻重复词与窗口内重复三词块，返回两个布尔。"""
    words = re.findall(r"[a-z']+", text.lower())
    adjacent = any(words[i] == words[i + 1] for i in range(len(words) - 1))
    seen = set()
    repeated_trigram = False
    for i in range(len(words) - 2):
        trigram = " ".join(words[i : i + 3])
        if trigram in seen:
            repeated_trigram = True
            break
        seen.add(trigram)
    return adjacent, repeated_trigram


def main():
    path = sys.argv[1]
    with open(path, encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle, delimiter="\t"))

    latencies = sorted(int(row["云端延迟(ms)"]) for row in rows)
    print(f"句数 {len(latencies)}")
    print(f"延迟中位数 {statistics.median(latencies)}")
    print(f"延迟均值 {round(statistics.mean(latencies), 1)}")
    print(f"P90 {round(percentile(latencies, 0.9), 1)}")
    print(f"最快 {min(latencies)}  最慢 {max(latencies)}")
    within_800 = sum(1 for value in latencies if value <= 800)
    print(f"≤800ms 句数 {within_800}（{within_800}%）")

    marked = []
    empty = 0
    for row in rows:
        translation = row["云端译文"]
        if not translation.strip() or "没给译文" in translation:
            empty += 1
        adjacent, trigram = repetition_flags(translation)
        if adjacent or trigram:
            kind = ("相邻重复 " if adjacent else "") + ("三词块重复" if trigram else "")
            marked.append((row["编号"], kind.strip(), translation))
    print(f"空译文 {empty}")
    print(f"重复退化句数 {len(marked)}")
    for number, kind, translation in marked:
        print(f"  #{number} [{kind}] {translation}")


if __name__ == "__main__":
    main()
