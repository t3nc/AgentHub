use serde_json::Value;

use crate::error::{Error, Result};
use crate::l10n::{self, Msg, t};
use crate::model::market::{MarketFile, MarketPage, MarketSkill, MarketSkillDetail, MarketSort};

use super::http::{Http, status_error};
use super::json::{
    absolutize, dig, ms_to_secs, one_line, pick_bool2, pick_str, pick_str_list, pick_str2, pick_u64,
};
use super::{MAX_ARCHIVE_BYTES, MAX_JSON_BYTES, PAGE_SIZE_MAX, encode};

/// SkillHub（api.skillhub.cn）的公开读接口实现。
///
/// 与 ClawHub 的差异都收敛在这个文件里：
/// - 列表/搜索共用一个端点 `/api/skills`（`keyword` / `sortBy` / `order` / `pageSize`），
///   且响应套了一层 `{code, message, data}`；
/// - `/api/v1/*` 反而是裸对象，没有信封；
/// - 文件清单是独立端点 `/files?version=`，不在版本对象里；
/// - 详情不含 SKILL.md 正文（`overviewMd` 实测为空），要另外读一次 `/file`；
/// - 下载与单文件读取都会 302 到对象存储，由网络层跟随。
///
/// 审核信息只吃详情自带的 `securityReports`（`keen` / `sanbu` 两个报告源），
/// 不额外请求 evaluation 接口。
pub struct SkillHub<H: Http> {
    http: H,
    base: String,
}

impl<H: Http> SkillHub<H> {
    pub fn new(http: H, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn get_json(&self, path: &str, what: impl AsRef<str>) -> Result<Value> {
        let what = what.as_ref();
        let resp = self.http.get(&self.url(path), MAX_JSON_BYTES)?;
        if !resp.ok() {
            return Err(status_error(resp.status, what));
        }
        let text = resp.text()?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|err| Error::Other(t!(Msg::ErrMarketParse, what = what, error = err)))?;
        // `/api/skills` 有业务码信封；`/api/v1/*` 没有 code 字段，天然跳过
        if let Some(code) = json.get("code").and_then(Value::as_i64)
            && code != 0
        {
            let message = json
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_else(|| l10n::str_of(Msg::MarketUnknownError));
            return Err(Error::Other(format!("{what}：{message}（code {code}）")));
        }
        Ok(json)
    }

    fn clamp_limit(limit: usize) -> usize {
        // SkillHub 的 pageSize 上限就是 100，超出会被服务端忽略
        limit.clamp(1, PAGE_SIZE_MAX)
    }

    /// 游标 = 下一页页码（`"2"`、`"3"`…）；空 / 非法值一律回第 1 页。
    fn page_of(cursor: Option<&str>) -> usize {
        cursor
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .filter(|page| *page >= 1)
            .unwrap_or(1)
    }

    /// 榜单 / 浏览排序：SkillHub 用 `sortBy` + `order`，取值和 ClawHub 完全不同。
    fn sort_params(sort: MarketSort) -> (&'static str, &'static str) {
        match sort {
            MarketSort::Recommended => ("score", "desc"),
            MarketSort::Downloads => ("downloads", "desc"),
            MarketSort::Updated => ("updated_at", "desc"),
        }
    }

    /// 最新版本号（版本列表已按时间倒序）。
    fn latest_version(&self, slug: &str) -> Result<Option<String>> {
        let json = self.get_json(
            &format!("/api/v1/skills/{}/versions", encode(slug.trim())),
            t!(Msg::MarketOpVersions),
        )?;
        for key in ["versions", "items", "results"] {
            let Some(list) = json.get(key).and_then(Value::as_array) else {
                continue;
            };
            for item in list {
                if let Some(version) = item.get("version").and_then(Value::as_str) {
                    return Ok(Some(version.trim().to_string()));
                }
            }
        }
        Ok(None)
    }

    fn list_path(sort: MarketSort, page: usize, limit: usize) -> String {
        let (sort_by, order) = Self::sort_params(sort);
        format!(
            "/api/skills?sortBy={sort_by}&order={order}&pageSize={}&page={page}",
            Self::clamp_limit(limit)
        )
    }
}

impl<H: Http> super::Registry for SkillHub<H> {
    fn base_url(&self) -> &str {
        &self.base
    }

    fn search_page(&self, query: &str, cursor: Option<&str>, limit: usize) -> Result<MarketPage> {
        let q = query.trim();
        if q.is_empty() {
            return Err(Error::Other(t!(Msg::ErrSearchQueryEmpty)));
        }
        let limit = Self::clamp_limit(limit);
        let page = Self::page_of(cursor);
        // 搜索走同一个列表端点，按综合评分排序（SkillHub 没有独立 search 接口）
        let path = format!(
            "/api/skills?keyword={}&sortBy=score&order=desc&pageSize={limit}&page={page}",
            encode(q)
        );
        let json = self.get_json(&path, t!(Msg::MarketOpSearch))?;
        Ok(to_page(&json, page, limit))
    }

