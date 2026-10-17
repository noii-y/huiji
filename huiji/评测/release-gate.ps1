#requires -Version 5.1
<#
.SYNOPSIS
  灰迹输入法发版前本地质量门禁（cargo test + 已编译 cli 只读评测；不做 release 构建、不写用户数据）。

.DESCRIPTION
  三道门禁，全部跑完再下总判定：
    1) 全量单测：core / predict(--features local-nmt-system) / server / tsf / platform，均 --lib
       （--lib 避免编译跑无关二进制的测试；predict 必须带 local-nmt-system，否则端侧翻译那条
       编译路径根本没被测到）。失败自动重试一次（tsf anchor 测试曾因共享静态并行竞态偶发误败，
       单独/重跑即过；真退化会复现，重试救不掉）。
    2) 自造锁定集：huiji/评测/排序/首选-标准答案.tsv（133 句）首选率 >= 98.5% 不退化。
    3) 真实语料句子级采样：从 huiji/评测/真实语料/corpus/ 的 s01..s05 现代五类各按固定种子抽
       2000 个「纯汉字小句」（切句口径与 cli apps/cli/src/eval/extract.rs 完全一致：按非汉字切开，
       只留 3~20 字纯汉字句），实测首选率不得低于「本脚本内置基准 - 2×SE」；s06 古典文学不设
       阈值，仅打印参考值。

  退出码：0 = PASS，1 = FAIL（任一门禁不过），2 = 环境错误（缺 cli / cargo）。

.NOTES
  环境约定（本机，勿删）：
    - ct2 runtime 目录进 PATH、LIBRARY_PATH 指向 HuijiSDK/ct2/lib（predict 链接预编译 ctranslate2.dll）；
    - QINGJIAN_UIACCESS=0：server 测试二进制嵌 uiAccess manifest，未签名起不来（os error 740）。
  采样与阈值口径（2026-10-18 校准，替代旧的「按行抽 1000 行」口径）：
    - 旧口径按行抽，长行被 cli 切成更多短句、短句首选率天然偏高，五类采样率系统性高于全量
      基线约 +1.6pp，门禁盲区偏向放行。改为句级采样后，采样率即本分布的无偏估计，基准就是
      「无补丁当前数据」的实测值（见 $modernBaseline 表）。
    - 阈值 = 实测基准 p − 2×SE，SE = sqrt(p(1−p)/N) 为二项标准误，N 为 cli 实际评测句数
      （部分小句含词库查不到读音的字，会被 cli 跳过，N 小于 2000，以 cli 输出为准）。
      2×SE 即约 95% 置信区间半宽：产品无退化时误杀概率约 2.3%，真实退化幅度超过该宽度即 FAIL。
    - 采样量 2000 句/类：单类评测约数秒，六类全流程几分钟内可跑完；SE≈1.1pp，阈值半宽≈2.2pp。
    - 古典文学（s06）古白话用词问题，不纳入现代类门禁，仅打印参考值。
#>

$ErrorActionPreference = 'Continue'

# ---------- 环境（本机约定，勿删） ----------
$env:Path = "$env:LOCALAPPDATA\HuijiSDK\ct2\bin;D:\Dev\.cargo\bin;D:\Git\cmd;$env:Path"
$env:LIBRARY_PATH = "$env:LOCALAPPDATA\HuijiSDK\ct2\lib"
$env:QINGJIAN_UIACCESS = "0"

$appRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $appRoot

$cli = Join-Path $appRoot 'target\release\qingjian-cli.exe'
if (-not (Test-Path $cli)) { Write-Host "ENV ERROR: 找不到 cli 二进制: $cli"; exit 2 }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Write-Host 'ENV ERROR: cargo 不在 PATH'; exit 2 }

$script:Failed = $false
function Assert-Gate([string]$name, [bool]$ok, [string]$actual, [string]$threshold) {
  $tag = if ($ok) { 'PASS' } else { 'FAIL' }
  Write-Host ("[{0}] {1}`n      实际值: {2}`n      阈值:   {3}" -f $tag, $name, $actual, $threshold)
  if (-not $ok) { $script:Failed = $true }
}

