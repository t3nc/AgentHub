<#

.SYNOPSIS
  Phase 4 token usage verification: sandbox fixture end-to-end + real log read-only cross-check.

.DESCRIPTION
  Expected values are computed by THIS SCRIPT while writing the fixtures, not read back from
  the parser. A wrong parser therefore fails instead of agreeing with itself.

  ASCII only on purpose: Windows PowerShell 5.1 reads BOM-less files as ANSI and would
  mangle non-ASCII literals (including the quotes around them).

.NOTES
  Exit code: 0 all checks passed; 1 at least one assertion failed.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$script:pass = 0
$script:fail = 0

function Check($name, $actual, $expected) {
  if ("$actual" -eq "$expected") {
    Write-Host ("  PASS  {0} = {1}" -f $name, $actual) -ForegroundColor Green
    $script:pass++
  } else {
    Write-Host ("  FAIL  {0}: expected {1}, got {2}" -f $name, $expected, $actual) -ForegroundColor Red
    $script:fail++
  }
}

$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

$sandbox = Join-Path $env:TEMP ("ah-usage-verify-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
$homeDir = Join-Path $sandbox 'home'
$dataDir = Join-Path $sandbox 'data'
New-Item -ItemType Directory -Force -Path $homeDir, $dataDir | Out-Null

Write-Host ""
Write-Host "== fixtures (expected values derived here) ==" -ForegroundColor Cyan

# Claude: 100 input / 10 output / 20 cache read / 7 cache write.
# The same message.id appears twice and must be counted once.
$claudeDir = Join-Path $homeDir '.claude\projects\p1'
New-Item -ItemType Directory -Force -Path $claudeDir | Out-Null
$claudeLine = '{"type":"assistant","timestamp":"2026-10-31T06:12:00.000Z","sessionId":"s1","message":{"id":"m1","role":"assistant","model":"glm-5","usage":{"input_tokens":100,"output_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":7}}}'
Set-Content -Path (Join-Path $claudeDir 'a.jsonl') -Value @($claudeLine, $claudeLine) -Encoding utf8

# Codex: cumulative 100/10 then 300/30 with cached 100. Delta = input 200 (-100 cached), output 20.
$codexDir = Join-Path $homeDir '.codex\sessions\2026\10\31'
New-Item -ItemType Directory -Force -Path $codexDir | Out-Null
$codexLines = @(
  '{"timestamp":"2026-10-31T06:00:00.000Z","type":"session_meta","payload":{"session_id":"c1","model_provider":"relay"}}',
  '{"timestamp":"2026-10-31T06:00:01.000Z","type":"turn_context","payload":{"model":"gpt-6.1-sol"}}',
  '{"timestamp":"2026-10-31T06:00:02.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":10}}}}',
  '{"timestamp":"2026-10-31T06:05:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":300,"cached_input_tokens":100,"cache_write_input_tokens":0,"output_tokens":30}}}}'
)
Set-Content -Path (Join-Path $codexDir 'rollout.jsonl') -Value $codexLines -Encoding utf8

# CodeBuddy CLI (OpenAI style): input 1000 already includes 800 cached -> uncached 200.
$cbDir = Join-Path $homeDir '.codebuddy\projects\p1'
New-Item -ItemType Directory -Force -Path $cbDir | Out-Null
$cbLine = '{"timestamp":1792522730561,"providerData":{"model":"deepseek-v4.1-flash","usage":{"inputTokens":1000,"outputTokens":50,"inputTokensDetails":[{"cached_tokens":800}]}}}'
Set-Content -Path (Join-Path $cbDir 'b.jsonl') -Value @($cbLine) -Encoding utf8

# CodeBuddy IDE: detail file lives at history/<conv>/<req>/index.json; the model comes from the
# conversation listing one level up. Redirected under the sandbox home (see paths::home_is_overridden).
$ideConv = Join-Path $homeDir 'CodeBuddyExtension\Data\u\CodeBuddyIDE\u\history\conv-1'
$ideReq = Join-Path $ideConv 'req-1'
New-Item -ItemType Directory -Force -Path $ideReq | Out-Null
Set-Content -Path (Join-Path $ideConv 'index.json') -Value '{"conversations":[{"id":"conv-1","selectedModelId":"agnes-3.0-flash"}]}' -Encoding utf8
Set-Content -Path (Join-Path $ideReq 'index.json') -Value '{"requests":[{"id":"r1","startedAt":1792226349467,"usage":{"inputTokens":500,"outputTokens":20,"cacheTokens":400,"cachedWriteTokens":0}}]}' -Encoding utf8

# WorkBuddy: the same line is replayed into a copied session; cross-file fingerprint counts it once.
$wbDir = Join-Path $homeDir '.workbuddy\projects\p1'
$wbDir2 = Join-Path $homeDir '.workbuddy\projects\p2'
New-Item -ItemType Directory -Force -Path $wbDir, $wbDir2 | Out-Null
$wbLine = '{"timestamp":1781970085673,"providerData":{"model":"glm-5-3-flash","usage":{"inputTokens":600,"outputTokens":30}}}'
Set-Content -Path (Join-Path $wbDir 'c.jsonl') -Value @($wbLine) -Encoding utf8
Set-Content -Path (Join-Path $wbDir2 'd.jsonl') -Value @($wbLine) -Encoding utf8

Write-Host ""
Write-Host "== agenthub usage in sandbox ==" -ForegroundColor Cyan

function Run-Usage {
  $env:AGENTHUB_HOME = $homeDir
  $env:AGENTHUB_DATA_DIR = $dataDir
  $out = & $cargo run -q -p agenthub-cli -- usage --range all --json 2>&1
  if ($LASTEXITCODE -ne 0) { throw ("agenthub usage failed: " + ($out -join "`n")) }
  return ($out -join "`n") | ConvertFrom-Json
}

$first = Run-Usage

Write-Host ""
Write-Host "== aggregate semantics ==" -ForegroundColor Cyan
# Claude 100 + Codex 200 + CodeBuddy 200 + WorkBuddy 600 + IDE 100 = 1200
# cache read 20 + 100 + 800 + 0 + 400 = 1320 / cache write 7 / output 10+30+50+30+20 = 140
Check 'uncached input' $first.totals.input 1200
Check 'cache read' $first.totals.cache_read 1320
Check 'cache write' $first.totals.cache_write 7
Check 'output' $first.totals.output 140
Check 'total' $first.totals.total 2667
Check 'provider from codex log' (($first.by_provider | Where-Object { $_.key -eq 'relay' }).total) 330
Check 'copied session counted once' (($first.by_agent | Where-Object { $_.key -eq 'workbuddy' }).total) 630

Write-Host ""
Write-Host "== cache and prune ==" -ForegroundColor Cyan
$second = Run-Usage
Check 'second run uses cache' $second.stats.files_cached $second.stats.files_total
Check 'second run parses nothing' $second.stats.files_parsed 0
Check 'cached total matches first run' $second.totals.total $first.totals.total

Remove-Item (Join-Path $wbDir2 'd.jsonl') -Force
$third = Run-Usage
Check 'deleted file pruned from cache' $third.stats.pruned 1
# The pruned file was the de-duplicated copy, so the aggregate must not move.
Check 'total unchanged after removing duplicate copy' $third.totals.input 1200

Write-Host ""
Write-Host "== real logs (read only) ==" -ForegroundColor Cyan
Remove-Item Env:AGENTHUB_HOME -ErrorAction SilentlyContinue
Remove-Item Env:AGENTHUB_DATA_DIR -ErrorAction SilentlyContinue
$realData = Join-Path $env:TEMP ("ah-usage-real-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
$env:AGENTHUB_DATA_DIR = $realData
$realOut = & $cargo run -q -p agenthub-cli -- usage --range all --json 2>&1
$real = ($realOut -join "`n") | ConvertFrom-Json
$claudeRoot = Join-Path $env:USERPROFILE '.claude\projects'
$claudeFiles = 0
if (Test-Path $claudeRoot) {
  $claudeFiles = (Get-ChildItem $claudeRoot -Recurse -Filter *.jsonl -ErrorAction SilentlyContinue | Measure-Object).Count
}
$claudeReported = ($real.sources | Where-Object { $_.source.key -eq 'claude' }).files
Check 'claude file count matches disk' $claudeReported $claudeFiles
Check 'real data has usage' ($real.totals.total -gt 0) $true

Remove-Item Env:AGENTHUB_DATA_DIR -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $realData -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $sandbox -ErrorAction SilentlyContinue

Write-Host ""
$color = if ($script:fail -eq 0) { 'Green' } else { 'Red' }
Write-Host ("PASS {0} / FAIL {1}" -f $script:pass, $script:fail) -ForegroundColor $color
Write-Host "Note: with AGENTHUB_HOME set, the CodeBuddy IDE root is redirected under the sandbox home" -ForegroundColor DarkGray
Write-Host "      (paths::home_is_overridden); real %LOCALAPPDATA% data is never touched here." -ForegroundColor DarkGray
exit $(if ($script:fail -eq 0) { 0 } else { 1 })
