# 阶段 3：横切 QC 问题清单（独立抽验）

> 执行人：独立横切 QC，不参与开发。只读审查 + 临时目录破坏实验，未改 A/B/C 任何交付物与仓库公共文件；
> 未 git add/commit/push；未 cargo build 全量；未写 data/generated；未做真机/重启/打包。
> 唯一写动作 = 本文件。临时验证产物均在 `target/hj-qc-verify/`。
> 日期：2026-10-17。cli = `target/release/qingjian-cli.exe`（--eval-text 冷启动不学习不写用户数据）。

---

## A 分片（离线补词，`阶段3-高频错例与补词.md` + `target/hj-patch-verify/patch_full.tsv`）

### 已核验通过

- **A1a 自造锁定集**：无补丁跑 `huiji/评测/排序/首选-标准答案.tsv` = 133 句、首选 98.5%、前三 99.2%，未命中「行(xing)」「什么狗屁玩意(shenmegouspiwanyi)」；加 `patch_full.tsv` 复跑仍 98.5%/99.2%、未命中句完全相同。与报告 §5 一致。
  - 命令：`qingjian-cli.exe --eval-text <tsv>` / `--extra-dict target/hj-patch-verify/patch_full.tsv`
- **A1c 关键机制结论复现**：独立单查 `xiecheng` 加补丁后首选仍「写成」(#2 携程)——证实报告「附加词库不盖主库同拼音词」成立；`tuifang` 推翻#1→**退房#1**；`mentong` 们同#1→**门童#1**；`en` 出「嗯」但排 #2（恩#1），与报告「恩#1/嗯#2，需重跑 bigram 才首选」一致。
- **A1d 基线同源（重点）**：从基线 jsonl 固定规则抽 341 句（现代五类各每 100 行取 1、≤70/类），当前 cli **无补丁**重跑的逐句 `top` 与 jsonl 记录的 `top` **341/341 完全一致**。另从 jsonl 全量复算各类首选率 = 56.6/56.6/61.0/60.2/49.6/16.3，与 `baseline_summary.txt` 逐类吻合。→ 基线数据与补丁侧 cli/词库同源，报告「基线取自既有 jsonl top」的数字诚实，无不同源失真。
- **A1b 补丁净效应**：同一份 341 句样本，无补丁首选 204/341=59.8%，加补丁 204/341=59.8%；逐句 diff **修好 0、新错 0**。与报告「修好 5（退房×2/门童×2/播放器×1）、新错 0」量级一致——我的样本小且未恰好抽中那 5 个稀有 OOV，无新错聚集。
- **A2 补丁格式**：426 行全部 3 字段、无重复词、拼音音节全部合法、字数=音节数；抽 5 个多音字/生僻词（元大都/九曲回肠/东交民巷/侵略者/原种场）在其补丁拼音下均作首选出现，拼音正确。
- **A2 命名风险音节**：`fangjia` 仍放假#1、`fangfei` 仍放飞#1、`sixing` 仍死刑#1、`zhengjie` 仍正解#1、`xiecheng` 仍写成#1——报告点名怕被顶的常用词全部未被顶掉。

### 问题清单

| # | 严重度 | 问题 | 证据 |
|---|---|---|---|
| A-1 | **should-fix** | `patch_full.tsv` 带 UTF-8 BOM（首字节 EF BB BF），而词典加载器 `Dictionary::parse` 用 `line.trim()` 不剥 U+FEFF（FEFF 非 Unicode 空白），导致首行「嗯」被存成 `U+FEFF+嗯`。若原样随包/合并，候选里带不可见 BOM，上屏会往用户文本里插 U+FEFF。 | 实跑 `--extra-dict patch_full.tsv en`，候选第 2 位 codepoint = `U+FEFF U+55EF`（落盘 hexdump 核实）；源码 `crates/qingjian-dictionary/src/dictionary/mod.rs:97-100` 仅 `trim()`+`starts_with('#')`，无 BOM 处理。做无 BOM 对照后干净「嗯」排 #8（而非 #2），进一步坐实 BOM 是元凶。修法：交付前去 BOM（仅首行受影响）。 |
| A-2 | should-fix | 报告 §3.1 表格与实际 `patch_full.tsv` 数值/词表对不上：① 嗯 freq 报告写 340105，文件实为 **400000**；② 四星 freq 报告写 3000，文件实为 **8000**；③ 报告 §3.1 附加词库表**未列携程**（§3.2 说携程须进主库），但文件第 2 行有 `携程 xie cheng 20000`；④ 播放器报告列为手挑词，文件里却在第 205 行 freq=145（埋在上游批次里）。 | 逐行核对 `patch_full.tsv`：行1 嗯 en 400000；行2 携程 xie cheng 20000；行4 四星 si xing 8000；行205 播放器 bo fang qi 145。携程在附加词库实测无害（xiecheng 仍写成#1），但报告描述应与文件一致。 |
| A-3 | nit | 报告 §5 称「常用词独立首选全部保持 #1」，但实测上游高频词 `citiao` 加补丁后首选由「此条」变为「词条」（补丁 `词条 ci tiao 18486`）。这是报告未枚举到的 top1 变化；判定中性偏正面（词条比此条更常用），且 341 句真实样本 0 新错、自造集 0 退化，不影响结论。 | 无补丁 `citiao`=此条；加补丁=词条。 |
| A-4 | nit | 报告 §4「3612 句、56.4%→56.5%」我未复现其精确样本（其采样脚本 sample.py 每类 700 步长抽样），但独立 341 句样本已证实「净改善为正方向、0 新错」，+0.1pp 在噪声内，报告 §8 也自述置信区间 ±1.6pp、不夸大。 | cmp.py 结果：0 修好/0 新错。 |

---

## B 分片（release-gate.ps1 + 门禁实跑报告 + 发版SOP）

### 已核验通过

- **B1 端到端实跑**：`powershell -ExecutionPolicy Bypass -File huiji\评测\release-gate.ps1` 复跑 = `RELEASE GATE: PASS`、退出码 0、**耗时 159s（约 2.7 分钟）**，符合「几分钟内跑完」。脚本开头自设 Path/LIBRARY_PATH/QINGJIAN_UIACCESS=0 三变量（脚本 35-37 行）。五 crate 单测真跑（core 329 / predict local-nmt-system 44 / server 20 / tsf 18 / platform 48，全 `--lib`），非跳过。采样各项数字与 B 报告逐字一致。
- **B2 FAIL 机制（关键）**：把脚本复制到 `target/hj-qc-verify/gate-failtest.ps1`（原脚本未改），仅把自造集阈值 `98.5→99.9`，实跑正确输出 `[FAIL] 锁定集首选率`、`RELEASE GATE: FAIL`、**退出码=1**。FAIL 路径有效。
- **B3 固定种子可复现**：我的独立复跑与 B 报告的采样首选率/句数逐项相同（s01 59.2%/2937、s02 57.5%/11545、s03 62.1%/5848、s04 61.6%/1222、s05 51.5%/7950、s06 15.9%/24108）。种子 4242 Fisher-Yates 确定性复现已获证据。
- **B3 古典类只参考**：脚本 129-135 行，`i>=5` 走 `[INFO]` 分支、不置 `$script:Failed`，s06 不参与 PASS/FAIL。
- **B5 脚本卫生**：脚本无 git 写操作、无 `cargo build --release`、无写 `data/generated/`、无改公共层；只 `cargo test`（允许）+ cli 只读评测；采样临时文件写 `%TEMP%\huiji-release-gate`。

### 重试逻辑评估（B 重点问题）

- 脚本 67-72 行：任一 crate 首次 `cargo test` 非零即重试一次。**判定：合理缓冲，不是放水**。理由：① 稳定失败（真退化）重跑仍失败，`$code` 仍非零 → Assert-Gate 正确 FAIL；② 仅重试一次、有界，且打印失败尾部 8 行；③ B 报告已坦承这只是缓冲、根治需产品侧改 tsf `anchor.rs` 测试隔离（`serial_test`/实例态），并提请 PM。残留风险：若某测试只在并行时偶发且偶发窗口恰好掩盖了一个真退化，单次重试可能漏——但概率低，且 B 已把根因挂账。可接受。

### 问题清单

| # | 严重度 | 问题 | 证据 |
|---|---|---|---|
| B-1 | nit | 采样按「1000 行」而非「1000 句」抽样，长行（酒店评论、红楼长段）贡献更多短句，而短句首选率本就更高（阶段0 报告：3-4 字 57% vs 13-20 字 36%），导致实测采样率系统性比全量基线高约 +1.6pp（s01 +2.6/s02 +0.9/s03 +1.1/s04 +1.4/s05 +1.9）。叠加 2pp 容差，实际灵敏度约 3.5pp。方向偏松（会放过 <3.5pp 的真退化），B 报告 §七.2 已自述「亚 2pp 漂移会放过、靠全量复盘」。建议 PM 知悉。 | 我用 jsonl 全量复算基线率 = baseline_summary；采样率来自 gate-rerun.log；SE 按二项比例估算 n=2937..11545 时 1σ≈0.4-0.9pp。 |
| B-2 | nit | SOP 中个别预期偏主观：C6「译文完整对应全句，不漏词、不截断」需人工逐词比对；C2「位置为首位/可 Tab 接受项」用「/」使判定标准略含糊（首位是必须还是可接受？）。其余 A1-A5/B1-B3/C1-C10 均有可照做步骤 + 可判定预期/日志。所有真机/重启项（A5、B/C 两节）均已标 **[真机/重启]**。 | `huiji/发版SOP.md` C2/C6。 |

---

## C 分片（`huiji/发版卫生审查.md`，轻量抽查）

### 已核验通过（5 处 file:line 引用全部属实）

- `apps/windows/installer/qingjian.iss:272-293` = `DeleteStaleDlls` 过程体；`:278` = `FindFirst('qingjian_tsf*.dll')` 通配符；`:286` = `RestartReplace(Path,'')`。逐项属实。
- `apps/windows/installer/settings-runtime.txt:43` = 「各语言 WinUI 资源目录」注释；`:44` = af-za 起、`:129` = zh-tw 止，44-129 共 86 个语言目录，与「86 个」一致。
- `crates/qingjian-platform/src/config/general.rs:26/33/207-209` = `learning_language` 说明/字段/`learning_language_off()`；grep `emoji`/`english_in_chinese` 在该文件零命中（佐证 #104 开关不在本仓）。

C 分片未发现失实引用。

---

## 总评

- **A 分片：修复后可交 PM 终验。** 核心机制结论（extra-dict 不盖主库、修好 5/新错 0、自造集不退化、基线同源）全部独立复现成立。但有 1 个 should-fix：patch_full.tsv 的 UTF-8 BOM 会把首行「嗯」存成 `U+FEFF嗯`，合并前去 BOM 即可；另有报告与补丁文件的 freq/词表 4 处不一致需对齐。
- **B 分片：可交 PM 终验。** 门禁端到端 PASS、FAIL 路径有效、固定种子可复现、脚本卫生干净、重试逻辑是合理缓冲非放水。仅 2 个 nit（采样按行偏高约 1.6pp、SOP 两处预期略主观），不阻塞。
- **C 分片：可交 PM 终验。** 抽查 5 处引用全部属实。

红线遵守：本 QC 未改 A/B/C 交付物与仓库公共文件；破坏实验只在 `target/hj-qc-verify/` 副本；未 git 操作、未 cargo 全量 build、未写 data/generated、未真机/重启/打包。不宣布验收通过。
