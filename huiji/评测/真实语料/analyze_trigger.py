# -*- coding: utf-8 -*-
"""
用全量本地基线（约 16 万句）分析「本地置信度触发」：
找一条可操作规则（按整句长度 / 是否有整句候选），让「不上云的句子本地正确率尽量高」，
同时「上云比例可控」，给阶段 1 的触发阈值提供数据。
"""
import os, json, glob

BASE = os.path.dirname(os.path.abspath(__file__))
RES = os.path.join(BASE, 'results')

def hanzi(s):
    return sum(1 for c in s if '\u4e00' <= c <= '\u9fff')

rows = []
for path in sorted(glob.glob(os.path.join(RES, 's*.jsonl'))):
    kind = os.path.basename(path).split('.')[0]
    for line in open(path, 'r', encoding='utf-8'):
        line = line.strip()
        if not line:
            continue
        try:
            r = json.loads(line)
        except Exception:
            continue
        if r.get('error') or not r.get('text'):
            continue
        r['_kind'] = kind
        r['_len'] = hanzi(r['text'])
        r['_ok'] = (r.get('position') == 0)
        r['_has_sent'] = bool(r.get('sentence'))
        rows.append(r)

N = len(rows)
print('全量句数:', N)

# 1) 按长度分桶的本地首选率
print('\n== 按汉字数（≈音节数）分桶：本地首选率 ==')
buckets = [(1,2),(3,4),(5,6),(7,8),(9,12),(13,20),(21,99)]
for lo, hi in buckets:
    sub = [r for r in rows if lo <= r['_len'] <= hi]
    if not sub:
        continue
    acc = sum(r['_ok'] for r in sub) / len(sub)
    print(f'  {lo:>2}-{hi:<2}字: n={len(sub):>6}  本地首选 {acc:.1%}  占比 {len(sub)/N:.1%}')

# 2) 有无整句候选
print('\n== 本地是否给出整句候选（sentence）==')
for has in (True, False):
    sub = [r for r in rows if r['_has_sent'] == has]
    if not sub:
        continue
    acc = sum(r['_ok'] for r in sub) / len(sub)
    print(f'  has_sentence={has}: n={len(sub):>6} ({len(sub)/N:.1%})  本地首选 {acc:.1%}')

# 3) 模拟触发规则
def evaluate(rule, name):
    cloud = [r for r in rows if rule(r)]
    local = [r for r in rows if not rule(r)]
    cloud_ratio = len(cloud) / N
    local_acc = sum(r['_ok'] for r in local) / max(len(local), 1)
    cloud_err = 1 - sum(r['_ok'] for r in cloud) / max(len(cloud), 1)  # 上云句子的本地错误率=救回空间
    print(f'  {name:<34} 上云 {cloud_ratio:5.1%}  留下本地正确率 {local_acc:5.1%}  上云句本地错误率 {cloud_err:5.1%}')

print('\n== 触发规则模拟（目标：留下本地正确率高、上云比例可控）==')
for n in (4, 5, 6, 7, 8, 9, 10):
    evaluate(lambda r, n=n: r['_len'] >= n, f'规则A: 长度>={n}字 上云')
print()
evaluate(lambda r: not r['_has_sent'], '规则B: 本地无整句候选 上云')
for n in (5, 6, 7, 8):
    evaluate(lambda r, n=n: r['_len'] >= n or not r['_has_sent'], f'规则C: 长度>={n} 或 无整句候选')