# ---------- 门禁 1/3: 全量单测 ----------
Write-Host "`n===== 门禁 1/3: 全量单测 (--lib) ====="
$tests = @(
  @{ Name = 'qingjian-core';            Args = @('-p', 'qingjian-core', '--lib') },
  @{ Name = 'qingjian-predict(local-nmt-system)'; Args = @('-p', 'qingjian-predict', '--lib', '--features', 'local-nmt-system') },
  @{ Name = 'qingjian-windows-server';  Args = @('-p', 'qingjian-windows-server', '--lib') },
  @{ Name = 'qingjian-windows-tsf';     Args = @('-p', 'qingjian-windows-tsf', '--lib') },
  @{ Name = 'qingjian-platform';        Args = @('-p', 'qingjian-platform', '--lib') }
)
foreach ($t in $tests) {
  $out = & cargo test $t.Args 2>$null
  $code = $LASTEXITCODE
  # 已知 tsf 的 anchor 两个测试共享 LAST_LINE_HEIGHT 静态、并行跑存在竞态（偶发误败，
  # 单独/重跑即过），此处对失败重试一次区分真退化与测试竞态；真失败会复现，重试救不掉。
  if ($code -ne 0) {
    Start-Sleep -Milliseconds 500
    Write-Host ("      " + $t.Name + " 首次失败，重试一次...")
    $out = & cargo test $t.Args 2>$null
    $code = $LASTEXITCODE
  }
  $summary = ($out | Select-String -Pattern 'test result:' | Select-Object -Last 1).Line
  if (-not $summary) { $summary = '(无 test result 输出)' }
  Assert-Gate ("单测 " + $t.Name) ($code -eq 0) (($summary.Trim()) + " [exit=$code]") 'cargo test --lib 退出码 0'
  if ($code -ne 0) { Write-Host "      --- 失败尾部输出 ---"; ($out | Select-Object -Last 8) | ForEach-Object { Write-Host ("      " + $_) } }
}

# ---------- 门禁 2/3: 自造锁定集 ----------
Write-Host "`n===== 门禁 2/3: 自造锁定集首选率 ====="
$lockSet = Join-Path $PSScriptRoot '排序\首选-标准答案.tsv'
$out = & $cli --eval-text $lockSet 2>$null
$text = ($out -join "`n")
$mN = [regex]::Match($text, '句子\s+(\d+)\s*条')
$m1 = [regex]::Match($text, '首选\s+([\d.]+)%')
if (-not $m1.Success) {
  Assert-Gate '锁定集首选率' $false 'cli 输出解析失败' '首选 >= 98.5%'
} else {
  $pct = [double]$m1.Groups[1].Value
  $n = if ($mN.Success) { $mN.Groups[1].Value } else { '?' }
  Assert-Gate '锁定集首选率' ($pct -ge 98.5) ("{0}% ({1} 句)" -f $pct, $n) '>= 98.5% 不退化'
}

# ---------- 门禁 3/3: 真实语料句子级采样 ----------
# 与 cli eval/extract.rs 同一口径：按非汉字切分，只留 3~20 字纯汉字小句（含扩展 B surrogate 对）。
function Test-HanCp([int]$code, [string]$line, [int]$i, [int]$len) {
  if ($code -ge 0xD800 -and $code -le 0xDBFF -and ($i + 1) -lt $len) {
    $cp = [System.Char]::ConvertToUtf32($line.Substring($i, 2), 0)
    return ($cp -ge 0x20000 -and $cp -le 0x323AF)
  }
  return (($code -ge 0x3400 -and $code -le 0x4DBF) -or ($code -ge 0x4E00 -and $code -le 0x9FFF) -or ($code -ge 0xF900 -and $code -le 0xFAFF))
}
function Extract-HanClauses([string]$line) {
  $out = New-Object System.Collections.Generic.List[string]
  $sb = New-Object System.Text.StringBuilder
  $len = 0
  $i = 0
  $n = $line.Length
  while ($i -lt $n) {
    $code = [int][char]$line[$i]
    if (Test-HanCp $code $line $i $n) {
      if ($code -ge 0xD800 -and $code -le 0xDBFF) { [void]$sb.Append($line.Substring($i, 2)); $i += 2 }
      else { [void]$sb.Append($line[$i]); $i++ }
      $len++
    } else {
      if ($len -ge 3 -and $len -le 20) { $out.Add($sb.ToString()) }
      [void]$sb.Clear(); $len = 0; $i++
    }
  }
  if ($len -ge 3 -and $len -le 20) { $out.Add($sb.ToString()) }
  return ,$out
}

