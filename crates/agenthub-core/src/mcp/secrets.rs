use crate::error::{Error, Result};
use crate::l10n::{Msg, t};
use crate::model::mcp::SecretRef;

/// 在「写入目标配置的那一刻」才解析密钥。
///
/// AgentHub 自己不保存明文：数据库只存 `env:VAR` / `keychain:service/account` 引用。
/// 目标 agent 的配置格式大多不支持变量插值，因此落盘时必须是真值——这是用户
/// 显式选择的结果（`SecretRef::Literal` 需要 `acknowledged = true` 才放行）。
pub fn resolve(secret: &SecretRef) -> Result<String> {
    match secret {
        SecretRef::Env { var } => {
            std::env::var(var).map_err(|_| Error::Other(t!(Msg::ErrSecretEnvUnset, var = var)))
        }
        SecretRef::Keychain { service, account } => read_keychain(service, account),
        SecretRef::Literal {
            value,
            acknowledged,
        } => {
            if *acknowledged {
                Ok(value.clone())
            } else {
                Err(Error::Unsupported(t!(Msg::ErrSecretLiteralUnconfirmed)))
            }
        }
    }
}

/// macOS 走 `security` 命令读取登录钥匙串；Windows 走凭据管理器；
/// 其他平台明确不支持并给出指引。
#[cfg(target_os = "macos")]
fn read_keychain(service: &str, account: &str) -> Result<String> {
    let output = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", service, "-a", account, "-w"])
        .output()
        .map_err(|e| Error::Other(t!(Msg::ErrSecretSecurityCall, error = e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(classify_security_failure(
            service,
            account,
            output.status.code(),
            &stderr,
        ));
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        // 空串会被当成「读到密钥了」，写进配置就是把空值发给上游，报错必须在这里发生
        return Err(Error::Other(format!(
            "钥匙串条目 {service}/{account} 存在但内容为空，请用 \
             `security add-generic-password -s {service} -a {account} -U -w <密钥>` 重写，或改用 env:VAR 引用"
        )));
    }
    Ok(value)
}

/// `security find-generic-password` 的失败分类（纯函数，便于单测）。
///
/// 退出码 44 = `errSecItemNotFound`（2026-11-02 在 macOS 26.6 实测）；「用户取消 /
/// 不允许交互」在 CLI 上不固定映射到某个 code，所以还要看 stderr 文案。
#[cfg(target_os = "macos")]
fn classify_security_failure(
    service: &str,
    account: &str,
    code: Option<i32>,
    stderr: &str,
) -> Error {
    const ERR_SEC_ITEM_NOT_FOUND: i32 = 44;
    let lower = stderr.to_ascii_lowercase();
    if code == Some(ERR_SEC_ITEM_NOT_FOUND) || lower.contains("could not be found") {
        return Error::Other(format!(
            "钥匙串中没有 {service}/{account}（security 退出码 44）；可用 \
             `security add-generic-password -s {service} -a {account} -w <密钥>` 添加，或改用 env:VAR 引用"
        ));
    }
    if lower.contains("interaction is not allowed")
        || lower.contains("user canceled")
        || lower.contains("cancel")
        || lower.contains("denied")
        || lower.contains("authorization")
    {
        return Error::Other(format!(
            "读取钥匙串 {service}/{account} 被拒绝（security 退出码 {code:?}）：请打开「钥匙串访问」搜索 \
             {service} → 右键「显示简介」→「访问控制」里允许 security，或在弹窗里选「允许」，再重试；\
             也可改用 env:VAR 引用"
        ));
    }
    Error::Other(format!(
        "读取钥匙串 {service}/{account} 失败（security 退出码 {code:?}）：{stderr}；\
         可用 `security find-generic-password -s {service} -a {account} -w` 手动排查，或改用 env:VAR 引用"
    ))
}

/// Windows 走凭据管理器（通用凭据）：`target` = service，`user` = account。
///
/// 与 macOS 的 `security` 分支语义一致：只读、不落日志、错误串只带 service/account。
#[cfg(windows)]
fn read_keychain(service: &str, account: &str) -> Result<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
    use windows_sys::Win32::Security::Credentials::{
        CRED_TYPE_DOMAIN_PASSWORD, CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
    };

    let target: Vec<u16> = OsStr::new(service)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut raw: *mut CREDENTIALW = std::ptr::null_mut();
    let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut raw) };
    if ok == 0 || raw.is_null() {
        // `cmdkey /add:<target>`（不带 `/generic`）建出来的其实是「域密码」凭据：
        // 它能被读到，但 Windows 不保存可读密码（blob 为空），所以只用来给一句准确的指引。
        let mut probe: *mut CREDENTIALW = std::ptr::null_mut();
        let domain =
            unsafe { CredReadW(target.as_ptr(), CRED_TYPE_DOMAIN_PASSWORD, 0, &mut probe) };
        if domain != 0 && !probe.is_null() {
            unsafe { CredFree(probe as *const core::ffi::c_void) };
            return Err(Error::Other(keychain_help(
                service,
                account,
                KeychainLookup::DomainPassword,
            )));
        }
        let code = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or_default();
        if code == ERROR_NOT_FOUND as i32 {
            return Err(Error::Other(keychain_help(
                service,
                account,
                KeychainLookup::Missing,
            )));
        }
        return Err(Error::Other(format!(
            "读取系统凭据失败 {service}/{account}（错误码 {code}）；\
             可用 `cmdkey /generic:{service} /user:{account} /pass:<密钥>` 添加后重试，或改用 env:VAR 引用"
        )));
    }

    let cred = unsafe { &*raw };
    let stored_user = if cred.UserName.is_null() {
        String::new()
    } else {
        unsafe { wide_to_string(cred.UserName) }
    };
    let value = if cred.CredentialBlob.is_null() || cred.CredentialBlobSize == 0 {
        String::new()
    } else {
        let bytes = unsafe {
            std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize)
        };
        String::from_utf8_lossy(bytes).trim().to_string()
    };
    unsafe { CredFree(raw as *const core::ffi::c_void) };

    if !stored_user.is_empty() && !account.is_empty() && stored_user != account {
        return Err(Error::Other(t!(
            Msg::ErrSecretUserMismatch,
            service = service,
            stored = stored_user,
            account = account
        )));
    }
    if value.is_empty() {
        return Err(Error::Other(t!(
            Msg::ErrSecretEmptyValue,
            service = service,
            account = account
        )));
    }
    Ok(value)
}

