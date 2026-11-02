# 构建与运行

本文只讲「怎么把这份源码跑起来、打包出来」。产品能力与用法见根目录 [README](../README.md)。

## 1. 依赖

| 依赖 | 版本 | 说明 |
|---|---|---|
| Rust | stable（edition 2024，最低 1.90） | `rust-toolchain.toml` 已钉 stable + clippy/rustfmt，装了 rustup 就会自动用对版本 |
| Node.js | 22.x 起 | 仅前端与 Tauri CLI 需要 |
| Tauri 2 系统依赖 | 见下 | 两端不同 |

### Windows

1. **Visual Studio 2022 生成工具** → 勾选「使用 C++ 的桌面开发」（`MSVC v143` + Windows SDK）。
   只装 rustup 不装它，链接阶段会报 `link.exe not found` 或 `LNK1104`。
2. **WebView2 运行时**：Windows 11 自带；Windows 10 一般也已随 Edge 安装。
3. **rustup**：安装时选 `x86_64-pc-windows-msvc` 工具链。

```powershell
# 确认工具链齐了
rustc -V
cargo -V
node -v
```

### macOS

```bash
xcode-select --install      # Xcode Command Line Tools
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
node -v                     # 22.x 起
```

### Linux

上游只验证了 macOS 与 Windows。Linux 需要 `libwebkit2gtk-4.1-dev` 等 Tauri 依赖，**未经测试**，遇到问题欢迎开 Issue。

## 2. 取代码与装依赖

```bash
git clone https://github.com/t3nc/AgentHub.git
cd AgentHub

cd apps/desktop
npm ci                      # 有 lockfile，用 ci 而不是 install
cd ../..
```

> npm 默认源慢时：`npm ci --registry=https://registry.npmmirror.com`（不要改全局配置）。

## 3. 跑 CLI

```bash
cargo run -p agenthub-cli -- scan
cargo run -p agenthub-cli -- --help
```

首次编译要拉完整依赖树，几分钟是正常的。

装成全局命令：

```bash
cargo install --path crates/agenthub-cli
agenthub scan
```

## 4. 跑桌面端

两种形态，别混：

| 目的 | 命令 | 前端从哪来 |
|---|---|---|
| 浏览器演示模式（改界面最方便） | `cd apps/desktop && npm run dev` | vite，`http://localhost:5273` |
| 原生窗口 + 热更 | `cd apps/desktop && npx tauri dev` | 同上的 dev server（`beforeDevCommand` 自动起） |
| 自包含二进制（双击即跑） | `cd apps/desktop && npm run tauri build -- --no-bundle` | 编译期把 `dist/` 嵌进二进制 |
| 同上但只用 cargo | `npm run build` → `cargo build --release -p agenthub-desktop --features custom-protocol` | 同上 |

浏览器里没有 Tauri 运行时，会自动切到**演示模式**：内置一套与真实环境同构的样本数据，
「生成计划 → 预览 → 执行 → 回滚」都能走通，但不碰任何真实目录。窗口内检测到 Tauri 运行时才会走真实 IPC。

### 一个必须知道的坑

在仓库根直接 `cargo run -p agenthub-desktop` 或 `cargo build`（不带 `custom-protocol` feature）编出来的是 **dev 模式**产物：
它去连 `devUrl`（`http://localhost:5273`），而不是加载内嵌的 `dist/`。dev server 没起时双击只会看到「无法访问此页面 / localhost 拒绝连接」。

- 要出自包含二进制：`npm run tauri build -- --no-bundle`，或显式加 `--features custom-protocol`。
- `dist/` 是**编译期**读入的：改了前端要先 `npm run build` 再编壳，否则嵌进去的是旧页面。
- 自检：`dist/assets/` 里的字体文件名（如 `AgentHubNerdMono-XXXX.ttf`）能否在二进制里被字节检索到。命中 = 已内嵌。

产物在 workspace 共享根：`target/release/agenthub-desktop.exe`（Windows）或 `target/release/agenthub-desktop`（macOS）。

## 5. 打安装包

```powershell
# Windows → NSIS，产物 target\release\bundle\nsis\
powershell -ExecutionPolicy Bypass -File scripts\build-installer.ps1
```

```bash
# macOS → dmg
cd apps/desktop && npm run tauri build -- --bundles dmg
```

- 首次打包需要联网下载打包器（NSIS / WiX）。只打 NSIS 可以避开 WiX 的额外下载（脚本已加 `--bundles nsis`）。
- macOS 产物默认**没有签名与公证**，分发给别人时对方需要右键 → 打开。
- 图标由 `npx tauri icon src-tauri/icons/icon.png` 生成，换图标后重跑一次。

## 6. 改完代码怎么自检

两端各有一条一键验证，覆盖同一组检查（fmt → clippy → test → 前端 typecheck/build → 沙箱端到端）：

```bash
./scripts/check-macos.sh
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts\check-windows.ps1
```

也可以逐条跑：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p agenthub-core
cd apps/desktop && npm run typecheck && npm run build
node scripts/i18n-check.mjs --strict
```

端到端测试全程在临时目录的沙箱里跑，**不碰** `~/.claude` 等真实目录。

## 7. 故障排查

| 症状 | 处理 |
|---|---|
| `Port 5273 is already in use` | 前一次 dev 进程没退，或别的项目占了端口。查一下再决定杀谁：<br>macOS `lsof -nP -iTCP:5273 -sTCP:LISTEN`<br>Windows `Get-NetTCPConnection -LocalPort 5273 -State Listen` |
| `npm run dev` 日志里出现 `oxc` / `rolldown` / `vite:react-babel` | `vite` 被升到了 8.x。Vite 8 走 rolldown，与 `@vitejs/plugin-react` 4 + Tailwind 3.4 不兼容。本项目锁定 `vite ^5.4.11`，别执行 `npm i -D vite@latest` |
| `cargo` 卡在 `Updating crates.io index` | 网络问题。可以给本机配一个镜像（写进 `~/.cargo/config.toml`，**不要提交到仓库**），或换网络 |
| 原生窗口白屏 | `apps/desktop/dist/` 是否已构建，`tauri.conf.json` 的 `frontendDist` 是否指向 `../dist` |
| Windows `link.exe not found` / `LNK1104` | 没装 VS 生成工具，见 §1 |
| Windows 建链接提示「需要管理员权限」 | 该次用的是符号链接策略；改用默认的目录联接（junction），或开启开发者模式 |
| 双击 exe 报「拒绝连接」 | 见 §4 的坑：这是 dev 模式产物，需要用 `tauri build` 重新打包 |
| 改了前端但界面没变 | `dist/` 没有重新构建；`npm run build` 后重新编壳 |
| 扫描结果为空 | `AGENTHUB_HOME` 是否被设到了沙箱；`config.json` 的 `store_root` 是否指错 |
