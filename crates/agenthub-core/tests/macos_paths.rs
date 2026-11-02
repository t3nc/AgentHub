//! macOS 平台专属断言。
//!
//! 这里钉住的是 2026-10-29 在 Mac 真机上实测到的布局，防止后续「顺手把默认值改回 Windows 形态」：
//! - Hermes 在 macOS 上只有 `~/.hermes` 一个候选根（`%LOCALAPPDATA%\hermes` 是 Windows 安装器的）；
//! - OpenCode 的技能来自兼容目录 `~/.agents/skills` 与 `~/.claude/skills`，且不接管 MCP；
//! - WorkBuddy 在 macOS 上同样落在 `~/.workbuddy`，可以纳入受管；
//! - Claude Desktop 的 MCP 配置在 `~/Library/Application Support/Claude/`。
//!
//! 整个文件在非 macOS 上编译成空 crate（Windows 侧的对应断言在 `catalog.rs` / `windows_compat.rs`）。
#![cfg(target_os = "macos")]

use std::fs;

use agenthub_core::adapters::AgentAdapter;
use agenthub_core::adapters::builtin::{self, BuiltinAdapter};
use agenthub_core::ext::catalog;
use agenthub_core::model::agent::{AgentId, Scope};

fn adapter(all: &[BuiltinAdapter], id: AgentId) -> &BuiltinAdapter {
    all.iter()
        .find(|a| a.agent == id)
        .unwrap_or_else(|| panic!("{id:?} 不在内置适配器表里"))
}

/// Hermes：空目录与「有 config.yaml」两种情况都必须选 `~/.hermes`。
///
/// Windows 上 `~/.hermes` 可能是早期版本留下的空目录，真实数据在 `%LOCALAPPDATA%\hermes`；
/// macOS 没有那个候选，`pick_root` 的「跳过空目录」逻辑不能把 mac 的根跳成别处。
#[test]
fn hermes_root_is_dot_hermes_on_macos() {
    let home = tempfile::tempdir().unwrap();

    // 什么都不存在时回退首选 `~/.hermes`
    let all = builtin::all(home.path());
    assert_eq!(
        adapter(&all, AgentId::Hermes).config_dir,
        home.path().join(".hermes"),
        "空沙箱里应该回退到 ~/.hermes"
    );

    fs::create_dir_all(home.path().join(".hermes")).unwrap();
    fs::write(home.path().join(".hermes/config.yaml"), "mcp_servers: {}\n").unwrap();
    let all = builtin::all(home.path());
    let hermes = adapter(&all, AgentId::Hermes);
    assert_eq!(hermes.config_dir, home.path().join(".hermes"));
    assert_eq!(
        hermes.skills_dir.as_deref(),
        Some(home.path().join(".hermes/skills").as_path())
    );
    assert_eq!(
        hermes.mcp_user.as_ref().map(|(path, _)| path.clone()),
        Some(home.path().join(".hermes/config.yaml"))
    );
}

/// OpenCode：自己的技能目录 + 两个兼容目录；MCP 不接管（`opencode.jsonc` 的 `mcp` 是数组形态）。
#[test]
fn opencode_reads_claude_and_agents_skill_dirs() {
    let home = tempfile::tempdir().unwrap();
    let all = builtin::all(home.path());
    let opencode = adapter(&all, AgentId::Opencode);

    assert_eq!(
        opencode.skills_dir.as_deref(),
        Some(home.path().join(".config/opencode/skills").as_path())
    );
    assert_eq!(
        opencode.extra_skills_dirs,
        vec![
            home.path().join(".agents/skills"),
            home.path().join(".claude/skills"),
        ],
        "官方文档确认 OpenCode 会读这两处兼容目录"
    );
    assert!(
        !opencode.caps.mcp,
        "OpenCode 的 MCP 是数组形态，与 mcpServers 不兼容，不接管"
    );

    // 用户级技能目标 = 本地目录 + 兼容目录，缺一个就会漏扫
    let targets = opencode.skill_targets(&Scope::User);
    let dirs: Vec<_> = targets.iter().map(|t| t.dir.clone()).collect();
    assert_eq!(dirs.len(), 3);
    assert!(dirs.contains(&home.path().join(".claude/skills")));
}

/// WorkBuddy：macOS 实测目录与 Windows 同构，因此必须可纳入受管（不再有 blocked 原因）。
#[test]
fn workbuddy_is_manageable_on_macos() {
    let spec = catalog::find("workbuddy").expect("目录表里要有 WorkBuddy");
    assert!(
        spec.blocked.is_none(),
        "macOS 已实测，不该再挂「未实测」的 blocked 原因：{:?}",
        spec.blocked
    );

    let def = catalog::managed_def("workbuddy").expect("macOS 上应该可以纳入受管");
    assert_eq!(def.id, "workbuddy");
    assert_eq!(def.skills_dir.as_deref(), Some("~/.workbuddy/skills"));
    assert_eq!(def.mcp_file.as_deref(), Some("~/.workbuddy/mcp.json"));
    assert_eq!(
        def.instructions,
        vec!["~/.workbuddy/SOUL.md", "~/.workbuddy/USER.md"]
    );
}

