//! 跨平台兼容回归。
//!
//! 原则：断言「语义」而不是「实现」——同一份断言在 macOS 与 Windows 都成立；
//! 确实只属于某个平台的细节，用 `#[cfg]` 精确标注并在注释里写清原因。

use std::path::Path;

use agenthub_core::instructions::managed_block;
use agenthub_core::mcp::merge_yaml;
use agenthub_core::model::mcp::{McpFormat, McpServer, Transport};
use agenthub_core::{paths, text};
// `platform::is_junction` 只在 Windows 用例里出现，非 Windows 平台导入即未使用（`-D warnings` 会红）。
#[cfg(windows)]
use agenthub_core::platform;

/// 断言文本里没有「裸 LF」——即所有换行都是 CRLF。
fn assert_all_crlf(out: &str) {
    if out.is_empty() {
        return;
    }
    assert!(
        !out.replace("\r\n", "\u{0}").contains('\n'),
        "输出里残留了裸 LF（说明原文件的 CRLF 被改写）:\n{out:?}"
    );
}

#[test]
fn crlf_instruction_file_keeps_crlf_on_inject() {
    let existing = "# 用户自己的规则\r\n\r\n第二段\r\n";
    let out = managed_block::inject(Some(existing), "受控内容").unwrap();

    assert_all_crlf(&out);
    assert!(
        out.starts_with("# 用户自己的规则\r\n"),
        "区块外内容必须逐字节保留"
    );
    assert!(out.contains("受控内容"));
    assert!(managed_block::find_block(&out).present);
}

#[test]
fn crlf_instruction_file_keeps_crlf_on_strip() {
    let existing = managed_block::inject(Some("# 头\r\n"), "body").unwrap();
    let stripped = managed_block::strip(&existing);
    assert_all_crlf(&stripped);
    assert!(!managed_block::find_block(&stripped).present);
}

#[test]
fn lf_instruction_file_stays_lf() {
    let existing = "# head\n\nbody\n";
    let out = managed_block::inject(Some(existing), "受控").unwrap();
    assert!(!out.contains('\r'), "LF 文件不应被改成 CRLF: {out:?}");
}

#[test]
fn crlf_yaml_upsert_keeps_crlf() {
    let original = "# hermes\r\nmcp_servers:\r\n  old:\r\n    command: npx\r\nplatform_toolsets:\r\n  cli: []\r\n";
    let out = merge_yaml::upsert(Some(original), "mcp_servers", &[]).unwrap();

    assert_all_crlf(&out);
    assert!(out.contains("platform_toolsets:"), "顶层其它键必须保留");
    assert!(out.contains("mcp_servers: {}"), "空列表应渲染成 {{}}");
}

/// Windows 编辑器（记事本 / PowerShell 5.1 的 Set-Content / Visual Studio）默认写 UTF-8 BOM。
#[test]
fn frontmatter_ignores_bom() {
    let fm = agenthub_core::scan::frontmatter::parse_text(
        "\u{feff}---\nname: demo\ndescription: demo skill\n---\n\n# demo\n",
    );
    assert_eq!(
        fm.name.as_deref(),
        Some("demo"),
        "带 BOM 的 SKILL.md 也要能解析出 frontmatter"
    );
}

#[test]
fn mcp_upsert_preserves_bom() {
    let format = McpFormat::Json {
        pointer: vec!["mcpServers".into()],
    };
    let server = McpServer {
        name: "demo".into(),
        transport: Transport::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "demo-mcp".into()],
            env: Default::default(),
            cwd: None,
        },
        enabled: true,
        timeout_secs: None,
        note: None,
    };
    let out = agenthub_core::mcp::upsert(Some("\u{feff}{\"mcpServers\":{}}"), &format, &[server])
        .expect("带 BOM 的 JSON 也应能合并");
    assert!(out.starts_with('\u{feff}'), "原文件带 BOM 时应保留");
    assert!(out.contains("demo"), "新条目应写入");
}

#[test]
fn eol_detection_matches_first_newline() {
    assert_eq!(text::eol_of("a\r\nb"), "\r\n");
    assert_eq!(text::eol_of("a\nb"), "\n");
    assert_eq!(text::eol_of("single"), "\n");
    assert_eq!(text::with_eol("a\nb", "\r\n"), "a\r\nb");
}

