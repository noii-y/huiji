$env:Path = "C:\Users\28030\.cargo\bin;" + $env:Path
# Relocate via $PSScriptRoot so the script contains no non-ASCII literals (avoids GBK mojibake).
$root = $PSScriptRoot
$appRoot = Join-Path $root "..\..\.."
$corpus = Join-Path $root "corpus"
$out = Join-Path $root "results"
Set-Location $appRoot
New-Item -ItemType Directory -Force -Path $out | Out-Null

$summary = Join-Path $out "baseline_summary.txt"
if (Test-Path $summary) { Remove-Item $summary }

Get-ChildItem (Join-Path $corpus "s*.txt") | Sort-Object Name | ForEach-Object {
    $name = $_.BaseName
    $det = Join-Path $out ($name + ".jsonl")
    $tmp = Join-Path $out ($name + ".report.txt")
    if (Test-Path $det) { Remove-Item $det }
    $t0 = Get-Date
    & .\target\release\qingjian-cli.exe --eval-text $_.FullName --misses 0 --eval-details $det 2>$null > $tmp
    $sec = [int]((Get-Date) - $t0).TotalSeconds
    $head = (Get-Content $tmp -Encoding UTF8 -TotalCount 3 | Out-String)
    Add-Content -Path $summary -Value ("===== " + $name + " (" + $sec + "s) =====") -Encoding UTF8
    Add-Content -Path $summary -Value $head -Encoding UTF8
    Write-Output ("done " + $name + " " + $sec + "s")
}
Write-Output "ALL DONE"
