<#
.SYNOPSIS
  Phase 3（Provider）在 Windows 上的真机验证：全程在 %TEMP% 沙箱里跑，不碰真实 ~/.claude 与 ~/.codex。

.DESCRIPTION
  覆盖 docs/windows.md 第 7.3 节：
    1) 空表可读、登记（env: 引用，不落明文）
    2) Claude 链路：CRLF/hooks 保留、反斜杠路径、回滚字节一致
    3) Codex 链路：只写 env_key、注释与其它 provider 段保留、幂等
    4) 凭据管理器：Claude 可写，Codex + keychain 必须被拒绝
    5) 中文 + 空格路径：写盘与目录联接都正常

  判定一律用证据（sha256 / 正则 / ReparsePoint），不写「看起来对」。
  真实配置文件只做只读 hash 复核，前后一致才算「零改动」。

.NOTES
  退出码：0 全部 PASS；1 有 FAIL（最后会列出失败项）。
  依赖：先 `cargo build --workspace`，本脚本用 target\debug\agenthub.exe。
#>
[CmdletBinding()]
param(
  [string]$Sandbox = (Join-Path $env:TEMP 'ah-win-verify'),
  [string]$ChineseSandbox = (Join-Path $env:TEMP 'ah-win-中文 目录')
)

$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $repo 'target\debug\agenthub.exe'
if (-not (Test-Path $cli)) { throw "缺少 $cli，先跑 cargo build --workspace" }

$script:failures = @()
$script:passes = 0

function Step([string]$title) {
  Write-Host ""
  Write-Host "== $title ==" -ForegroundColor Cyan
}

function Check([string]$name, $ok, [string]$evidence = '') {
  $suffix = if ($evidence) { " · $evidence" } else { '' }
  if ($ok) {
    $script:passes++
    Write-Host ("  [PASS] $name$suffix") -ForegroundColor Green
  } else {
    $script:failures += $name
    Write-Host ("  [FAIL] $name$suffix") -ForegroundColor Red
  }
}

function Invoke-Cli([string[]]$CliArgs) {
  $out = & $cli @CliArgs 2>&1 | Out-String
  [pscustomobject]@{ Text = ($out -replace "`r`n", "`n").Trim(); Code = $LASTEXITCODE }
}

function New-Sandbox([string]$root) {
  if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
  $homeDir = Join-Path $root 'home'
  $dataDir = Join-Path $root 'data'
  New-Item -ItemType Directory -Force -Path (Join-Path $homeDir '.claude'), (Join-Path $homeDir '.codex'), $dataDir | Out-Null
  [pscustomobject]@{ Root = $root; Home = $homeDir; Data = $dataDir }
}

function Use-Sandbox($sb) {
  $env:AGENTHUB_HOME = $sb.Home
  $env:AGENTHUB_DATA_DIR = $sb.Data
}

function Write-TextCrlf([string]$path, [string]$text) {
  [IO.File]::WriteAllText($path, $text, (New-Object System.Text.UTF8Encoding($false)))
}

