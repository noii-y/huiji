# -*- coding: utf-8 -*-
"""
阶段 0 云对比：从本地基线 JSONL 分层抽样约 100 句，用与灰迹完全相同的提示词请求 DeepSeek，
对比「本地整句」与「云整句」的整句正确率、字准确率，并统计 token 用量。
密钥只从环境变量 HUIJI_EVAL_KEY 读，不写进文件、不入库。
"""
import os, json, glob, random, time, urllib.request, ssl, sys

BASE = os.path.dirname(os.path.abspath(__file__))
RES = os.path.join(BASE, 'results')
PER_KIND = 17          # 每类抽样句数，六类约 100 句
SEED = 42
BASE_URL = 'https://api.deepseek.com'
MODEL = 'deepseek-v4-flash'

def load_key():
    """优先环境变量；否则从本机 config.toml 解析，避免命令行里出现密钥。"""
    k = os.environ.get('HUIJI_EVAL_KEY', '').strip()
    if k:
        return k
    cfg = os.path.join(os.environ.get('APPDATA', ''), 'Qingjian', 'config.toml')
    if os.path.exists(cfg):
        import re
        for line in open(cfg, 'r', encoding='utf-8', errors='ignore'):
            m = re.search(r'^\s*api_key\s*=\s*"([^"]+)"', line)
            if m and m.group(1).startswith('sk-'):
                return m.group(1).strip()
    return ''

API_KEY = load_key()

SYSTEM_PROMPT = (
    "你是一个拼音输入法的云端联想引擎。用户正在打拼音，还没选词。你会收到一段 JSON："
    "letters（用户实际敲的字母，可能有错字、漏字、多字、音节切错）、pinyin（输入法按 letters 做的切分，' 分隔音节，单个字母是声母缩写，切分可能是错的）、"
    "syllables（切分出的音节数，仅供参考）、before / after（当前光标前后的文本，应用给不出时为空）、"
    "local_sentence（本地整句转换的结果，可能错）、local_candidates（本地词库排在前面的候选，第一个是本地首选）、max_items、want_sentence。\n\n"
    "local_candidates 和 local_sentence 只是本地的猜测，可能全错。它们的用途是告诉你本地已经能给什么："
    "和它们重复的词会被丢掉，所以不要照抄；也不要被它们带偏——请只根据 letters 与 before / after 独立判断用户想打什么。\n\n"
    "输出 JSON：{\"words\": [{\"text\": \"…\", \"pinyin\": \"…\"}], \"sentence\": \"…\" 或 null}\n\n"
    "sentence：want_sentence 为 true 时，把 letters 还原成用户本来想打的那一句中文。用户可能打错、漏打、多打了字母，或用了声母缩写，你的任务是纠错、还原本意，而不是续写。"
    "结果要和 letters 的拼音对应，只纠错、不扩写：用户没敲的意思不要自行补成更长的句子，不要接下文，也不要加任何评论；"
    "不能有把握地还原成完整一句，就给 null。语言跟随上下文。不解释、不加引号、不加序号。"
)

def hanzi_count(s):
    return sum(1 for c in s if '\u4e00' <= c <= '\u9fff')

def char_acc(a, b):
    """与评测器同口径：按位比对，分母取两者长度。"""
    if not a or not b:
        return 0.0, 0
    n = max(len(a), len(b))
    same = sum(1 for x, y in zip(a, b) if x == y)
    return same / n, n