    fn list_page(
        &self,
        sort: MarketSort,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<MarketPage> {
        let limit = Self::clamp_limit(limit);
        let page = Self::page_of(cursor);
        let json = self.get_json(&Self::list_path(sort, page, limit), t!(Msg::MarketOpList))?;
        Ok(to_page(&json, page, limit))
    }

    fn detail(&self, slug: &str) -> Result<MarketSkillDetail> {
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(Error::Other(t!(Msg::ErrSkillSlugEmpty)));
        }
        let root = self.get_json(
            &format!("/api/v1/skills/{}", encode(slug)),
            t!(Msg::MarketOpDetail),
        )?;
        let primary = root.get("skill").unwrap_or(&root);

        let Some(skill) = parse_skill(primary, &root) else {
            return Err(Error::NotFound(t!(Msg::ErrMarketSkillMissing, slug = slug)));
        };

        let version = skill.version.clone();
        let fallback_summary = pick_str2(
            primary,
            &root,
            &[
                &["summary_zh"],
                &["summary"],
                &["description_zh"],
                &["description"],
            ],
        )
        .unwrap_or_default();

        // 详情接口不返回 SKILL.md 正文：补读一次，读不到就退回摘要，绝不把空正文给 UI
        let description = match self.file_text(slug, "SKILL.md", version.as_deref()) {
            Ok(text) if !text.trim().is_empty() => text,
            Ok(_) => fallback_summary.clone(),
            Err(err) => {
                eprintln!(
                    "{}",
                    t!(Msg::LogMarketSkillMdFailed, slug = slug, error = err)
                );
                fallback_summary.clone()
            }
        };

        let files = self.files(slug, version.as_deref()).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                t!(Msg::LogMarketFilesFailed, slug = slug, error = err)
            );
            Vec::new()
        });

        let audit = Audit::of(&root);
        Ok(MarketSkillDetail {
            description,
            license: pick_str2(
                &root,
                primary,
                &[&["latestVersion", "license"], &["license"]],
            ),
            changelog: pick_str(&root, &[&["latestVersion", "changelog"]]),
            moderation_summary: audit.summary(),
            reason_codes: audit.codes.clone(),
            homepage: pick_str2(&root, primary, &[&["homepage"], &["sourceUrl"]])
                .map(|raw| absolutize(&self.base, &raw)),
            files,
            skill,
        })
    }

    fn files(&self, slug: &str, version: Option<&str>) -> Result<Vec<MarketFile>> {
        let version = match version.map(str::trim).filter(|v| !v.is_empty()) {
            Some(v) => v.to_string(),
            None => match self.latest_version(slug)? {
                Some(latest) => latest,
                None => return Ok(Vec::new()),
            },
        };
        let path = format!(
            "/api/v1/skills/{}/files?version={}",
            encode(slug.trim()),
            encode(&version)
        );
        let json = self.get_json(&path, t!(Msg::MarketOpFiles))?;
        Ok(parse_files(&json))
    }

    fn archive(&self, slug: &str, version: Option<&str>) -> Result<Vec<u8>> {
        let mut path = format!("/api/v1/download?slug={}", encode(slug.trim()));
        if let Some(v) = version.map(str::trim).filter(|v| !v.is_empty()) {
            path.push_str(&format!("&version={}", encode(v)));
        }
        let resp = self
            .http
            .get(&self.url(&path), MAX_ARCHIVE_BYTES as usize)?;
        if !resp.ok() {
            // 4xx（除鉴权/不存在/限流）视为「该技能不提供 ZIP」，交给逐文件降级
            return match resp.status {
                400..=499
                    if resp.status != 401
                        && resp.status != 403
                        && resp.status != 404
                        && resp.status != 429 =>
                {
                    Err(Error::Unsupported(t!(
                        Msg::ErrMarketZipUnavailableStatus,
                        status = resp.status
                    )))
                }
                _ => Err(status_error(resp.status, &t!(Msg::MarketOpDownload))),
            };
        }
        if !resp.looks_like_zip() {
            // 302 没跟到对象存储时会拿到 HTML/JSON；同样降级，而不是把垃圾当 zip 解
            return Err(Error::Unsupported(t!(Msg::ErrMarketZipUnavailable)));
        }
        Ok(resp.body)
    }

    fn file_text(&self, slug: &str, path: &str, version: Option<&str>) -> Result<String> {
        let mut url = format!(
            "/api/v1/skills/{}/file?path={}",
            encode(slug.trim()),
            encode(path)
        );
        if let Some(v) = version.map(str::trim).filter(|v| !v.is_empty()) {
            url.push_str(&format!("&version={}", encode(v)));
        }
        let resp = self.http.get(&self.url(&url), MAX_JSON_BYTES)?;
        if !resp.ok() {
            return Err(status_error(
                resp.status,
                &t!(Msg::MarketOpReadFile, path = path),
            ));
        }
        let text = resp.text()?;
        // 302 未跟随 / 对象存储报错时可能回一个网页，别把它当文件内容
        let head = text.trim_start().to_ascii_lowercase();
        if head.starts_with("<!doctype") || head.starts_with("<html") {
            return Err(Error::Unsupported(t!(
                Msg::ErrMarketReturnedPage,
                path = path
            )));
        }
        Ok(text)
    }
}

