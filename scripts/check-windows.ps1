<#
.SYNOPSIS
  AgentHub Windows 一键验证：fmt → clippy → test → 前端 typecheck/build → 沙箱端到端。

.DESCRIPTION
  与 scripts/check-macos.sh 覆盖同一组检查，两端结果应一致。
  换到 Windows 机器后先跑这个，再改代码。
  只读真实环境：端到端全程在 $env:TEMP 下的沙箱里跑，不碰 ~/.claude 等目录。

.NOTES
  退出码：0 全部通过；1 有步骤失败（会打印失败步骤）。
#>
[CmdletBinding()]
param(
  [switch]$SkipFrontend,
  [switch]$SkipE2E
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$failures = @()

function Write-Step($name) {
  Write-Host ""
  Write-Host "== $name ==" -ForegroundColor Cyan
}

function Invoke-Step($name, [scriptblock]$body) {
  Write-Step $name
  try {
    & $body
    if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) { throw "退出码 $LASTEXITCODE" }
    Write-Host "  PASS" -ForegroundColor Green
  } catch {
    Write-Host "  FAIL: $_" -ForegroundColor Red
    $script:failures += $name
  }
}

# rustup 装好的 cargo 未必在当前会话 PATH 里，优先用绝对路径
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }
$npm = (Get-Command npm -ErrorAction SilentlyContinue).Source
if (-not $npm) { throw '找不到 npm，请先安装 Node.js 20+' }

