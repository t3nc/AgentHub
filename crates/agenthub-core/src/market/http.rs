use std::sync::Arc;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::l10n::{Msg, t};

/// 一次 HTTP 响应的最小形态：只要状态码、内容类型和原始字节，
/// 解析（JSON / ZIP）交给上层，这样测试可以完全绕开网络。
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(&self) -> Result<String> {
        String::from_utf8(self.body.clone()).map_err(|_| Error::Other(t!(Msg::ErrMarketUtf8)))
    }

    /// 内容类型是否算 ZIP。有些 CDN 会回 `application/octet-stream`，所以按魔数兜底。
    pub fn looks_like_zip(&self) -> bool {
        let ct = self.content_type.to_ascii_lowercase();
        ct.contains("zip") || self.body.starts_with(b"PK\x03\x04")
    }
}

/// 网络层抽象。
///
/// 存在的唯一理由是**可测**：单测注入 `FakeHttp` 就能离线验证解析与安装计划，
/// 不必让 `cargo test` 联网。将来换 HTTP 客户端也只改一个实现。
pub trait Http: Send + Sync {
    /// `max_bytes` 是硬上限，超了直接报错（防止恶意源把内存撑爆）。
    fn get(&self, url: &str, max_bytes: usize) -> Result<HttpResponse>;

