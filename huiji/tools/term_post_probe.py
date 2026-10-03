# -*- coding: utf-8 -*-
"""术语表后处理探针（二，修订）。

验证后处理：让模型正常翻译，再把「正常英语不会出现、模型稳定硬译」的短语
替换成标准译法，正确处理时态人称，看替换后是否通顺、是否误伤。
"""
import ctranslate2
import re
import sentencepiece

MODEL = r"C:\Users\dril\AppData\Roaming\Qingjian\models\opus-mt-zh-en-ct2"

src_sp = sentencepiece.SentencePieceProcessor()
src_sp.load(MODEL + r"\source.spm")
tgt_sp = sentencepiece.SentencePieceProcessor()
tgt_sp.load(MODEL + r"\target.spm")
tr = ctranslate2.Translator(MODEL, device="cpu", compute_type="int8")


def trans(text):
    toks = src_sp.encode(text, out_type=str)
    res = tr.translate_batch(
        [toks], beam_size=2, repetition_penalty=1.2, no_repeat_ngram_size=3
    )
    return tgt_sp.decode(res[0].hypotheses[0])


# 摸鱼动词形态 -> slack 对应形态；只替换正常英语不出现的 feel/touch fish。
FISH_VERB = {
    "feel": "slack", "feels": "slacks", "feeling": "slacking", "felt": "slacked",
    "touch": "slack", "touches": "slacks", "touching": "slacking", "touched": "slacked",
}
FISH_RE = re.compile(
    r"\b(feel|feels|feeling|felt|touch|touches|touching|touched)\s+(?:the\s+)?fish\b",
    re.IGNORECASE,
)


def post(sentence):
    def repl(m):
        return FISH_VERB.get(m.group(1).lower(), "slack") + " off"
    sentence = FISH_RE.sub(repl, sentence)
    sentence = re.sub(r"\binner\s+volume\b", "rat race", sentence, flags=re.IGNORECASE)
    return sentence


print("== 摸鱼：多时态验证 ==")
FISH = [
    "摸鱼一时爽，一直摸鱼一直爽。",
    "他上班总是摸鱼。",
    "今天不想干活，只想摸鱼。",
    "别摸鱼了，老板来了。",
    "摸鱼摸了一整天。",
]
for zh in FISH:
    base = trans(zh)
    print("原 : " + zh)
    print("  基线  : " + base)
    print("  后处理: " + post(base))
print("")

print("== 内卷：多句验证 ==")
JUAN = [
    "现在年轻人都在内卷。",
    "职场内卷太严重了。",
    "我不想再内卷了。",
]
for zh in JUAN:
    base = trans(zh)
    print("原 : " + zh)
    print("  基线  : " + base)
    print("  后处理: " + post(base))