Push-Location $repo
try {
  Invoke-Step 'cargo fmt --check' { & $cargo fmt --all --check }
  Invoke-Step 'cargo clippy' { & $cargo clippy --workspace --all-targets -- -D warnings }
  Invoke-Step 'cargo test' { & $cargo test -p agenthub-core }
  # 网关有自己的集成测试（本地 fake upstream，验证透传 / SSE / 重试 / 鉴权）
  Invoke-Step 'cargo test gateway' { & $cargo test -p agenthub-gateway }
  # 桌面壳：真起 PTY 跑默认 shell（TERM / COLORTERM / 初始尺寸）+ 关闭标签后进程回收 + 字体排序
  # 非 Windows 的用例在 cfg 里跳过，两端跑同一条命令
  Invoke-Step 'cargo test desktop' { & $cargo test -p agenthub-desktop }
  Invoke-Step 'cargo build --workspace' { & $cargo build --workspace }

  if (-not $SkipFrontend) {
    Push-Location (Join-Path $repo 'apps\desktop')
    try {
      Invoke-Step 'npm run typecheck' { & $npm run typecheck }
      Invoke-Step 'npm run build' { & $npm run build }
    } finally { Pop-Location }
  }

  # 本地化门槛：词典三语齐平 + 后端无残留（前端有意保留项只报告）
  Invoke-Step 'i18n 校验' { & node (Join-Path $repo 'scripts\i18n-check.mjs') --strict }

  if (-not $SkipE2E) {
    $sandbox = Join-Path $env:TEMP 'agenthub-e2e'
    if (Test-Path $sandbox) { Remove-Item -Recurse -Force $sandbox }
    $homeDir = Join-Path $sandbox 'home'
    $dataDir = Join-Path $sandbox 'data'
    foreach ($agent in '.claude', '.codex', '.hermes') {
      $dir = Join-Path $homeDir "$agent\skills\cloudflare"
      New-Item -ItemType Directory -Force -Path $dir | Out-Null
      Set-Content -Path (Join-Path $dir 'SKILL.md') -Value "# cloudflare`n" -Encoding utf8
      Set-Content -Path (Join-Path $dir 'notes.md') -Value 'same' -Encoding utf8
    }
    New-Item -ItemType Directory -Force -Path $dataDir | Out-Null

    $cli = Join-Path $repo 'target\debug\agenthub.exe'
    $plan = Join-Path $sandbox 'plan.json'
    $env:AGENTHUB_HOME = $homeDir
    $env:AGENTHUB_DATA_DIR = $dataDir

    Invoke-Step '端到端：scan' { & $cli scan | Out-Host }
    Invoke-Step '端到端：plan-dedupe' { & $cli plan-dedupe --out $plan | Out-Host }
    Invoke-Step '端到端：apply' { & $cli apply $plan --yes | Out-Host }

    Invoke-Step '端到端：链接与真身校验' {
      $storeSkill = Join-Path $dataDir 'store\cloudflare\SKILL.md'
      if (-not (Test-Path $storeSkill)) { throw '中央仓库里的真身不见了' }
      foreach ($agent in '.claude', '.codex', '.hermes') {
        $link = Join-Path $homeDir "$agent\skills\cloudflare"
        $item = Get-Item $link -Force
        if (-not $item.Attributes.ToString().Contains('ReparsePoint')) {
          throw "$agent 下的技能不是链接（Windows 上应为目录联接）"
        }
        if (-not (Test-Path (Join-Path $link 'SKILL.md'))) { throw "$link 读不到内容" }
      }
      $doctor = & $cli doctor
      $doctor | Out-Host
      if ($doctor -match '失效|破损') { throw 'doctor 报了链接失效，链接判定可能有问题' }
    }

    $batchLine = (& $cli batches | Select-String -Pattern 'cs-' | Select-Object -First 1).ToString()
    $batchId = ($batchLine -split '\s+')[0]
    Invoke-Step '端到端：rollback' { & $cli rollback $batchId | Out-Host }

    Invoke-Step '端到端：回滚校验' {
      foreach ($agent in '.claude', '.codex', '.hermes') {
        $dir = Join-Path $homeDir "$agent\skills\cloudflare"
        $item = Get-Item $dir -Force
        if ($item.Attributes.ToString().Contains('ReparsePoint')) { throw "$dir 回滚后仍是链接" }
        if (-not (Test-Path (Join-Path $dir 'SKILL.md'))) { throw "$dir 回滚后内容缺失" }
      }
      if (Test-Path (Join-Path $dataDir 'store\cloudflare')) { throw '回滚后中央仓库应清空' }
    }

    # 项目级 MCP：计划与 apply 必须用同一作用域，否则项目内路径会被写入门禁拒绝
    $projRoot = Join-Path $sandbox 'proj'
    New-Item -ItemType Directory -Force -Path $projRoot | Out-Null
    $mcpJson = Join-Path $sandbox 'mcp.json'
    Set-Content -Path $mcpJson -Value '{"mcpServers":{"demo":{"command":"npx","args":["-y","demo-mcp"]}}}' -Encoding utf8

    Invoke-Step '项目作用域：project init' { & $cli project init $projRoot | Out-Host }
    Invoke-Step '项目作用域：mcp import --project' {
      & $cli mcp import $mcpJson --agents claude,codex --project $projRoot --yes | Out-Host
    }
    Invoke-Step '项目作用域：落盘位置校验' {
      $claudeMcp = Join-Path $projRoot '.mcp.json'
      $codexMcp = Join-Path $projRoot '.codex\config.toml'
      if (-not (Test-Path $claudeMcp)) { throw "项目内缺少 $claudeMcp" }
      if (-not (Test-Path $codexMcp)) { throw "项目内缺少 $codexMcp" }
      if ((Get-Content $claudeMcp -Raw) -notmatch 'demo') { throw '.mcp.json 里没有 demo 条目' }
      if (Test-Path (Join-Path $homeDir '.claude.json')) {
        throw '项目作用域不应写全局 ~/.claude.json'
      }
    }
  }
} finally {
  Pop-Location
  Remove-Item Env:AGENTHUB_HOME -ErrorAction SilentlyContinue
  Remove-Item Env:AGENTHUB_DATA_DIR -ErrorAction SilentlyContinue
}

# UI 断言：需要一个跑着的演示站点（npm --prefix apps\desktop run dev，5273）与一个 CDP 端点。
# 默认跳过（CI 上没有浏览器）；本机跑：$env:UI=1; .\scripts\check-windows.ps1
if ($env:UI -eq '1') {
  Invoke-Step 'UI 断言（ui-smoke）' { & node (Join-Path $repo 'scripts\ui-smoke.mjs') }
} else {
  Write-Step 'UI 断言（ui-smoke）'
  Write-Host '  跳过：设 $env:UI=1 并先起 npm --prefix apps\desktop run dev（CDP 端点见 --cdp）' -ForegroundColor DarkGray
}

Write-Host ""
if ($failures.Count -eq 0) {
  Write-Host "全部通过" -ForegroundColor Green
  exit 0
}
Write-Host ("失败的步骤: " + ($failures -join ', ')) -ForegroundColor Red
exit 1
