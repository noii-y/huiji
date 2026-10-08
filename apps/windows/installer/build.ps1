<#
.SYNOPSIS
    在 Windows 上打灰迹安装包：release 构建三个产物 + 用 Inno Setup 编 qingjian.iss。
.DESCRIPTION
    在编译机（MSVC 工具链 + Inno Setup）上跑。步骤：
      1) cargo build --release 出 DLL / Server / 设置程序，再单独编一份 32 位 DLL；
      2) 从 apps\windows\server\Cargo.toml 读版本号（-dev 版接 git 短哈希）；
      3) 找 ISCC.exe（PATH 或常见安装位置）；
      4) iscc /DAppVersion=<版本> 编脚本，成品在 target\installer\huiji-<版本>-windows-x86_64-setup.exe。
    随包数据（.qj / .tsv）直接由 .iss 从仓库 data\generated 与 assets 里取，不另建暂存目录；
    确保打包前 data\generated 里的 .qj 是最新的（bundle 流程见仓库 CLAUDE.md）。
    uiAccess 跟着 -Sign 走，不用手设 QINGJIAN_UIACCESS（见 -Sign）。
.PARAMETER SkipBuild
    跳过 cargo build（数据或 .iss 改了、二进制没变时重编安装包用）。
.PARAMETER Sign
    自签产物（sign-local.ps1）并开 uiAccess（候选窗才能盖过商店 / 任务栏搜索）。uiAccess=true 的 exe 要本机受信任的签名
    才准启动，自签证书只有编译机信任——所以 -Sign 只用于本机真机测，对外分发的包不加此开关：不签名、关 uiAccess，
    候选窗在那几个系统界面里会被盖住，但任何机器都能起。
#>
[CmdletBinding()]
param([switch]$SkipBuild, [switch]$Sign)

$ErrorActionPreference = 'Continue'  # cargo 等原生命令的 warning 走 stderr，Stop 会把它误判成致命错误；真实失败靠下面每步的显式判断与 throw 兜底

# 仓库根：本脚本在 apps\windows\installer 下，往上三层是 ime\。
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$Iss  = Join-Path $PSScriptRoot 'qingjian.iss'

# uiAccess 跟着 -Sign 走（server\build.rs 读这个变量，改了会自动重编 Server）；理由见 -Sign 的说明。
$env:QINGJIAN_UIACCESS = if ($Sign) { '1' } else { '0' }
if ($Sign) {
    Write-Host 'uiAccess=1（-Sign：仅本机真机测，别用于对外分发）' -ForegroundColor Yellow
} else {
    Write-Host 'uiAccess=0（对外分发：Server 任何机器都能起；候选窗在商店 / 任务栏搜索里可能被盖）' -ForegroundColor Cyan
}

