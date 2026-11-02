# 贡献指南

感谢愿意花时间。请先读完这一页——尤其是「这个仓库是什么」和「许可与再许可」两节，它们会影响你的提交方式。

## 这个仓库是什么

`t3nc/AgentHub` 是对外**发布快照**：维护者在自己的开发仓里完成开发，再按白名单导出到这里。
所以你看到的提交历史是一条条发布记录，而不是完整的开发历史。

由此带来两个约定：

1. **Issue 是最有效的入口**。Bug 报告、功能建议、使用问题都欢迎。
2. **PR 不会直接在这里合并**。你可以提交 PR，维护者会把它在开发仓里落地（保留你的署名），
   下一版发布时同步过来。如果你希望更快，也可以直接把补丁发到 Issue 里。

## 环境准备

```bash
git clone https://github.com/t3nc/AgentHub.git
cd AgentHub
cd apps/desktop && npm ci && cd ../..
cargo build -p agenthub-cli
```

完整依赖与打包方式见 [docs/build.md](docs/build.md)。Windows 需要 VS 2022 生成工具（C++ 桌面开发）+ WebView2；
macOS 需要 Xcode Command Line Tools。Linux 未验证。

## 提交前必须通过的检查

CI 会在 Windows 与 macOS 两端跑同一组检查，本地对不上的会直接红：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p agenthub-core
cd apps/desktop && npm run typecheck && npm run build
node scripts/i18n-check.mjs --strict
```

也可以一键跑：

```bash
./scripts/check-macos.sh                                        # macOS / Linux
powershell -ExecutionPolicy Bypass -File scripts\check-windows.ps1   # Windows
```

端到端测试全程在临时目录的沙箱里跑，不会碰 `~/.claude` 等真实目录。**请不要**为了让测试变绿去改真实环境，
也不要删断言或加 `#[allow]` 绕过。

## 代码约定

1. **core 不依赖 Tauri**。业务逻辑放 `crates/agenthub-core`，CLI 与 GUI 都只是它的壳。
2. **写操作只有一条路径**：`plan_*()` 产出 `ChangeSet` → `apply()` 执行。命令层不允许直接写文件。
3. **平台差异集中在 `platform.rs`**。新增 `#[cfg(windows)]` 必须补对应的 `#[cfg(not(windows))]`；
   测试断言用 `LinkState` / `platform::is_link_like` 这类语义 API，不要断言 `is_symlink()`。
4. **路径必须过门禁**。写入目标要落在受管根目录内，越界抛 `UnsafePath`。
5. **换行符保真**：读写配置时用 `text.rs` 里现成的函数，不要自己 `replace("\r\n", "\n")`。

## 三语文案

界面支持英文 / 简体中文 / 繁体中文。

- 前端词典在 `apps/desktop/src/lib/i18n/{zh-Hans,en,zh-Hant}/`，按域拆文件；`zh-Hans` 是 key 基准，
  另两语用类型约束锁齐平，**少一条或多一条都编译不过**。
- 后端词典在 `crates/agenthub-core/src/l10n/catalog.rs`，`messages!` 宏一次声明三语。
- 新增界面文案必须三语齐全；`node scripts/i18n-check.mjs` 会统计残留。
- **测试里不要断言某一种语言的文案**（全局 locale 与并行用例冲突）。断言结构、id 或错误变体。

## 提交信息

- 一个提交做一件事，消息前缀用 `feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`。
- 正文写清「改了什么 + 怎么验证的」。涉及界面的改动附截图或录屏更好。
- 不要在一个 PR 里混入无关的格式化改动。

## 许可与再许可（重要）

本项目采用自定义非商业许可（见 [LICENSE](LICENSE)）：允许个人使用与组织内部使用，
禁止销售、转售与并入付费产品或服务。

由于维护者需要保留「向商业用户单独授权」的能力，**提交贡献即表示你同意**：

1. 你的贡献以本项目当前许可发布；
2. 你授予维护者一项永久的、全球的、免费的、不可撤销的权利，可以以其他条款（包括商业许可）再许可你的贡献；
3. 你的贡献是原创的，或你已获得足够权利按上述方式提交。

如果你不能接受这三条，请不要提交代码，仍然欢迎提 Issue。

## 行为准则

- 讨论对事不对人，不攻击、不嘲讽、不刷屏。
- 不提交他人的私密信息（真实密钥、私人路径、会话日志原文）。
- 维护者可以关闭偏离主题或无法复现的讨论，并会说明理由。