    /// 带自定义请求头的 GET。
    ///
    /// 供应商探测模型列表需要 `x-api-key` 或 `Authorization`，而市场请求不需要。
    /// 默认实现忽略 headers 直接走 `get`：测试替身不必都为鉴权头写一遍。
    fn get_with(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<HttpResponse> {
        self.get(url, max_bytes)
    }
}

/// 生产实现：ureq + rustls（纯 Rust TLS，不依赖系统 OpenSSL）。
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            // 整个请求（含读 body）20s 封顶；连接阶段更短，避免源不可达时界面像卡死
            .timeout_global(Some(Duration::from_secs(20)))
            .timeout_connect(Some(Duration::from_secs(6)))
            // 状态码自己判：429 / 404 需要不同文案
            .http_status_as_error(false)
            // SkillHub 的下载与单文件读取都是 302 跳对象存储，必须跟到底
            .max_redirects(10)
            .user_agent(concat!(
                "AgentHub/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/agenthub)"
            ))
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for UreqHttp {
    fn get(&self, url: &str, max_bytes: usize) -> Result<HttpResponse> {
        self.request(
            url,
            &[],
            "application/json, application/zip;q=0.9, */*;q=0.8",
            max_bytes,
        )
    }

    fn get_with(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<HttpResponse> {
        self.request(url, headers, "application/json", max_bytes)
    }
}

impl UreqHttp {
    /// 状态码 / 长度上限 / 错误翻译只写一遍。
    ///
    /// ureq 3 的 `Request` / `Response` 都是泛型且不公开，所以构建与发送都留在这里，
    /// 别让它们出现在函数签名上。
    fn request(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        accept: &str,
        max_bytes: usize,
    ) -> Result<HttpResponse> {
        let mut request = self.agent.get(url).header("Accept", accept);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let mut resp = request.call().map_err(|err| map_ureq_error(url, err))?;

        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // 非 2xx 不读 body：错误页正文对用户没有价值，也不该进日志
        if !(200..300).contains(&status) {
            return Ok(HttpResponse {
                status,
                content_type,
                body: Vec::new(),
            });
        }

        let body = resp
            .body_mut()
            .with_config()
            .limit(max_bytes as u64)
            .read_to_vec()
            .map_err(|err| map_ureq_error(url, err))?;

        Ok(HttpResponse {
            status,
            content_type,
            body,
        })
    }
}

/// 把网络错误翻译成人能看懂的一句话。
///
/// 不把 URL 之外的任何东西拼进 UI 文案；URL 只进 `eprintln!` 便于排查。
fn map_ureq_error(url: &str, err: ureq::Error) -> Error {
    eprintln!(
        "{}",
        t!(Msg::LogMarketRequestFailed, url = url, error = err)
    );
    match err {
        ureq::Error::Timeout(_) => Error::Other(t!(Msg::ErrMarketTimeout)),
        ureq::Error::HostNotFound => Error::Other(t!(Msg::ErrMarketHostNotFound)),
        ureq::Error::ConnectionFailed => Error::Other(t!(Msg::ErrMarketConnectionFailed)),
        ureq::Error::Tls(_) => Error::Other(t!(Msg::ErrMarketTls)),
        ureq::Error::BodyExceedsLimit(limit) => {
            Error::Other(t!(Msg::ErrMarketBodyTooLarge, limit = limit))
        }
        // 这里只描述网络层故障：同一个 `Http` 既服务技能市场，也服务供应商模型探测，
        // 写死「技能市场」会让后者的报错莫名其妙。
        other => Error::Other(t!(Msg::ErrNetworkRequest, error = other)),
    }
}

/// 测试/离线实现：按「URL 子串 → 响应」的规则表返回预置数据。
///
/// 只放在 `#[cfg(test)]` 之外会让正常构建多一份死代码，因此这里用 feature-free 的
/// 简单结构，但**只由测试与 `cargo test` 使用**（生产路径不会构造它）。
pub struct FakeHttp {
    rules: Vec<(String, HttpResponse)>,
}

impl FakeHttp {
    pub fn new(rules: Vec<(&str, HttpResponse)>) -> Self {
        Self {
            rules: rules
                .into_iter()
                .map(|(needle, resp)| (needle.to_string(), resp))
                .collect(),
        }
    }

    pub fn json(body: impl Into<String>) -> HttpResponse {
        HttpResponse {
            status: 200,
            content_type: "application/json".into(),
            body: body.into().into_bytes(),
        }
    }

    pub fn zip(body: Vec<u8>) -> HttpResponse {
        HttpResponse {
            status: 200,
            content_type: "application/zip".into(),
            body,
        }
    }

    pub fn status(code: u16) -> HttpResponse {
        HttpResponse {
            status: code,
            content_type: "text/plain".into(),
            body: Vec::new(),
        }
    }
}

impl Http for FakeHttp {
    fn get(&self, url: &str, max_bytes: usize) -> Result<HttpResponse> {
        for (needle, resp) in &self.rules {
            if url.contains(needle.as_str()) {
                if resp.body.len() > max_bytes {
                    return Err(Error::Other("响应超过上限".into()));
                }
                return Ok(resp.clone());
            }
        }
        Err(Error::NotFound(format!("FakeHttp 没有为 {url} 配置响应")))
    }
}

/// 共享引用也是一种 `Http`，省掉计划层到处写 `&dyn Http` 的噪音。
impl<T: Http + ?Sized> Http for Arc<T> {
    fn get(&self, url: &str, max_bytes: usize) -> Result<HttpResponse> {
        (**self).get(url, max_bytes)
    }
}

/// 非 2xx 统一映射：文案固定，避免把服务端 HTML 错误页漏进 UI。
pub fn status_error(status: u16, what: &str) -> Error {
    match status {
        401 | 403 => Error::Other(t!(Msg::ErrMarketStatusAuth, what = what)),
        404 => Error::NotFound(t!(Msg::ErrMarketStatusMissing, what = what)),
        // ClawHub 对「同名 slug 不唯一」的技能回 409（文档未描述），此时无法唯一定位
        409 => Error::Conflict(t!(Msg::ErrMarketStatusConflict, what = what)),
        429 => Error::Other(t!(Msg::ErrMarketStatusRateLimit, what = what)),
        code if code >= 500 => {
            Error::Other(t!(Msg::ErrMarketStatusServer, what = what, code = code))
        }
        code => Error::Other(t!(Msg::ErrMarketStatusFailed, what = what, code = code)),
    }
}