# ct2rs 的 system 后端在 Windows 上会让链接器去找 cublas / cublasLt（x86_64 还要 cpu_features）几个静态库，
# 即便最终只走 CPU、没有符号真的引用它们；缺了报 LNK1104「打不开文件」。缺库时用 MSVC 编一个不引头的空目标，
# 各自打成静态库占位：链接器打开后没有要解析的符号就跳过，CPU 推理不碰 CUDA。
function Ensure-CudaStubLibs {
    param([string]$LibDir)
    $stubNames = @('cublas.lib', 'cublasLt.lib', 'cpu_features.lib')
    $missing = @($stubNames | Where-Object { -not (Test-Path (Join-Path $LibDir $_)) })
    if ($missing.Count -eq 0) { return }
    Write-Host "缺链接占位库 $($missing -join ', ')，自动生成…" -ForegroundColor Cyan

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { throw "缺占位库且找不到 vswhere（$vswhere）：装 VS 的 C++ 工具链，或手工放 $($missing -join ', ')" }
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsPath) { throw 'vswhere 没找到带 C++ x64 工具的 Visual Studio' }
    $toolVer = Get-ChildItem (Join-Path $vsPath 'VC\Tools\MSVC') -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $hostBin = Join-Path $toolVer.FullName 'bin\Hostx64\x64'
    $cl = Join-Path $hostBin 'cl.exe'
    $lib = Join-Path $hostBin 'lib.exe'
    if (-not (Test-Path $cl) -or -not (Test-Path $lib)) { throw "在 $hostBin 找不到 cl.exe / lib.exe" }

    New-Item -ItemType Directory -Path $LibDir -Force | Out-Null
    $tmp = Join-Path $env:TEMP ("huiji-stub-" + [IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tmp -Force | Out-Null
    $cpp = Join-Path $tmp 'stub.cpp'
    # 不 include 任何头，编译就不需要 INCLUDE 环境；符号没人引用，只为让 lib.exe 有东西可打包。
    [IO.File]::WriteAllText($cpp, 'int huiji_cuda_link_stub() { return 0; }')
    $obj = Join-Path $tmp 'stub.obj'
    $prevPath = $env:Path
    $env:Path = "$hostBin;$env:Path"
    try {
        & $cl /nologo /c /MT $cpp "/Fo$obj" | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'cl.exe 编译占位目标失败' }
        foreach ($name in $missing) {
            $out = Join-Path $LibDir $name
            & $lib /nologo "/OUT:$out" $obj | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "lib.exe 生成占位库 $name 失败" }
        }
    } finally {
        $env:Path = $prevPath
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
    Write-Host "占位库已生成 → $LibDir" -ForegroundColor Green
}

# 1) 构建三个产物。
if (-not $SkipBuild) {
    Write-Host '构建 release 产物…' -ForegroundColor Cyan
    Push-Location $Repo
    try {
        cargo build --release --locked -p qingjian-windows-tsf -p qingjian-windows-settings
        if ($LASTEXITCODE -ne 0) { throw "cargo build 失败（退出码 $LASTEXITCODE）" }
        # Server 带端侧整句翻译：local-nmt-system 链接本机预编译 CT2 dll，LIBRARY_PATH 指向 .lib 目录。
        $sdkLib = Join-Path $env:LOCALAPPDATA 'HuijiSDK\ct2\lib'
        if (-not (Test-Path (Join-Path $sdkLib 'ctranslate2.lib'))) { throw "端侧翻译 SDK 不在：$sdkLib" }
        Ensure-CudaStubLibs -LibDir $sdkLib
        $env:LIBRARY_PATH = $sdkLib
        cargo build --release --locked -p qingjian-windows-server --features local-nmt-system
        if ($LASTEXITCODE -ne 0) { throw "Server cargo build 失败（退出码 $LASTEXITCODE）" }
        cargo build --release --locked -p qingjian-windows-tsf --target i686-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) { throw "32 位 DLL cargo build 失败（退出码 $LASTEXITCODE）" }
    } finally { Pop-Location }
}

# 缺一个产物就早报错。
$targets = @(
    'release\qingjian_tsf.dll',
    'i686-pc-windows-msvc\release\qingjian_tsf.dll',
    'release\qingjian-server.exe',
    'release\qingjian-settings.exe'
)
foreach ($t in $targets) {
    $p = Join-Path $Repo "target\$t"
    if (-not (Test-Path $p)) { throw "缺产物 $p，先跑一次不带 -SkipBuild 的构建" }
}