#[test]
fn expand_understands_home_prefixes() {
    let home = paths::home_dir();
    assert!(paths::paths_eq(
        &paths::expand("~/skills"),
        &home.join("skills")
    ));
    assert!(paths::paths_eq(
        &paths::expand("$HOME/skills"),
        &home.join("skills")
    ));
    assert_eq!(paths::expand("  "), std::path::PathBuf::new());
}

/// Git Bash 会上 Windows 的 `~\x` 形态；与 `~/x` 等价。
#[test]
fn expand_understands_windows_tilde_separator() {
    let home = paths::home_dir();
    assert!(paths::paths_eq(
        &paths::expand(r"~\.claude\skills"),
        &home.join(".claude").join("skills")
    ));
}

#[test]
fn is_within_is_component_wise() {
    let root = Path::new("/tmp/agenthub/root");
    assert!(paths::is_within(root, Path::new("/tmp/agenthub/root/a")));
    assert!(paths::is_within(root, root));
    assert!(
        !paths::is_within(root, Path::new("/tmp/agenthub/rootkit/a")),
        "同前缀但不同目录名不得判为之内"
    );
}

/// Windows 文件系统大小写不敏感：同一目录两种写法必须产生同一个库键。
#[cfg(windows)]
#[test]
fn path_key_folds_case_on_windows() {
    assert_eq!(
        paths::path_key(Path::new(r"C:\Users\Demo\.claude\skills")),
        paths::path_key(Path::new(r"c:\users\demo\.claude\SKILLS"))
    );
    assert!(paths::paths_eq(
        Path::new(r"C:\Users\Demo\proj"),
        Path::new(r"c:/users/demo/proj")
    ));
}

/// Unix 上保留大小写（macOS 索引库已按原始大小写建立，不做 fold）。
#[cfg(not(windows))]
#[test]
fn path_key_keeps_case_on_unix() {
    assert_ne!(
        paths::path_key(Path::new("/Users/Demo/.claude")),
        paths::path_key(Path::new("/Users/demo/.claude"))
    );
}

#[cfg(windows)]
#[test]
fn path_key_normalizes_separators_on_windows() {
    assert_eq!(
        paths::path_key(Path::new(r"C:\Users\Demo\project")),
        paths::path_key(Path::new("C:/Users/Demo/project"))
    );
}

#[cfg(windows)]
#[test]
fn expand_expands_env_prefix() {
    let appdata = std::env::var("APPDATA").expect("Windows 上应有 APPDATA");
    let expanded = paths::expand(r"%APPDATA%\dsh\skills");
    assert!(
        paths::paths_eq(
            &expanded,
            &std::path::PathBuf::from(appdata).join("dsh").join("skills")
        ),
        "实际: {}",
        expanded.display()
    );
    // 变量不存在时原样返回，不猜测
    assert_eq!(
        paths::expand("%AGENTHUB_NOT_A_REAL_VAR%\\x"),
        std::path::PathBuf::from("%AGENTHUB_NOT_A_REAL_VAR%\\x")
    );
}

/// 重点回归：Windows 上删除 junction 不能沿 reparse point 递归删掉 store 真身。
#[cfg(windows)]
#[test]
fn removing_junction_keeps_store_content() {
    use agenthub_core::link::{LinkStrategy, create_dir_link, remove_linked_or_dir};

    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("store").join("cloudflare");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.join("SKILL.md"), "真身内容").unwrap();

    let agent_dir = root.path().join("agent").join("skills");
    std::fs::create_dir_all(&agent_dir).unwrap();
    let at = agent_dir.join("cloudflare");
    create_dir_link(&store, &at, LinkStrategy::Junction).unwrap();

    assert!(platform::is_junction(&at), "Windows 默认策略应建出目录联接");
    assert!(platform::is_link_like(&at), "junction 必须被识别为链接");
    assert!(at.join("SKILL.md").is_file(), "经 junction 应能读到内容");

    let target = std::fs::read_link(&at).unwrap();
    assert!(
        paths::paths_eq(&target, &store),
        "read_link 应解析出 store 目标，实际 {target:?}"
    );

    remove_linked_or_dir(&at, false).unwrap();
    assert!(!at.exists(), "链接本身应被摘掉");
    assert!(
        store.join("SKILL.md").is_file(),
        "删除 junction 绝不能递归删掉中央仓库里的真身"
    );
}
