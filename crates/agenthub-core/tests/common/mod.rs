use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use agenthub_core::AppConfig;

/// 同一测试二进制里的用例是并行跑的，`ts_label()` 只到秒，
/// 单靠进程号 + 时间戳会让并行用例共用同一个沙箱目录（互相污染）。
static SANDBOX_SEQ: AtomicUsize = AtomicUsize::new(0);

/// 注意：`paths::init_overrides` 用的是 `OnceLock`，**一个测试进程只能钉死一次**
/// home/data。所以同一个测试文件里不要放两个各自建沙箱的 `#[test]`——
/// 第二个用例会拿到第一个用例的 home，扫描结果对不上。
/// 需要多组场景就在同一个用例里顺序做，或者拆到不同的测试文件（各自是独立进程）。
#[allow(dead_code)]
pub fn write_skill(dir: &Path, name: &str, body: &str) {
    fs::create_dir_all(dir).unwrap();
    let text = format!(
        "---\nname: {name}\ndescription: {name} demo skill used by AgentHub tests.\n---\n\n# {name}\n\n{body}\n"
    );
    fs::write(dir.join("SKILL.md"), text).unwrap();
    fs::write(dir.join("notes.md"), format!("notes for {name}\n{body}\n")).unwrap();
}

#[allow(dead_code)]
pub fn sandbox() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "agenthub-test-{}-{}-{}",
        std::process::id(),
        agenthub_core::paths::ts_label(),
        SANDBOX_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let home = root.join("home");
    let data = root.join("data");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&data).unwrap();
    (home, data)
}

/// 4 个 agent 各一份相同技能 + 1 个独有技能 + 1 个只读系统技能，外加三种 MCP 配置。
#[allow(dead_code)]
pub fn setup(home: &Path) {
    for agent in [".claude", ".codex", ".hermes", ".gemini"] {
        write_skill(
            &home.join(agent).join("skills/cloudflare"),
            "cloudflare",
            "same",
        );
    }
    write_skill(
        &home.join(".claude/skills/only-claude"),
        "only-claude",
        "unique",
    );
    write_skill(
        &home.join(".codex/skills/.system/imagegen"),
        "imagegen",
        "sys",
    );
    fs::create_dir_all(home.join(".codebuddy/skills")).unwrap();

    fs::write(
        home.join(".claude.json"),
        r#"{"mcpServers":{},"theme":"dark","numStartups":3}"#,
    )
    .unwrap();
    fs::write(
        home.join(".codex/config.toml"),
        "model_provider = \"custom\"\nmodel = \"gpt-6.1-sol\"\n\n[model_providers.custom]\nname = \"custom\"\nbase_url = \"https://example.com\"\n",
    )
    .unwrap();
    fs::write(
        home.join(".hermes/config.yaml"),
        "# hermes\nmcp_discovery_timeout: 1.5\nmcp_servers:\n  old:\n    command: npx\nplatform_toolsets:\n  cli:\n    - browser\n",
    )
    .unwrap();
}

/// 用「平台默认策略」建目录链接：macOS/Linux 符号链接，Windows 目录联接。
///
/// 测试里不要直接写 `std::os::unix::fs::symlink`——那样在 Windows 上编译都过不去，
/// 而且断言 `is_symlink()` 会把 junction 判成实体目录。
#[allow(dead_code)]
pub fn link_dir(target: &Path, at: &Path) {
    let strategy = agenthub_core::link::LinkStrategy::platform_default();
    agenthub_core::link::create_dir_link(target, at, strategy)
        .unwrap_or_else(|e| panic!("建链接失败 {} -> {}: {e}", at.display(), target.display()));
}

/// 断言某路径确实是链接（软链 / Windows 目录联接都算）。
#[allow(dead_code)]
pub fn assert_linked(at: &Path) {
    assert!(
        agenthub_core::platform::is_link_like(at),
        "{} 应为链接（当前平台 {}）",
        at.display(),
        std::env::consts::OS
    );
}

#[allow(dead_code)]
pub fn link_target(at: &Path) -> PathBuf {
    fs::read_link(at).unwrap_or_else(|e| panic!("读取链接 {} 失败: {e}", at.display()))
}

#[allow(dead_code)]
pub fn hub(home: &Path, data: &Path) -> agenthub_core::AgentHub {
    agenthub_core::paths::init_overrides(home, data);
    let config = AppConfig {
        store_root: data.join("store"),
        ..AppConfig::default()
    };
    agenthub_core::AgentHub::open(config).unwrap()
}