# 1.2) 自包含 Windows App Runtime：设置程序不再依赖机器上装的框架包（Windows 10 上框架依赖的引导用不了，
#      见 apps\windows\settings\build.rs）。cargo 构建时 windows-reactor-setup 已按清单把运行时铺到
#      target\release\，这里挑进暂存目录；target\release 里还有 deps\ 之类的中间产物，不能整个目录装。
$runtimeStage = Join-Path $Repo 'target\installer\settings-runtime'
$runtimeList  = Join-Path $PSScriptRoot 'settings-runtime.txt'
# 清单是 UTF-8 且带中文注释：不指定编码时 PowerShell 5.1 按 GBK 读，注释末尾的字节会吞掉换行，紧跟其后的一项被当成注释漏掉。
$wanted = Get-Content $runtimeList -Encoding UTF8 | Where-Object { $_ -and -not $_.StartsWith('#') } | ForEach-Object { $_.Trim() }
if (Test-Path $runtimeStage) { Remove-Item $runtimeStage -Recurse -Force }
New-Item -ItemType Directory -Path $runtimeStage -Force | Out-Null
$missing = @()
foreach ($name in $wanted) {
    $src = Join-Path $Repo "target\release\$name"
    if (Test-Path $src) {
        Copy-Item $src -Destination (Join-Path $runtimeStage $name) -Recurse -Force
    } else {
        $missing += $name
    }
}
# 缺文件说明自包含运行时没铺成功（build.rs 下载 NuGet 或解 MSIX 失败），早报错，别打出个跑不起来的包。
if ($missing.Count -gt 0) { throw "自包含 Windows App Runtime 缺 $($missing.Count) 项：$($missing -join ', ')" }
Write-Host "自包含运行时 $($wanted.Count) 项 → target\installer\settings-runtime" -ForegroundColor Cyan

# 1.5) 签名（必须在 iscc 打包前：Inno 把已签的文件原样拷进安装包）。
if ($Sign) {
    Write-Host '自签产物（uiAccess 要求 Server 代码签名）…' -ForegroundColor Cyan
    $binaries = $targets | ForEach-Object { Join-Path $Repo "target\$_" }
    & (Join-Path $PSScriptRoot 'sign-local.ps1') -Path $binaries
}

# 2) 从 server 的 Cargo.toml 读版本（apps\* 各自写死版本，不跟 workspace）。
$cargoToml = Get-Content (Join-Path $Repo 'apps\windows\server\Cargo.toml')
$verLine = $cargoToml | Where-Object { $_ -match '^\s*version\s*=\s*"(.+)"' } | Select-Object -First 1
if (-not ($verLine -match '"(.+)"')) { throw '在 server\Cargo.toml 里没找到 version' }
$Version = $Matches[1]
# 开发版接 git 短哈希（0.1.3-dev-1a2b3c4，脏加 +），有 bug 能定位到哪次改动；发版提交去掉 -dev 就不接。
if ($Version.EndsWith('-dev')) {
    Push-Location $Repo
    try {
        $rev = (git rev-parse --short HEAD 2>$null)
        if ($LASTEXITCODE -eq 0 -and $rev) {
            if (git status --porcelain 2>$null) { $rev = "$rev+" }
            $Version = "$Version-$rev"
        }
    } finally { Pop-Location }
}
# Inno 的 VersionInfoVersion 只认数字：去掉 -alpha.1 这类预发布后缀。
$VersionNumeric = $Version -replace '-.*$', ''
Write-Host "版本 $Version" -ForegroundColor Cyan

# 3) 找 ISCC.exe：先 Program Files 里的 7（与开发机同版本；CI 镜像 PATH 上自带 Chocolatey 的 6，不带简中翻译，不能让它抢先），
#    再 PATH，最后 6。QINGJIAN_ISCC 环境变量可直接指定。
$iscc = $env:QINGJIAN_ISCC
if (-not $iscc) {
    $candidates = @(
        "${env:ProgramFiles}\Inno Setup 7\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe"
    )
    $iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { $iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source }
if (-not $iscc) {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe"
    )
    $iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw '找不到 ISCC.exe：装 Inno Setup 7 或用 QINGJIAN_ISCC 指定' }
Write-Host "用 $iscc" -ForegroundColor Cyan

# 4) 编安装包。
& $iscc "/DAppVersion=$Version" "/DAppVersionNumeric=$VersionNumeric" $Iss
if ($LASTEXITCODE -ne 0) { throw "iscc 失败（退出码 $LASTEXITCODE）" }

$out = Join-Path $Repo "target\installer\huiji-$Version-windows-x86_64-setup.exe"
Write-Host "完成：$out" -ForegroundColor Green
