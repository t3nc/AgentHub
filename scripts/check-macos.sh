#!/usr/bin/env bash
# AgentHub macOS 一键验证：fmt → clippy → test → 前端 typecheck/build → 沙箱端到端。
# 与 scripts/check-windows.ps1 覆盖同一组检查，两端结果应一致。
# 端到端全程在 mktemp 出来的沙箱里跑，不碰 ~/.claude 等真实目录。
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FAILED=()
SKIP_FRONTEND="${SKIP_FRONTEND:-0}"
SKIP_E2E="${SKIP_E2E:-0}"

step() {
  echo
  echo "== $1 =="
}

run() {
  local name="$1"
  shift
  step "$name"
  if "$@"; then
    echo "  PASS"
  else
    echo "  FAIL"
    FAILED+=("$name")
  fi
}

cd "$REPO" || exit 1

run "cargo fmt --check" cargo fmt --all --check
run "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
run "cargo test" cargo test -p agenthub-core
# 网关集成测试（本地 fake upstream）
run "cargo test gateway" cargo test -p agenthub-gateway
# 桌面壳：真起 PTY 跑 $SHELL（TERM / COLORTERM / 初始尺寸）+ 关闭标签后进程回收 + 字体排序
run "cargo test desktop" cargo test -p agenthub-desktop
run "cargo build --workspace" cargo build --workspace

if [[ "$SKIP_FRONTEND" != "1" ]]; then
  run "npm run typecheck" npm --prefix apps/desktop run typecheck
  run "npm run build" npm --prefix apps/desktop run build
fi

# 本地化门槛：词典三语齐平 + 后端无残留（前端有意保留项只报告）
run "i18n 校验" node "$REPO/scripts/i18n-check.mjs" --strict

if [[ "$SKIP_E2E" != "1" ]]; then
  SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-e2e.XXXXXX")"
  export AGENTHUB_HOME="$SANDBOX/home"
  export AGENTHUB_DATA_DIR="$SANDBOX/data"
  mkdir -p "$AGENTHUB_DATA_DIR"
  for agent in .claude .codex .hermes; do
    dir="$AGENTHUB_HOME/$agent/skills/cloudflare"
    mkdir -p "$dir"
    printf '# cloudflare\n' >"$dir/SKILL.md"
    printf 'same\n' >"$dir/notes.md"
  done

  CLI="$REPO/target/debug/agenthub"
  PLAN="$SANDBOX/plan.json"

  run "端到端：scan" "$CLI" scan
  run "端到端：plan-dedupe" "$CLI" plan-dedupe --out "$PLAN"
  run "端到端：apply" "$CLI" apply "$PLAN" --yes

  step "端到端：链接与真身校验"
  e2e_ok=1
  [[ -f "$AGENTHUB_DATA_DIR/store/cloudflare/SKILL.md" ]] || { echo "  中央仓库真身丢失"; e2e_ok=0; }
  for agent in .claude .codex .hermes; do
    link="$AGENTHUB_HOME/$agent/skills/cloudflare"
    [[ -L "$link" ]] || { echo "  $link 不是符号链接"; e2e_ok=0; }
    [[ -f "$link/SKILL.md" ]] || { echo "  $link 读不到内容"; e2e_ok=0; }
  done
  "$CLI" doctor
  if [[ $e2e_ok -eq 1 ]]; then echo "  PASS"; else echo "  FAIL"; FAILED+=("端到端：链接与真身校验"); fi

  BATCH_ID="$("$CLI" batches | awk '/cs-/{print $1; exit}')"
  run "端到端：rollback" "$CLI" rollback "$BATCH_ID"

  step "端到端：回滚校验"
  rollback_ok=1
  for agent in .claude .codex .hermes; do
    dir="$AGENTHUB_HOME/$agent/skills/cloudflare"
    [[ -d "$dir" && ! -L "$dir" ]] || { echo "  $dir 回滚后不是实体目录"; rollback_ok=0; }
    [[ -f "$dir/SKILL.md" ]] || { echo "  $dir 回滚后内容缺失"; rollback_ok=0; }
  done
  [[ ! -e "$AGENTHUB_DATA_DIR/store/cloudflare" ]] || { echo "  回滚后仓库应清空"; rollback_ok=0; }
  if [[ $rollback_ok -eq 1 ]]; then echo "  PASS"; else echo "  FAIL"; FAILED+=("端到端：回滚校验"); fi

  rm -rf "$SANDBOX"
fi

# UI 断言：需要一个跑着的演示站点（npm --prefix apps/desktop run dev，5273）与一个 CDP 端点。
# 默认跳过（CI 上没有浏览器）；本机跑：UI=1 ./scripts/check-macos.sh
if [[ "${UI:-0}" == "1" ]]; then
  run "UI 断言（ui-smoke）" node "$REPO/scripts/ui-smoke.mjs"
else
  step "UI 断言（ui-smoke）"
  echo "  跳过：设 UI=1 && 先起 npm --prefix apps/desktop run dev + moli serve --layout"
fi

echo
if [[ ${#FAILED[@]} -eq 0 ]]; then
  echo "全部通过"
  exit 0
fi
echo "失败的步骤: ${FAILED[*]}"
exit 1