def load_samples():
    random.seed(SEED)
    samples = []
    for path in sorted(glob.glob(os.path.join(RES, 's*.jsonl'))):
        kind = os.path.basename(path).split('.')[0]
        rows = []
        with open(path, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    r = json.loads(line)
                except Exception:
                    continue
                if r.get('error') or not r.get('pinyin') or not r.get('text'):
                    continue
                rows.append(r)
        random.shuffle(rows)
        for r in rows[:PER_KIND]:
            r['_kind'] = kind
            samples.append(r)
    return samples

def call_cloud(r):
    letters = r['pinyin']
    text = r['text']
    payload_user = {
        'letters': letters,
        'pinyin': letters,
        'syllables': hanzi_count(text),
        'before': r.get('context', '') or '',
        'after': '',
        'local_sentence': r.get('sentence') or r.get('top') or '',
        'local_candidates': (r.get('candidates') or [])[:5],
        'max_items': 2,
        'want_sentence': True,
    }
    body = {
        'model': MODEL,
        'messages': [
            {'role': 'system', 'content': SYSTEM_PROMPT},
            {'role': 'user', 'content': json.dumps(payload_user, ensure_ascii=False)},
        ],
        'max_tokens': 200,
        'temperature': 0.3,
        'response_format': {'type': 'json_object'},
        'reasoning_effort': 'none',
    }
    data = json.dumps(body, ensure_ascii=False).encode('utf-8')
    req = urllib.request.Request(
        BASE_URL + '/chat/completions',
        data=data,
        headers={'Authorization': 'Bearer ' + API_KEY, 'Content-Type': 'application/json'},
        method='POST',
    )
    ctx = ssl.create_default_context()
    with urllib.request.urlopen(req, timeout=60, context=ctx) as resp:
        out = json.loads(resp.read().decode('utf-8'))
    content = out['choices'][0]['message']['content']
    usage = out.get('usage', {})
    try:
        sentence = json.loads(content).get('sentence')
    except Exception:
        sentence = None
    return (sentence or '').strip(), usage

def main():
    if not API_KEY:
        print('未设置 HUIJI_EVAL_KEY 环境变量，退出')
        sys.exit(1)
    samples = load_samples()
    print('抽样句数:', len(samples))
    stats = {
        'n': 0,
        'local_sent_hit': 0, 'cloud_sent_hit': 0,
        'local_char_same': 0, 'cloud_char_same': 0, 'char_total': 0,
        'rescue': 0, 'break': 0, 'both_wrong': 0, 'both_right': 0,
        'cloud_empty': 0,
        'prompt_tokens': 0, 'completion_tokens': 0, 'total_tokens': 0,
    }
    by_kind = {}
    detail = []
    for i, r in enumerate(samples):
        text = r['text']
        local = r.get('sentence') or ''
        try:
            cloud, usage = call_cloud(r)
        except Exception as e:
            print(f'[{i+1}] 请求失败: {e}')
            time.sleep(1)
            continue
        local_ok = (local == text)
        cloud_ok = (cloud == text)
        la, cn = char_acc(local, text)
        ca, _ = char_acc(cloud, text)
        stats['n'] += 1
        stats['local_sent_hit'] += local_ok
        stats['cloud_sent_hit'] += cloud_ok
        stats['local_char_same'] += round(la * cn)
        stats['cloud_char_same'] += round(ca * cn)
        stats['char_total'] += cn
        if local_ok and cloud_ok:
            stats['both_right'] += 1
        elif (not local_ok) and cloud_ok:
            stats['rescue'] += 1
        elif local_ok and not cloud_ok:
            stats['break'] += 1
        else:
            stats['both_wrong'] += 1
        if not cloud:
            stats['cloud_empty'] += 1
        stats['prompt_tokens'] += usage.get('prompt_tokens', 0)
        stats['completion_tokens'] += usage.get('completion_tokens', 0)
        stats['total_tokens'] += usage.get('total_tokens', 0)
        k = r['_kind']
        bk = by_kind.setdefault(k, {'n': 0, 'local': 0, 'cloud': 0})
        bk['n'] += 1
        bk['local'] += local_ok
        bk['cloud'] += cloud_ok
        detail.append({'kind': k, 'text': text, 'pinyin': r['pinyin'],
                       'local': local, 'cloud': cloud,
                       'local_ok': local_ok, 'cloud_ok': cloud_ok})
        if (i + 1) % 10 == 0:
            print(f'已完成 {i+1}/{len(samples)}')
        time.sleep(0.1)

    with open(os.path.join(RES, 'cloud_compare_detail.jsonl'), 'w', encoding='utf-8') as f:
        for d in detail:
            f.write(json.dumps(d, ensure_ascii=False) + '\n')

    n = stats['n'] or 1
    print('\n========== 云 vs 本地（约 100 句分层抽样）==========')
    print(f"样本数 {stats['n']}")
    print(f"整句正确率  本地 {stats['local_sent_hit']/n:.1%}  云 {stats['cloud_sent_hit']/n:.1%}")
    print(f"字准确率    本地 {stats['local_char_same']/max(stats['char_total'],1):.1%}  云 {stats['cloud_char_same']/max(stats['char_total'],1):.1%}")
    print(f"本地错云救回 {stats['rescue']}  本地对云改错 {stats['break']}  都对 {stats['both_right']}  都错 {stats['both_wrong']}  云给空 {stats['cloud_empty']}")
    print(f"token 合计 {stats['total_tokens']}（prompt {stats['prompt_tokens']} / completion {stats['completion_tokens']}），平均每句 {stats['total_tokens']/n:.0f}")
    print('\n分类别整句正确率:')
    for k in sorted(by_kind):
        b = by_kind[k]
        print(f"  {k}: 本地 {b['local']/b['n']:.0%}  云 {b['cloud']/b['n']:.0%}  (n={b['n']})")

if __name__ == '__main__':
    main()