/// 拼一页结果：`has_more` 优先信 `data.total`，缺失时退回「拿满了就当作还有下一批」。
fn to_page(json: &Value, page: usize, limit: usize) -> MarketPage {
    let items = parse_list(json);
    let total = dig(json, &["data", "total"])
        .or_else(|| dig(json, &["total"]))
        .and_then(Value::as_u64);
    let has_more = !items.is_empty()
        && match total {
            Some(total) => (page as u64).saturating_mul(limit as u64) < total,
            None => items.len() >= limit,
        };
    MarketPage {
        items,
        cursor: if has_more {
            Some((page + 1).to_string())
        } else {
            None
        },
        has_more,
    }
}

/// 解析 `/api/skills` 的 `{code, data:{skills:[]}}` 信封。
///
/// 保持接口给的顺序（榜单/搜索结果本身有排序语义），只去重，不重排。
fn parse_list(json: &Value) -> Vec<MarketSkill> {
    let items = dig(json, &["data", "skills"])
        .or_else(|| dig(json, &["skills"]))
        .or_else(|| dig(json, &["data", "items"]))
        .and_then(Value::as_array);
    let Some(items) = items else {
        return Vec::new();
    };
    let mut out: Vec<MarketSkill> = items
        .iter()
        .filter_map(|item| parse_skill(item, item))
        .collect();
    let mut seen = std::collections::HashSet::new();
    out.retain(|skill| seen.insert(skill.slug.clone()));
    out
}

fn parse_skill(primary: &Value, root: &Value) -> Option<MarketSkill> {
    let slug = pick_str2(primary, root, &[&["slug"], &["namespace", "publicSlug"]])?;

    let display_name = pick_str2(
        primary,
        root,
        &[&["displayName"], &["name"], &["skill", "displayName"]],
    )
    .unwrap_or_else(|| slug.clone());

    // 中文描述优先（SkillHub 会为海外技能补 `description_zh` / `summary_zh`）
    let raw_summary = pick_str2(
        primary,
        root,
        &[
            &["summary_zh"],
            &["summary"],
            &["description_zh"],
            &["description"],
            &["skill", "summary_zh"],
            &["skill", "summary"],
        ],
    )
    .unwrap_or_default();

    let audit = Audit::of(root);

    Some(MarketSkill {
        slug,
        display_name,
        summary: one_line(&raw_summary, 320),
        owner: pick_str2(
            primary,
            root,
            &[
                &["owner", "handle"],
                &["ownerName"],
                &["namespace", "handle"],
                &["publisher", "name"],
            ],
        )
        .unwrap_or_else(|| "unknown".into()),
        version: pick_str2(
            primary,
            root,
            &[
                &["latestVersion", "version"],
                &["version"],
                &["tags", "latest"],
            ],
        ),
        downloads: pick_u64(primary, &[&["stats", "downloads"], &["downloads"]])
            .or_else(|| pick_u64(root, &[&["stats", "downloads"], &["downloads"]]))
            .unwrap_or(0),
        installs: pick_u64(primary, &[&["stats", "installs"], &["installs"]])
            .or_else(|| pick_u64(root, &[&["stats", "installs"], &["installs"]]))
            .unwrap_or(0),
        stars: pick_u64(primary, &[&["stats", "stars"], &["stars"]])
            .or_else(|| pick_u64(root, &[&["stats", "stars"], &["stars"]]))
            .unwrap_or(0),
        updated_at: pick_u64(primary, &[&["updatedAt"], &["updated_at"]])
            .or_else(|| pick_u64(root, &[&["updatedAt"], &["updated_at"]]))
            .map(ms_to_secs),
        topics: topics_of(primary),
        suspicious: audit.suspicious,
        malware_blocked: audit.malware_blocked,
        verdict: audit.verdict.clone(),
        // `source` 是开放枚举（community / enterprise / official…），不写死 match
        official: pick_str2(primary, root, &[&["source"]])
            .map(|source| source.eq_ignore_ascii_case("official"))
            .unwrap_or(false)
            || pick_bool2(primary, root, &[&["publisher", "verified"]]).unwrap_or(false),
    })
}