/// 国内版 / 国际版在 macOS 上同样是两套数据根与两套 .app，不能互相回退。
#[test]
fn intl_and_cn_variants_stay_separate_on_macos() {
    for (key, id, skills, mcp) in [
        (
            "workbuddy",
            "workbuddy",
            "~/.workbuddy/skills",
            "~/.workbuddy/mcp.json",
        ),
        (
            "workbuddy-ai",
            "workbuddy-ai",
            "~/.workbuddy-ai/skills",
            "~/.workbuddy-ai/mcp.json",
        ),
    ] {
        let def = catalog::managed_def(key).unwrap_or_else(|e| panic!("{key} 应该可受管: {e}"));
        assert_eq!(def.id, id);
        assert_eq!(def.skills_dir.as_deref(), Some(skills));
        assert_eq!(def.mcp_file.as_deref(), Some(mcp));
    }

    // 国际版人格文件未观察，不该混进国内版的 SOUL.md / USER.md
    let intl = catalog::managed_def("workbuddy-ai").unwrap();
    assert!(intl.instructions.is_empty());

    // Trae 两版：id 不同、项目文件同名
    let trae = catalog::managed_def("trae").expect("Trae 应该可登记");
    assert_eq!(trae.project_mcp_file.as_deref(), Some(".trae/mcp.json"));
    let trae_cn = catalog::managed_def("trae-cn").expect("Trae CN 应该可登记");
    assert_eq!(trae_cn.id, "trae-cn");
    assert_eq!(trae_cn.project_mcp_file.as_deref(), Some(".trae/mcp.json"));

    let cn_app = catalog::find("trae-cn").unwrap();
    assert!(cn_app.mac_apps.contains(&"/Applications/Trae CN.app"));
    let wb_ai = catalog::find("workbuddy-ai").unwrap();
    assert!(wb_ai.mac_apps.contains(&"/Applications/WorkBuddy AI.app"));
}

/// Claude Desktop：macOS 的 MCP 配置在 `~/Library/Application Support/Claude/`，且没有技能目录。
#[test]
fn claude_desktop_mcp_lives_in_library() {
    let def = catalog::managed_def("claude-desktop").expect("Claude Desktop 应该可以纳入受管");
    assert_eq!(
        def.mcp_file.as_deref(),
        Some("~/Library/Application Support/Claude/claude_desktop_config.json"),
        "macOS 上不能回落到 %APPDATA% 那套路径"
    );
    assert!(def.skills_dir.is_none(), "Claude Desktop 没有技能目录约定");
    assert!(def.instructions.is_empty());
}

/// CodeBuddy IDE 与 Trae CN：macOS 侧的数据根与「未实测」状态。
///
/// 2026-11-02 在装了 `CodeBuddy CN.app` / `Trae CN.app` 的 Mac 上复核：
/// - CodeBuddy CN 的数据根是 `~/Library/Application Support/CodeBuddy CN`（VSCode 派生），
///   `~/.codebuddycn` 只有 argv.json + extensions，两边都没有 mcpServers → 仍保持 blocked；
/// - Trae CN 的数据根是 `~/Library/Application Support/Trae CN` 与 `~/.trae-cn`，后者确实有
///   `skills/`（用户自建）与 `mcps/`，但用户级 MCP 文件没有约定，模板仍只接管项目级。
#[test]
fn codebuddy_ide_and_trae_cn_paths_match_macos_reality() {
    let ide = catalog::find("codebuddy-ide").expect("目录表里要有 CodeBuddy IDE");
    assert!(
        ide.mac_apps.contains(&"/Applications/CodeBuddy CN.app"),
        "国内版 app 名带 CN 后缀"
    );
    for path in [
        "~/Library/Application Support/CodeBuddy",
        "~/Library/Application Support/CodeBuddy CN",
        "~/.codebuddycn",
    ] {
        assert!(
            ide.mac_config.contains(&path),
            "CodeBuddy IDE 的 mac_config 少了 {path}"
        );
    }
    assert!(
        ide.blocked.is_some(),
        "IDE 的 MCP / 技能文件约定仍未实测到，不该解除 blocked"
    );
    assert!(
        catalog::managed_def("codebuddy-ide").is_err(),
        "blocked 的条目不允许纳入受管"
    );

    let trae_cn = catalog::find("trae-cn").expect("目录表里要有 Trae CN");
    assert!(trae_cn.mac_apps.contains(&"/Applications/Trae CN.app"));
    for path in ["~/Library/Application Support/Trae CN", "~/.trae-cn"] {
        assert!(
            trae_cn.mac_config.contains(&path),
            "Trae CN 的 mac_config 少了 {path}"
        );
    }
}

/// macOS 没有 Store（MSIX）概念：任何 `mac_config` 都不该出现 Windows 占位符。
#[test]
fn mac_configs_have_no_windows_placeholders() {
    for spec in catalog::specs() {
        for raw in spec.mac_config {
            assert!(
                !raw.contains('%') && !raw.contains('\\'),
                "{} 的 mac_config 混进了 Windows 写法：{raw}",
                spec.key
            );
        }
    }
}
