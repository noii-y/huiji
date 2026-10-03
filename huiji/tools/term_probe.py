# -*- coding: utf-8 -*-
"""术语表注入探针：验证把中文术语在源端替换成标准英文后，Marian 能否保留英文、组出通顺句。

一次性探针，用来决定术语表走哪条技术路径（源端替换 / 占位符 / 后处理）。
"""
import ctranslate2
import sentencepiece

MODEL = r"C:\Users\dril\AppData\Roaming\Qingjian\models\opus-mt-zh-en-ct2"

sp = sentencepiece.SentencePieceProcessor()
sp.load(MODEL + r"\source.spm")
tr = ctranslate2.Translator(MODEL, device="cpu", compute_type="int8")


def trans(text):
    toks = sp.encode(text, out_type=str)
    res = tr.translate_batch(
        [toks], beam_size=2, repetition_penalty=1.2, no_repeat_ngram_size=3
    )
    return sp.decode(res[0].hypotheses[0])


# (原句, [(中文术语, 标准英文)])
CASES = [
    ("摸鱼一时爽，一直摸鱼一直爽。", [("摸鱼", "slack off")]),
    ("这个 bug 我复现了，正在排查。", [("复现", "reproduce")]),
    ("这就是传说中的神仙颜值吗。", [("神仙颜值", "stunning looks")]),
    ("破防了家人们，真的好感动。", [("破防", "overwhelmed")]),
    ("帮我点一杯奶茶，三分糖去冰。", [("三分糖", "30% sugar")]),
    # 对照：高铁本就翻对，替换不应把它改坏
    ("我们坐高铁还是飞机去？", [("高铁", "high-speed rail")]),
]

for orig, subs in CASES:
    mod = orig
    for zh, en in subs:
        mod = mod.replace(zh, en)
    print("原句 : " + orig)
    print("  基线 : " + trans(orig))
    print("  替换 : " + trans(mod))
    print("")

# 占位符法试验：看模型是否原样保留没见过的占位标记
ph = "摸鱼 __TERM1__ 一时爽。"
print("占位符试验 : " + ph)
print("  -> " + trans(ph))