#[cfg(windows)]
unsafe fn wide_to_string(ptr: *const u16) -> String {
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16_lossy(slice)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn read_keychain(service: &str, account: &str) -> Result<String> {
    Err(Error::Unsupported(t!(
        Msg::ErrSecretNoKeychain,
        service = service,
        account = account
    )))
}

/// 凭据查不到时的两种情形：真没有，和有但读不出来。
#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeychainLookup {
    /// 凭据库里没有这个 target
    Missing,
    /// 只有「域密码」类型（`cmdkey /add:` 建的），Windows 不保存可读密码
    DomainPassword,
}

/// 把上面两种情形翻成能照着做的指引（纯函数，跨平台可测）。
#[cfg(any(windows, test))]
fn keychain_help(service: &str, account: &str, lookup: KeychainLookup) -> String {
    match lookup {
        KeychainLookup::Missing => {
            t!(Msg::ErrSecretMissing, service = service, account = account)
        }
        KeychainLookup::DomainPassword => {
            t!(
                Msg::ErrSecretDomainPassword,
                service = service,
                account = account
            )
        }
    }
}

/// 判断值看起来是否像密钥，用于 UI 风险提示与自动降级为 SecretRef。
pub fn looks_secret(value: &str) -> bool {
    let v = value.trim();
    if v.len() < 16 {
        return false;
    }
    let prefixes = [
        "sk-",
        "ghp_",
        "github_pat_",
        "xoxb-",
        "AKIA",
        "ASIA",
        "AIza",
        "Bearer ",
        "eyJ",
    ];
    prefixes.iter().any(|p| v.starts_with(p)) || v.contains("-----BEGIN")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keychain_hint_uses_generic_switch() {
        // 曾经的 bug：提示写的是 `cmdkey /add:`，而 /add 建的是「域密码」凭据，
        // 用完整流程试过——读出来是空的，用户照着提示做必然失败。
        let text = keychain_help("relay", "me", KeychainLookup::Missing);
        assert!(text.contains("cmdkey /generic:relay /user:me /pass:"));
        assert!(!text.contains("cmdkey /add:"), "不能推荐读不出来的 /add:");
        assert!(text.contains("env:VAR"));
    }

    #[test]
    fn domain_password_hint_explains_why_and_how_to_fix() {
        let text = keychain_help("relay", "me", KeychainLookup::DomainPassword);
        assert!(text.contains("域密码"));
        assert!(text.contains("cmdkey /generic:relay /user:me /pass:"));
    }
}

/// macOS `security` 失败分类的单测：三种情形必须给出不同且可操作的文案。
#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::*;

    fn text(err: Error) -> String {
        format!("{err}")
    }

    #[test]
    fn item_not_found_is_reported_as_missing_not_as_denied() {
        let err = classify_security_failure(
            "agenthub",
            "relay",
            Some(44),
            "security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain.",
        );
        let msg = text(err);
        assert!(msg.contains("没有 agenthub/relay"), "实际：{msg}");
        assert!(
            msg.contains("add-generic-password"),
            "要给出添加办法：{msg}"
        );
        assert!(!msg.contains("被拒绝"), "找不到 ≠ 授权被拒：{msg}");
    }

    #[test]
    fn interaction_denied_points_at_keychain_access_panel() {
        let err = classify_security_failure(
            "agenthub",
            "relay",
            Some(1),
            "security: SecKeychainItemCopyContent: User interaction is not allowed.",
        );
        let msg = text(err);
        assert!(msg.contains("被拒绝"), "实际：{msg}");
        assert!(msg.contains("钥匙串访问"), "要指向钥匙串访问面板：{msg}");
        assert!(msg.contains("访问控制"), "要说明在哪里允许：{msg}");
    }

    #[test]
    fn other_failures_keep_stderr_and_manual_command() {
        let err = classify_security_failure(
            "agenthub",
            "relay",
            Some(7),
            "security: some unexpected failure",
        );
        let msg = text(err);
        assert!(
            msg.contains("读取钥匙串 agenthub/relay 失败"),
            "实际：{msg}"
        );
        assert!(
            msg.contains("some unexpected failure"),
            "要带上原始线索：{msg}"
        );
        assert!(
            msg.contains("find-generic-password"),
            "要给出排查命令：{msg}"
        );
    }
}