$corpusDir = Join-Path $PSScriptRoot '真实语料\corpus'
$SampleClauses = 2000
$Seed = 4242
# 句子级采样校准基准（2026-10-18，无补丁当前数据实测；与 s01..s05 文件排序一一对应）：
#   p = 实测首选率%, n = cli 实际评测句数；阈值下限 = p − 2×SE，SE = sqrt(p(1−p)/n)。
$modernBaseline = @(
  @{ p = 60.3; n = 1875 },   # s01_外卖口语      → 阈值下限 58.04% (2×SE 2.26pp)
  @{ p = 57.1; n = 1957 },   # s02_酒店口语      → 阈值下限 54.86% (2×SE 2.24pp)
  @{ p = 62.9; n = 1958 },   # s03_电商评论      → 阈值下限 60.72% (2×SE 2.18pp)
  @{ p = 60.8; n = 1985 },   # s04_日常通用      → 阈值下限 58.61% (2×SE 2.19pp)
  @{ p = 50.6; n = 1984 }    # s05_新闻报纸      → 阈值下限 48.36% (2×SE 2.24pp)
)
$tmpDir = Join-Path $env:TEMP 'huiji-release-gate'
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
Write-Host "`n===== 门禁 3/3: 真实语料句子级采样 (种子 $Seed, 每类 $SampleClauses 句) ====="

$files = Get-ChildItem $corpusDir -Filter 's*.txt' | Sort-Object Name
for ($i = 0; $i -lt $files.Count; $i++) {
  $f = $files[$i]
  $pool = New-Object System.Collections.Generic.List[string]
  foreach ($ln in [System.IO.File]::ReadAllLines($f.FullName, [System.Text.Encoding]::UTF8)) {
    $cl = Extract-HanClauses $ln.Trim()
    if ($cl.Count -gt 0) { $pool.AddRange($cl) }
  }
  $rng = New-Object System.Random($Seed)
  for ($k = $pool.Count - 1; $k -gt 0; $k--) {
    $j = $rng.Next($k + 1); $t = $pool[$k]; $pool[$k] = $pool[$j]; $pool[$j] = $t
  }
  $take = [Math]::Min($SampleClauses, $pool.Count)
  $tmpFile = Join-Path $tmpDir ('clauses-' + $f.BaseName + '.txt')
  [System.IO.File]::WriteAllLines($tmpFile, $pool.GetRange(0, $take), (New-Object System.Text.UTF8Encoding($false)))

  $out = & $cli --eval-text $tmpFile 2>$null
  $text = ($out -join "`n")
  $mN = [regex]::Match($text, '句子\s+(\d+)\s*条')
  $m1 = [regex]::Match($text, '首选\s+([\d.]+)%')
  if (-not $m1.Success) {
    Assert-Gate ('真实语料 ' + $f.BaseName) $false 'cli 输出解析失败' '见阈值表'
    continue
  }
  $pct = [double]$m1.Groups[1].Value
  $nLabel = if ($mN.Success) { $mN.Groups[1].Value } else { '?' }
  if ($i -lt 5) {
    $b = $modernBaseline[$i]
    $pp = $b.p / 100.0
    $se = [Math]::Sqrt($pp * (1.0 - $pp) / $b.n) * 100.0
    $limit = [Math]::Round($b.p - 2.0 * $se, 2)
    Assert-Gate ('真实语料 ' + $f.BaseName) ($pct -ge $limit) ("{0}% ({1} 句)" -f $pct, $nLabel) ('>= {0}% (基准 {1}% − 2×SE {2:F2}pp)' -f $limit, $b.p, (2.0 * $se))
  } else {
    Write-Host ("[INFO] 真实语料 {0} [古典文学, 不设门禁]`n      实际值: {1}% ({2} 句)`n      阈值:   仅参考（全量基线 16.3%）" -f $f.BaseName, $pct, $nLabel)
  }
}

# ---------- 总判定 ----------
Write-Host "`n=========================================="
if ($script:Failed) {
  Write-Host 'RELEASE GATE: FAIL'
  exit 1
} else {
  Write-Host 'RELEASE GATE: PASS'
  exit 0
}