function File-Hash([string]$path) {
  if (-not (Test-Path -LiteralPath $path)) { return '<不存在>' }
  (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}

function Read-Text([string]$path) { [IO.File]::ReadAllText($path) }

function Test-Crlf([string]$text) {
  $text.Contains("`r`n") -and -not [regex]::IsMatch($text, "(?<!`r)`n")
}

function Test-LoneLf([string]$text) { [regex]::IsMatch($text, "(?<!`r)`n") }

# ---------------------------------------------------------------- 0. 准备
Step '0 · 沙箱与真实文件基线'

$sb = New-Sandbox $Sandbox
Use-Sandbox $sb
Write-Host "  沙箱：$($sb.Root)"

$claudeCfg = Join-Path $sb.Home '.claude\settings.json'
$codexCfg = Join-Path $sb.Home '.codex\config.toml'

# CRLF + 无 BOM，才测的是「EOL 保真」而不是「BOM 剥离」
Write-TextCrlf $claudeCfg "{`r`n  `"hooks`": {}`r`n}`r`n"
Write-TextCrlf $codexCfg "# 我的 codex 配置`r`nmodel = `"gpt-6.1`"`r`n`r`n[model_providers.official]`r`nname = `"official`"`r`nbase_url = `"https://api.openai.com/v1`"`r`n"

$claudeBase = File-Hash $claudeCfg
$codexBase = File-Hash $codexCfg
Write-Host "  claude 基线 sha256：$claudeBase"
Write-Host "  codex  基线 sha256：$codexBase"

$realClaude = Join-Path $env:USERPROFILE '.claude\settings.json'
$realCodex = Join-Path $env:USERPROFILE '.codex\config.toml'
$realClaudeBase = File-Hash $realClaude
$realCodexBase = File-Hash $realCodex
Write-Host "  真实 claude：$realClaudeBase"
Write-Host "  真实 codex ：$realCodexBase"

# ---------------------------------------------------------------- 1. 空表
Step '1 · provider list（空表）'

$r = Invoke-Cli @('provider', 'list')
Check '空表能读' ($r.Code -eq 0 -and $r.Text -match '还没有登记 provider') $r.Text.Split("`n")[0]
Check 'Claude Code 未绑定' ($r.Text -match 'Claude Code\s+未绑定')
Check 'Codex 未绑定' ($r.Text -match 'Codex\s+未绑定')
$unsupported = @('CodeBuddy', 'Hermes', 'Gemini CLI', 'OpenCode') | Where-Object { $r.Text -match "$_\s+暂不支持写入" }
Check '其余 4 个「暂不支持写入」' ($unsupported.Count -eq 4) ($unsupported -join ' / ')

# ---------------------------------------------------------------- 2. 登记
Step '2 · 登记 provider（env: 引用）'

$r = Invoke-Cli @('provider', 'add', 'relay', '--name', '中转站', '--base-url', 'https://relay.example.com/v1',
  '--key', 'env:RELAY_API_KEY', '--model', 'claude-sonnet-4-5', '--model', 'claude-opus-4-1')
Check '登记成功' ($r.Code -eq 0)
Check '密钥显示为 env: 引用' ($r.Text -match 'env:RELAY_API_KEY')

$configJson = Read-Text (Join-Path $sb.Data 'config.json')
Check 'config.json 有 providers' ($configJson -match '"providers"')
Check 'config.json 无 sk- 明文' (-not ($configJson -match 'sk-'))
$cfg = $configJson | ConvertFrom-Json
Check 'provider 字段正确' ($cfg.providers[0].id -eq 'relay' -and $cfg.providers[0].base_url -eq 'https://relay.example.com/v1' -and $cfg.providers[0].models.Count -eq 2) ($cfg.providers[0].models -join ' / ')

# ---------------------------------------------------------------- 3. Claude
Step '3 · Claude 链路（CRLF / 合并 / 反斜杠 / 回滚）'

$r = Invoke-Cli @('provider', 'use', '--agent', 'claude', '--provider', 'relay', '--model', 'claude-sonnet-4-5', '--default')
$envIssue = $r.Text -match '环境变量 RELAY_API_KEY 未设置'
Check '未设变量时明确报「环境变量未设置」' ($r.Code -ne 0 -and $envIssue) $r.Text

$env:RELAY_API_KEY = 'relay-win-token'
$planPath = Join-Path $sb.Root 'plan.json'
$r = Invoke-Cli @('provider', 'use', '--agent', 'claude', '--provider', 'relay', '--model', 'claude-sonnet-4-5', '--default', '--out', $planPath)
Check '生成计划成功' ($r.Code -eq 0 -and (Test-Path -LiteralPath $planPath))
Check '备注里是反斜杠目标路径' ($r.Text -match [regex]::Escape($claudeCfg)) $claudeCfg

$plan = Read-Text $planPath | ConvertFrom-Json
$planAt = $plan.ops[0].action.at
Check '计划标题正确' ($plan.title -eq '写入 Provider「中转站」到 Claude Code') $plan.title
Check 'plan.json 的 at 用反斜杠' ($planAt -eq $claudeCfg -and -not $planAt.Contains('/')) $planAt
Check '明文密钥触发 Warn' (($plan.risks | Where-Object { $_.level -eq 'warn' }).Count -ge 1)
Check '设为默认触发 Warn' ($r.Text -match '设为 Claude Code 的默认模型')

$r = Invoke-Cli @('apply', $planPath, '--yes')
Check 'apply 成功' ($r.Code -eq 0 -and $r.Text -match '已执行 1 个操作') $r.Text

$after = Read-Text $claudeCfg
$doc = $after | ConvertFrom-Json
Check 'hooks 保留（合并非覆盖）' ($null -ne $doc.hooks) ($after -replace "`r`n", ' ')
Check 'env.ANTHROPIC_BASE_URL' ($doc.env.ANTHROPIC_BASE_URL -eq 'https://relay.example.com/v1')
Check 'env.ANTHROPIC_AUTH_TOKEN 为真值' ($doc.env.ANTHROPIC_AUTH_TOKEN -eq 'relay-win-token')
Check 'env.ANTHROPIC_MODEL' ($doc.env.ANTHROPIC_MODEL -eq 'claude-sonnet-4-5')
Check '顶层 model（--default）' ($doc.model -eq 'claude-sonnet-4-5')
Check '仍是 CRLF' (Test-Crlf $after) "lone-LF=$(Test-LoneLf $after)"

$batchLine = (& $cli batches | Select-String -Pattern 'cs-' | Select-Object -First 1).ToString()
$batchId = ($batchLine -split '\s+')[0]
$r = Invoke-Cli @('rollback', $batchId)
Check "回滚 $batchId" ($r.Code -eq 0) $r.Text
Check '回滚后与基线字节一致' ((File-Hash $claudeCfg) -eq $claudeBase) "$((File-Hash $claudeCfg))"

# ---------------------------------------------------------------- 4. Codex
Step '4 · Codex 链路（env_key / 保留 / 幂等）'

$codexPlanPath = Join-Path $sb.Root 'codex-plan.json'
$r = Invoke-Cli @('provider', 'use', '--agent', 'codex', '--provider', 'relay', '--model', 'claude-sonnet-4-5', '--out', $codexPlanPath)
Check 'Codex 生成计划成功' ($r.Code -eq 0 -and (Test-Path -LiteralPath $codexPlanPath))
if (Test-Path -LiteralPath $codexPlanPath) {
  $codexPlan = Read-Text $codexPlanPath | ConvertFrom-Json
  Check 'Codex 计划 at 用反斜杠' ($codexPlan.ops[0].action.at -eq $codexCfg) $codexPlan.ops[0].action.at
  $r = Invoke-Cli @('apply', $codexPlanPath, '--yes')
  Check 'Codex apply 成功' ($r.Code -eq 0) $r.Text

  $tomlAfter = Read-Text $codexCfg
  Check 'model_provider = "relay"' ($tomlAfter -match 'model_provider = "relay"')
  Check '[model_providers.relay] 段存在' ($tomlAfter -match '\[model_providers\.relay\]')
  Check 'env_key = "RELAY_API_KEY"' ($tomlAfter -match 'env_key = "RELAY_API_KEY"')
  Check '第一行注释保留' ($tomlAfter.StartsWith('# 我的 codex 配置'))
  Check '[model_providers.official] 段保留' ($tomlAfter -match '\[model_providers\.official\]')
  Check '全文无 sk- 明文' (-not ($tomlAfter -match 'sk-'))
  Check '仍是 CRLF' (Test-Crlf $tomlAfter) "lone-LF=$(Test-LoneLf $tomlAfter)"

  $idemPath = Join-Path $sb.Root 'codex-idem.json'
  $r = Invoke-Cli @('provider', 'plan', '--agent', 'codex', '--out', $idemPath)
  $idem = Read-Text $idemPath | ConvertFrom-Json
  Check '幂等：再生成计划无可执行操作' ($r.Code -eq 0 -and $idem.ops.Count -eq 0) "ops=$($idem.ops.Count)"

  # 明显不同的一份内容才应该产生操作（证明幂等不是「永远空计划」）
  $r = Invoke-Cli @('provider', 'use', '--agent', 'codex', '--provider', 'relay', '--model', 'claude-opus-4-1', '--out', (Join-Path $sb.Root 'codex-2.json'))
  $second = Read-Text (Join-Path $sb.Root 'codex-2.json') | ConvertFrom-Json
  Check '换模型后计划非空（幂等非误报）' ($r.Code -eq 0 -and $second.ops.Count -eq 1) "ops=$($second.ops.Count)"
}

# ---------------------------------------------------------------- 5. 凭据管理器
Step '5 · 凭据管理器（cmdkey / CredReadW）'

# 反例：`cmdkey /add:` 建的是「域密码」凭据，Windows 不保存可读密码
cmdkey /add:agenthub-domainpw /user:demo /pass:relay-kc-token 2>&1 | Out-Null
$r = Invoke-Cli @('provider', 'add', 'dp', '--name', '域密码凭据', '--base-url', 'https://relay0.example.com/v1', '--key', 'keychain:agenthub-domainpw/demo', '--model', 'claude-sonnet-4-5')
Check '域密码凭据登记成功' ($r.Code -eq 0)
$r = Invoke-Cli @('provider', 'use', '--agent', 'claude', '--provider', 'dp', '--model', 'claude-sonnet-4-5')
Check 'cmdkey /add 的域密码凭据被明确拒绝' ($r.Code -ne 0 -and $r.Text -match '域密码') ($r.Text -split "`n")[-1]
Check '拒绝时给出 /generic 指引' ($r.Text -match 'cmdkey /generic:agenthub-domainpw /user:demo')
cmdkey /delete:agenthub-domainpw 2>&1 | Out-Null

# 正例：`cmdkey /generic:` 才是可读路径
cmdkey /generic:agenthub-relay /user:demo /pass:relay-kc-token 2>&1 | Out-Null
$r = Invoke-Cli @('provider', 'add', 'kc', '--name', '钥匙串中转', '--base-url', 'https://relay2.example.com/v1', '--key', 'keychain:agenthub-relay/demo', '--model', 'claude-sonnet-4-5')
Check 'keychain 引用登记成功' ($r.Code -eq 0 -and $r.Text -match 'keychain:agenthub-relay/demo')

$kcPlan = Join-Path $sb.Root 'kc-plan.json'
$r = Invoke-Cli @('provider', 'use', '--agent', 'claude', '--provider', 'kc', '--model', 'claude-sonnet-4-5', '--out', $kcPlan)
Check 'Claude + keychain 能生成计划' ($r.Code -eq 0 -and (Test-Path -LiteralPath $kcPlan)) ($r.Text -split "`n")[-1]
if (Test-Path -LiteralPath $kcPlan) {
  $r = Invoke-Cli @('apply', $kcPlan, '--yes')
  $docKc = (Read-Text $claudeCfg) | ConvertFrom-Json
  Check '写入的密钥来自 Windows 凭据库' ($r.Code -eq 0 -and $docKc.env.ANTHROPIC_AUTH_TOKEN -eq 'relay-kc-token') $docKc.env.ANTHROPIC_AUTH_TOKEN
}

$r = Invoke-Cli @('provider', 'use', '--agent', 'codex', '--provider', 'kc', '--model', 'claude-sonnet-4-5')
Check 'Codex + keychain 必须被拒绝' ($r.Code -ne 0 -and $r.Text -match 'Codex 只支持 env: 形式的密钥引用') $r.Text
$tomlAfter = Read-Text $codexCfg
Check '被拒绝后 config.toml 未被改动' (-not ($tomlAfter -match 'relay-kc-token')) ''
$codexBinding = ((Invoke-Cli @('provider', 'list', '--json')).Text | ConvertFrom-Json).bindings |
  Where-Object { $_.agent -eq 'codex' }
Check '被拒绝后没有留下悬空绑定' ($codexBinding.provider_id -eq 'relay') "当前绑定=$($codexBinding.provider_id)"

# 读不到凭据时给的是可操作提示，而不是裸错误
$r = Invoke-Cli @('provider', 'add', 'nokc', '--name', '不存在的钥匙串', '--base-url', 'https://relay3.example.com/v1', '--key', 'keychain:agenthub-missing/demo', '--model', 'claude-sonnet-4-5')
$r2 = Invoke-Cli @('provider', 'use', '--agent', 'claude', '--provider', 'nokc', '--model', 'claude-sonnet-4-5')
Check '凭据不存在时提示 cmdkey /generic 用法' ($r2.Code -ne 0 -and $r2.Text -match 'cmdkey /generic:agenthub-missing /user:demo') $r2.Text

cmdkey /delete:agenthub-relay 2>&1 | Out-Null

# ---------------------------------------------------------------- 6. 中文 + 空格路径
Step '6 · 中文与空格路径'

$sb2 = New-Sandbox $ChineseSandbox
Use-Sandbox $sb2
Write-Host "  沙箱：$($sb2.Root)"

$cnClaudeCfg = Join-Path $sb2.Home '.claude\settings.json'
Write-TextCrlf $cnClaudeCfg "{`r`n  `"hooks`": {}`r`n}`r`n"
$cnBase = File-Hash $cnClaudeCfg

$r = Invoke-Cli @('provider', 'add', 'relay', '--name', '中文中转', '--base-url', 'https://relay.example.com/v1', '--key', 'env:RELAY_API_KEY', '--model', 'claude-sonnet-4-5')
Check '中文沙箱登记成功' ($r.Code -eq 0)

$cnPlan = Join-Path $sb2.Root '中文 plan.json'
$r = Invoke-Cli @('provider', 'use', '--agent', 'claude', '--provider', 'relay', '--model', 'claude-sonnet-4-5', '--default', '--out', $cnPlan)
Check '中文路径下生成计划' ($r.Code -eq 0 -and (Test-Path -LiteralPath $cnPlan))
$cnPlanAt = (Read-Text $cnPlan | ConvertFrom-Json).ops[0].action.at
Check '计划路径含中文且为反斜杠' ($cnPlanAt -eq $cnClaudeCfg -and $cnPlanAt -match '中文 目录') $cnPlanAt

$r = Invoke-Cli @('apply', $cnPlan, '--yes')
$cnDoc = (Read-Text $cnClaudeCfg) | ConvertFrom-Json
Check '中文路径下写入成功' ($r.Code -eq 0 -and $cnDoc.env.ANTHROPIC_MODEL -eq 'claude-sonnet-4-5') $r.Text
Check '中文路径下仍是 CRLF' (Test-Crlf (Read-Text $cnClaudeCfg))

$cnBatch = ((& $cli batches | Select-String -Pattern 'cs-' | Select-Object -First 1).ToString() -split '\s+')[0]
$r = Invoke-Cli @('rollback', $cnBatch)
Check '中文路径下回滚字节一致' ((File-Hash $cnClaudeCfg) -eq $cnBase) "$((File-Hash $cnClaudeCfg))"

# 目录联接也必须能在中文 + 空格路径下建出来
foreach ($agent in '.claude', '.codex') {
  $dir = Join-Path $sb2.Home "$agent\skills\中文技能"
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  Set-Content -LiteralPath (Join-Path $dir 'SKILL.md') -Value "# 中文技能`n" -Encoding utf8
}
$cnDedupe = Join-Path $sb2.Root 'dedupe.json'
$r = Invoke-Cli @('plan-dedupe', '--out', $cnDedupe)
$r2 = Invoke-Cli @('apply', $cnDedupe, '--yes')
$link = Join-Path $sb2.Home '.claude\skills\中文技能'
$isJunction = $false
if (Test-Path -LiteralPath $link) {
  $isJunction = (Get-Item -LiteralPath $link -Force).Attributes.ToString().Contains('ReparsePoint')
}
Check '中文路径下建出目录联接' ($r.Code -eq 0 -and $r2.Code -eq 0 -and $isJunction) "reparse=$isJunction"
Check '中文路径下联接可读' (Test-Path -LiteralPath (Join-Path $link 'SKILL.md'))

# ---------------------------------------------------------------- 7. 收尾
Step '7 · 真实文件零改动复核'

Check '真实 ~/.claude/settings.json 未被改动' ((File-Hash $realClaude) -eq $realClaudeBase) "$((File-Hash $realClaude))"
Check '真实 ~/.codex/config.toml 未被改动' ((File-Hash $realCodex) -eq $realCodexBase) "$((File-Hash $realCodex))"

Remove-Item Env:AGENTHUB_HOME -ErrorAction SilentlyContinue
Remove-Item Env:AGENTHUB_DATA_DIR -ErrorAction SilentlyContinue
Remove-Item Env:RELAY_API_KEY -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "PASS $($script:passes) / FAIL $($script:failures.Count)"
if ($script:failures.Count -eq 0) {
  Write-Host '全部通过' -ForegroundColor Green
  exit 0
}
Write-Host ('失败项: ' + ($script:failures -join ' | ')) -ForegroundColor Red
exit 1
