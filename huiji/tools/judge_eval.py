# -*- coding: utf-8 -*-
import json, re, subprocess, urllib.request

GOLDEN = r"C:\Users\dril\OneDrive\Desktop\灰迹输入法\app\huiji\评测\排序\首选-标准答案.tsv"
CLI = ["cargo", "run", "-q", "-p", "qingjian-cli", "--"]
LLM = "http://127.0.0.1:8899/v1/chat/completions"

def candidates(pinyin, n=9):
    p = subprocess.run(CLI + [pinyin, "--limit", str(n)],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="ignore")
    out = []
    for line in p.stdout.splitlines():
        m = re.match(r"\s*(\d+)\.\s+(.+)$", line)
        if not m:
            continue
        text = m.group(2)
        # 截到注释/空格区
        text = re.split(r"\s{2,}|\[|·", text)[0].strip()
        if text and text not in out:
            out.append(text)
    return out

def judge(pinyin, cands, context):
    listing = "\n".join(f"{i+1}. {c}" for i, c in enumerate(cands))
    ctx = f"上文是“{context}”。" if context else ""
    user = (f"{ctx}用户输入拼音“{pinyin}”，下面是候选，"
            "选出最自然、最常用的一个，只输出它前面的编号数字，不要别的。\n"
            f"{listing}")
    body = {"model": "qwen",
            "messages": [{"role": "user", "content": user}],
            "temperature": 0.0, "max_tokens": 8,
            "chat_template_kwargs": {"enable_thinking": False}}
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(LLM, data=data,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        content = json.load(r)["choices"][0]["message"]["content"]
    m = re.search(r"\d+", content)
    return int(m.group()) - 1 if m else -1, content

rows = []
with open(GOLDEN, encoding="utf-8") as f:
    for line in f:
        line = line.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        target, pinyin = parts[0], parts[1]
        context = parts[2] if len(parts) > 2 else ""
        rows.append((target, pinyin, context))

stat_hit = judge_hit = 0
flips_good = flips_bad = 0
for target, pinyin, context in rows:
    cands = candidates(pinyin)
    if not cands:
        print(f"[no candidates] {pinyin}")
        continue
    stat_ok = target in cands[0]
    try:
        ji, raw = judge(pinyin, cands, context)
        jc = cands[ji] if 0 <= ji < len(cands) else ""
    except Exception as e:
        jc, raw = "", f"ERR {e}"
    judge_ok = target in jc
    stat_hit += stat_ok
    judge_hit += judge_ok
    if judge_ok and not stat_ok:
        flips_good += 1
        print(f"GOOD {pinyin}: {cands[0]} -> {jc} (目标 {target})")
    if stat_ok and not judge_ok:
        flips_bad += 1
        print(f"BAD  {pinyin}: {cands[0]} -> {jc} (目标 {target}) raw={raw!r}")

n = len(rows)
print(f"\n共 {n} 例")
print(f"统计首选命中: {stat_hit} ({stat_hit/n*100:.1f}%)")
print(f"裁判选择命中: {judge_hit} ({judge_hit/n*100:.1f}%)")
print(f"改对 {flips_good}，改错 {flips_bad}")
