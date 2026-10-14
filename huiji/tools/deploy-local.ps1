# 把刚编译好的 release server 部署到本机安装目录，用于开发期快速真机验证，不必重打安装包。
# 用法：在 PowerShell 里运行本脚本（会自动请求管理员权限），或在文件资源管理器里右键“使用 PowerShell 运行”。
# 可选参数：-InstallDir "D:\Huiji" 覆盖安装目录。
param(
    [string]$InstallDir = "C:\Program Files\Huiji",
    [string]$ExeName = "qingjian-server.exe"
)

$ErrorActionPreference = "Stop"

# 非管理员则以管理员身份重启自己（会弹一次 UAC，点“是”）。
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "需要管理员权限写入安装目录，正在请求授权（UAC 点“是”）..."
    $args = @(
        "-NoProfile", "-ExecutionPolicy", "Bypass",
        "-File", "`"$PSCommandPath`"",
        "-InstallDir", "`"$InstallDir`""
    )
    Start-Process powershell -Verb RunAs -ArgumentList $args
    exit
}

# 脚本在 app/huiji/tools/ 下，项目根是上两级。
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$src = Join-Path $root "target\release\$ExeName"
$dst = Join-Path $InstallDir $ExeName
if (-not (Test-Path $src)) {
    throw "找不到 $src，请先执行 cargo build --release -p qingjian-windows-server"
}
if (-not (Test-Path $InstallDir)) {
    throw "安装目录不存在：$InstallDir（用 -InstallDir 指定）"
}

Write-Host "停止运行中的 server..."
Get-Process -Name "qingjian-server" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 700

Write-Host "复制 $src -> $dst"
Copy-Item -LiteralPath $src -Destination $dst -Force

Write-Host ("部署完成，文件时间：" + (Get-Item $dst).LastWriteTime)
Write-Host "重新打开任意应用并打字即可拉起新 server；若候选窗没更新，重新登录一次。"
Start-Sleep -Seconds 2