/// 二级类目优先，没有就退回列表项的 `tags`（详情里的 `tags` 是对象，不适用）。
fn topics_of(value: &Value) -> Vec<String> {
    if let Some(list) = dig(value, &["subCategories"]).and_then(Value::as_array) {
        let names: Vec<String> = list
            .iter()
            .filter_map(|item| pick_str(item, &[&["name"], &["key"]]))
            .collect();
        if !names.is_empty() {
            return names;
        }
    }
    pick_str_list(value, &["tags"])
}

/// 文件清单：`{count, version, files:[{path, size, sha256}]}`（没有 contentType）。
fn parse_files(json: &Value) -> Vec<MarketFile> {
    let Some(list) = dig(json, &["files"])
        .or_else(|| dig(json, &["data", "files"]))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|file| {
            let path = file.get("path").and_then(Value::as_str)?.trim().to_string();
            if path.is_empty() {
                return None;
            }
            Some(MarketFile {
                path,
                size: file.get("size").and_then(Value::as_u64).unwrap_or(0),
                content_type: None,
            })
        })
        .collect()
}

/// 安全审核汇总（来自详情自带的 `securityReports`）。
#[derive(Debug, Default, Clone)]
struct Audit {
    suspicious: bool,
    malware_blocked: bool,
    verdict: Option<String>,
    texts: Vec<String>,
    codes: Vec<String>,
}

impl Audit {
    fn of(root: &Value) -> Self {
        let Some(reports) = dig(root, &["securityReports"])
            .or_else(|| dig(root, &["skill", "securityReports"]))
            .and_then(Value::as_object)
        else {
            return Self::default();
        };

        let mut audit = Self::default();
        for (source, report) in reports {
            let Some(status) = pick_str(report, &[&["status"]]) else {
                continue;
            };
            let status = status.to_ascii_lowercase();
            audit.codes.push(format!("{source}.{status}"));
            if let Some(text) = pick_str(report, &[&["statusText"]]) {
                audit.texts.push(format!("{source}：{text}"));
            }
            match classify(&status) {
                Class::Safe => {}
                Class::Malicious => audit.malware_blocked = true,
                // 认不出来的状态一律按可疑处理：宁可多提醒，也不静默放行
                Class::Suspicious => audit.suspicious = true,
            }
        }

        audit.verdict = if audit.malware_blocked {
            Some("malicious".into())
        } else if audit.suspicious {
            Some("suspicious".into())
        } else if audit.codes.is_empty() {
            None
        } else {
            Some("clean".into())
        };
        audit
    }

    fn summary(&self) -> Option<String> {
        if self.texts.is_empty() {
            None
        } else {
            Some(self.texts.join(" · "))
        }
    }
}

enum Class {
    Safe,
    Malicious,
    Suspicious,
}

fn classify(status: &str) -> Class {
    const SAFE: [&str; 6] = ["benign", "safe", "clean", "pass", "passed", "ok"];
    if SAFE.contains(&status) {
        Class::Safe
    } else if ["malicious", "blocked", "unsafe", "danger", "virus"]
        .iter()
        .any(|needle| status.contains(needle))
    {
        Class::Malicious
    } else {
        Class::Suspicious
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classify_covers_safe_malicious_and_unknown() {
        assert!(matches!(classify("benign"), Class::Safe));
        assert!(matches!(classify("malicious"), Class::Malicious));
        assert!(matches!(classify("blocked"), Class::Malicious));
        assert!(matches!(classify("pending"), Class::Suspicious));
    }

    #[test]
    fn audit_maps_reports_to_flags() {
        let root = json!({
            "securityReports": {
                "keen": { "status": "benign", "statusText": "安全，无风险" },
                "sanbu": { "status": "malicious", "statusText": "发现风险" }
            }
        });
        let audit = Audit::of(&root);
        assert!(audit.malware_blocked);
        assert_eq!(audit.verdict.as_deref(), Some("malicious"));
        assert_eq!(audit.codes, vec!["keen.benign", "sanbu.malicious"]);
        assert_eq!(
            audit.summary().as_deref(),
            Some("keen：安全，无风险 · sanbu：发现风险")
        );
    }

    #[test]
    fn audit_without_reports_stays_neutral() {
        let audit = Audit::of(&json!({ "slug": "x" }));
        assert!(!audit.suspicious && !audit.malware_blocked);
        assert!(audit.verdict.is_none());
        assert!(audit.summary().is_none());
    }
}
