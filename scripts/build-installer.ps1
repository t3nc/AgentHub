<#
.SYNOPSIS
  构建 AgentHub 桌面端安装包（Windows → NSIS，macOS 请用 scripts/build-installer 的等价命令）。

.DESCRIPTION
  1) 先构建前端产物（tauri 的 beforeBuildCommand 也会做，这里显式跑一次便于定位前端错误）；
  2) 用 release profile（opt-level=s + lto）编译壳；
  3) 只打 NSIS，不碰 MSI/WiX（后者首次会额外下载 WiX 工具链）。
  产物：apps\desktop\src-tauri\target\release\bundle\nsis\*.exe

.NOTES
  首次执行需要联网下载 NSIS 打包器；失败时脚本会明确报错，不会静默产出「假成功」。
#>
[CmdletBinding()]
param(
  [switch]$SkipFrontend
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$tauriRoot = Join-Path $repo 'apps\desktop\src-tauri'

# rustup 装的 cargo 未必在 PATH 里（Tauri CLI 内部会调用 `cargo metadata`）
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $cargoBin) {
  $env:Path = "$cargoBin;$env:Path"
}

$cargo = Join-Path $cargoBin 'cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }
Write-Host "cargo: $cargo" -ForegroundColor Cyan
& $cargo --version

Push-Location (Join-Path $repo 'apps\desktop')
try {
  if (-not $SkipFrontend) {
    Write-Host "`n== npm run build ==" -ForegroundColor Cyan
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "前端构建失败（退出码 $LASTEXITCODE）" }
  }

  Write-Host "`n== tauri build --bundles nsis ==" -ForegroundColor Cyan
  npm run tauri build -- --bundles nsis
  if ($LASTEXITCODE -ne 0) { throw "打包失败（退出码 $LASTEXITCODE）" }
} finally {
  Pop-Location
}

# cargo workspace 共享根目录 target/，所以产物在仓库根而不是 crate 目录下；
# 两个位置都查，避免以后改了 workspace 布局脚本就失效。
$nsisDir = @(
  (Join-Path $repo 'target\release\bundle\nsis'),
  (Join-Path $tauriRoot 'target\release\bundle\nsis')
) | Where-Object { Test-Path $_ } | Select-Object -First 1

Write-Host ""
if (-not $nsisDir) {
  throw "没有找到 bundle\nsis 产物目录 —— 打包器可能未下载成功，检查上面的日志"
}
$installers = Get-ChildItem $nsisDir -Filter '*.exe' -ErrorAction SilentlyContinue
if (-not $installers) {
  throw "目录 $nsisDir 里没有 .exe 安装包"
}
foreach ($item in $installers) {
  $mb = [math]::Round($item.Length / 1MB, 1)
  Write-Host ("产物: {0}  ({1} MB)" -f $item.FullName, $mb) -ForegroundColor Green
}
