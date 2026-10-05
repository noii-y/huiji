import json
import os

here = os.path.dirname(os.path.abspath(__file__))
soak_dir = os.path.join(here, "..", "评测", "soak")
d = json.load(open(os.path.join(soak_dir, "soak-result.json"), encoding="utf-8"))
lines = d["lines"]
recs = d["records"]
print("total lines:", len(lines))

problems = []
for r in recs:
    if r["line"] is None:
        continue
    ln = lines[r["line"]] if r["line"] < len(lines) else "<OOR>"
    kind = r["kind"]
    s = ln.strip()
    flag = ""
    if kind in ("word", "page", "proper"):
        # 应是中文 / 专有名词；若整行是小写 ASCII（拼音残留）说明没转
        if s and s.isascii() and s.replace("'", "").isalnum() and s == s.lower():
            flag = "NO-CONVERT?"
    if not s:
        flag = "EMPTY"
    if flag:
        problems.append((r["round"], kind, r["typed"], repr(ln), flag))
for p in problems:
    print(p)

print("--- raw rounds (应保留拼音) ---")
for r in recs:
    if r["kind"] == "raw":
        print(r["round"], r["line"],
              repr(lines[r["line"]]) if r["line"] < len(lines) else "OOR")

print("--- proper rounds ---")
for r in recs:
    if r["kind"] == "proper":
        print(r["round"], r["typed"],
              repr(lines[r["line"]]) if r["line"] < len(lines) else "OOR")

print("--- toggle rounds ---")
for r in recs:
    if r["kind"] == "toggle":
        print(r["round"], r["typed"],
              repr(lines[r["line"]]) if r["line"] < len(lines) else "OOR")
