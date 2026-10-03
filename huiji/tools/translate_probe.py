#!/usr/bin/env python
# 端侧整句翻译探针：用 CTranslate2 + SentencePiece 加载离线模型，逐句翻译测试集，量推理与端到端延迟。
# 目的：拿到端侧在这台机器上的真实延迟与译文质量，判断端侧能不能作为默认路线。
# 用法：python translate_probe.py --model <模型目录> --input <测试集.tsv> --output <结果.tsv> [--compute int8] [--beam 2]
import argparse
import statistics
import time

import ctranslate2
import sentencepiece as spm


def read_rows(path):
    rows = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line.strip():
                continue
            cols = line.split("\t")
            if cols[0] == "编号":
                continue
            if len(cols) >= 2 and cols[0].isdigit():
                rows.append((cols[0], cols[1], cols[2] if len(cols) > 2 else ""))
            else:
                rows.append(("", line, ""))
    return rows


def summarize(values):
    ordered = sorted(values)
    n = len(ordered)
    p90 = ordered[min(int(n * 0.9), n - 1)]
    return (
        f"中位数={statistics.median(ordered)} 均值={round(statistics.mean(ordered), 1)} "
        f"P90={p90} 最快={min(ordered)} 最慢={max(ordered)}"
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--compute", default="int8")
    parser.add_argument("--beam", type=int, default=2)
    parser.add_argument("--rep-penalty", type=float, default=1.0)
    parser.add_argument("--no-repeat-ngram", type=int, default=0)
    args = parser.parse_args()

    source = spm.SentencePieceProcessor()
    source.load(f"{args.model}/source.spm")
    target = spm.SentencePieceProcessor()
    target.load(f"{args.model}/target.spm")

    translator = ctranslate2.Translator(args.model, device="cpu", compute_type=args.compute)
    rows = read_rows(args.input)

    decode_args = dict(
        beam_size=args.beam,
        max_decoding_length=256,
        repetition_penalty=args.rep_penalty,
        no_repeat_ngram_size=args.no_repeat_ngram,
    )

    def run_one(sentence):
        tokens = source.encode(sentence, out_type=str)
        result = translator.translate_batch([tokens], **decode_args)
        return target.decode(result[0].hypotheses[0])

    # 预热两句，把首次量化 / 构图开销排除掉
    if rows:
        run_one(rows[0][1])
        run_one(rows[0][1])

    lines = ["编号\t中文原句\t场景\t端侧译文\t推理ms\t端到端ms"]
    infer_times = []
    e2e_times = []
    for number, sentence, scene in rows:
        tokens = source.encode(sentence, out_type=str)
        started = time.perf_counter()
        result = translator.translate_batch([tokens], **decode_args)
        after_infer = time.perf_counter()
        translated = target.decode(result[0].hypotheses[0])
        ended = time.perf_counter()
        infer_ms = round((after_infer - started) * 1000, 1)
        e2e_ms = round((ended - started) * 1000, 1)
        infer_times.append(infer_ms)
        e2e_times.append(e2e_ms)
        lines.append(
            f"{number}\t{sentence}\t{scene}\t{translated}\t{infer_ms}\t{e2e_ms}"
        )

    print(f"句子数 {len(rows)}（compute_type={args.compute}, beam={args.beam}）")
    print(f"推理耗时(ms) {summarize(infer_times)}")
    print(f"端到端(ms) {summarize(e2e_times)}")
    with open(args.output, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
    print(f"已写 {args.output}")


if __name__ == "__main__":
    main()
