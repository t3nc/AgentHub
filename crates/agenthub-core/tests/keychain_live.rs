//! 系统钥匙串的真机用例（macOS `security` / Windows 凭据管理器）。
//!
//! **默认跳过**：只有 `KEYCHAIN_LIVE=1` 时才真的往登录钥匙串写一条自测条目——本机会因此
//! 留下痕迹（跑完自动删除），也可能弹出「security 想访问钥匙串」授权框，所以不进日常
//! `cargo test`。
//!
//! 钉住三条语义（与 `mcp/secrets.rs` 的错误分类一一对应）：
//! 1. 条目存在 → 读回原值；
//! 2. 条目不存在 → 报「没有 …」，而不是笼统的读取失败；
//! 3. 条目存在但值为空 → 报「内容为空」，不能把空串当密钥写进配置。

// 只在 macOS 用得上：非 mac 平台 compiles 掉整个用例，这两个 use 会变成 unused-import，
// 而 `-D warnings` 下直接失败（Windows 上 clippy 的实测结果）。
#[cfg(target_os = "macos")]
use agenthub_core::mcp::secrets;
#[cfg(target_os = "macos")]
use agenthub_core::model::mcp::SecretRef;

#[cfg(target_os = "macos")]
fn live() -> bool {
    match std::env::var("KEYCHAIN_LIVE").ok().as_deref() {
        Some("1") => true,
        _ => {
            eprintln!("跳过：未设置 KEYCHAIN_LIVE=1（会往登录钥匙串写一条临时条目）");
            false
        }
    }
}

#[cfg(target_os = "macos")]
fn add(service: &str, account: &str, value: &str) -> bool {
    std::process::Command::new("/usr/bin/security")
        .args([
            "add-generic-password",
            "-s",
            service,
            "-a",
            account,
            "-U",
            "-w",
            value,
        ])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn delete(service: &str, account: &str) {
    let _ = std::process::Command::new("/usr/bin/security")
        .args(["delete-generic-password", "-s", service, "-a", account])
        .output();
}

#[cfg(target_os = "macos")]
#[test]
fn keychain_read_roundtrip_and_error_classification() {
    if !live() {
        return;
    }
    let service = "agenthub-selftest";
    let account = format!(
        "probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    );
    let secret = "SecretProbe123456";

    let ref_ = SecretRef::Keychain {
        service: service.to_string(),
        account: account.clone(),
    };

    // 1) 不存在：必须是「没有」这一类，不能是「读取失败」或「被拒绝」
    let missing = secrets::resolve(&ref_).expect_err("不存在的条目必须报错");
    let msg = missing.to_string();
    assert!(
        msg.contains("没有") && !msg.contains("被拒绝"),
        "缺条目应报「没有」，实际：{msg}"
    );
    println!("实测（不存在）：{msg}");

    // 2) 存在：读回原值
    if !add(service, &account, secret) {
        eprintln!("跳过：本机 security 无法写入自测条目（钥匙串锁定 / 无 GUI 会话）");
        return;
    }
    let got = secrets::resolve(&ref_).expect("条目存在时应能读回");
    assert_eq!(got, secret, "读回的值必须与写入一致");
    println!("实测（存在）：读回 {} 个字符", got.chars().count());

    // 3) 存在但空值：不能当密钥写出去
    if add(service, &account, "") {
        match secrets::resolve(&ref_) {
            Ok(value) => panic!("空值必须报错，实际读到 {value:?}"),
            Err(err) => {
                let msg = err.to_string();
                assert!(msg.contains("内容为空"), "空值文案不对：{msg}");
                println!("实测（空值）：{msg}");
            }
        }
    } else {
        eprintln!("跳过：该 security 版本不接受空值写入");
    }

    delete(service, &account);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn keychain_read_roundtrip_is_macos_only_here() {
    // Windows 侧的等价断言见 scripts/verify-provider-windows.ps1（凭据管理器需要 GUI 会话）
    eprintln!(
        "跳过：本用例只覆盖 macOS 的 security 分支（当前平台 {}）",
        std::env::consts::OS
    );
}
