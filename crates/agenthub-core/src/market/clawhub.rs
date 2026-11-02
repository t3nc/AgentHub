use serde_json::Value;

use crate::error::{Error, Result};
use crate::l10n::{Msg, t};
use crate::model::market::{MarketFile, MarketPage, MarketSkill, MarketSkillDetail, MarketSort};

use super::http::{Http, HttpResponse, status_error};
use super::json::{
    absolutize, dig, ms_to_secs, one_line, pick_bool2, pick_str, pick_str_list,
    pick_str_list_multi, pick_str2, pick_u64,
};
use super::{MAX_ARCHIVE_BYTES, MAX_JSON_BYTES, PAGE_SIZE_MAX, encode};

/// 搜索用 offset 模拟分页时的累计上限：再多就该换关键词了。
const SEARCH_OFFSET_MAX: usize = 200;

/// ClawHub（clawhub.ai）的公开读接口实现。
///
/// 解析刻意做成「宽容」的：同一份技能在不同端点下的字段位置并不一致
/// （列表在 `stats.downloads`，搜索把它铺在顶层，详情又包在 `skill` 里），
/// 所以统一走 `pick_*` 按候选路径取值，缺字段就退默认值，不让一次 API 微调
/// 把整页打挂。
pub struct ClawHub<H: Http> {
    http: H,
    base: String,
}

impl<H: Http> ClawHub<H> {
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
        serde_json::from_str(&text)
            .map_err(|err| Error::Other(t!(Msg::ErrMarketParse, what = what, error = err)))
    }

    fn clamp_limit(limit: usize) -> usize {
        limit.clamp(1, PAGE_SIZE_MAX)
    }

    /// 搜索游标 = `offset:<已取条数>`。
    ///
    /// ClawHub 的 `/api/v1/search` 没有 `nextCursor`（实测只有 `results`），
    /// 只能用「请求更大的 limit 再切掉前面已取的部分」来模拟翻页：
    /// 第 n 批的代价是重传前面所有条目，所以累计上限压到 200 条。
    fn offset_of(cursor: Option<&str>) -> usize {
        cursor
            .and_then(|raw| raw.trim().strip_prefix("offset:"))
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(0)
            .min(SEARCH_OFFSET_MAX)
    }

    /// 从版本列表里取最新版本号（列表已按时间倒序）。
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
}

impl<H: Http> super::Registry for ClawHub<H> {
    fn base_url(&self) -> &str {
        &self.base
    }

    fn search_page(&self, query: &str, cursor: Option<&str>, limit: usize) -> Result<MarketPage> {
        let q = query.trim();
        if q.is_empty() {
            return Err(Error::Other(t!(Msg::ErrSearchQueryEmpty)));
        }
        let limit = Self::clamp_limit(limit);
        let offset = Self::offset_of(cursor);
        // 已经到累计上限：别再发一次注定白跑的大请求
        if offset >= SEARCH_OFFSET_MAX {
            return Ok(MarketPage {
                items: Vec::new(),
                cursor: None,
                has_more: false,
            });
        }
        // 没有游标可用：把「已取条数」折进 limit，请求完再切掉前 offset 条
        let want = offset.saturating_add(limit).min(SEARCH_OFFSET_MAX);
        let path = format!("/api/v1/search?q={}&limit={want}", encode(q));
        let json = self.get_json(&path, t!(Msg::MarketOpSearch))?;
        let all = collect_skills(&json, &["results", "items", "skills"]);
        let fetched = all.len();
        let items: Vec<MarketSkill> = all.into_iter().skip(offset).take(limit).collect();
        let next = offset + items.len();
        // 拿满这一批且服务端也确实给到了 want 条，才认为可能还有下一页
        let has_more = items.len() == limit && fetched >= want && next < SEARCH_OFFSET_MAX;
        Ok(MarketPage {
            items,
            cursor: has_more.then(|| format!("offset:{next}")),
            has_more,
        })
    }

    fn list_page(
        &self,
        sort: MarketSort,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<MarketPage> {
        let mut path = format!(
            "/api/v1/skills?sort={}&limit={}",
            sort.slug(),
            Self::clamp_limit(limit)
        );
        if let Some(cursor) = cursor.map(str::trim).filter(|raw| !raw.is_empty()) {
            // nextCursor 是服务端给的不透明串（内含 JSON），必须原样编码回传
            path.push_str(&format!("&cursor={}", encode(cursor)));
        }
        let json = self.get_json(&path, t!(Msg::MarketOpList))?;
        let items = collect_skills(&json, &["items", "results", "skills"]);
        let next = pick_str(&json, &[&["nextCursor"], &["next_cursor"]]);
        let has_more = next.is_some() && !items.is_empty();
        Ok(MarketPage {
            items,
            cursor: next.filter(|_| has_more),
            has_more,
        })
    }

    fn detail(&self, slug: &str) -> Result<MarketSkillDetail> {
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(Error::Other(t!(Msg::ErrSkillSlugEmpty)));
        }
        let path = format!("/api/v1/skills/{}", encode(slug));
        let root = self.get_json(&path, t!(Msg::MarketOpDetail))?;
        let primary = root.get("skill").unwrap_or(&root);

        let Some(skill) = parse_skill(primary, &root) else {
            return Err(Error::NotFound(t!(Msg::ErrMarketSkillMissing, slug = slug)));
        };

        let description = pick_str2(primary, &root, &[&["description"], &["summary"]])
            .unwrap_or_else(|| skill.summary.clone());
        let homepage = pick_str2(&root, primary, &[&["links", "canonical"], &["homepage"]])
            .map(|raw| absolutize(&self.base, &raw));

        Ok(MarketSkillDetail {
            description,
            license: pick_str2(
                &root,
                primary,
                &[&["latestVersion", "license"], &["license"]],
            ),
            changelog: pick_str2(&root, primary, &[&["latestVersion", "changelog"]]),
            moderation_summary: pick_str2(
                &root,
                primary,
                &[&["moderation", "summary"], &["trust", "clawHubVerdict"]],
            ),
            reason_codes: pick_str_list(&root, &["moderation", "reasonCodes"]),
            homepage,
            files: parse_files(&root, primary),
            skill,
        })
    }

    fn files(&self, slug: &str, version: Option<&str>) -> Result<Vec<MarketFile>> {
        let version = match version.map(str::trim).filter(|v| !v.is_empty()) {
            Some(v) => v.to_string(),
            None => {
                // 详情接口有时会顺带给出 files（省一次请求）；没有再查版本列表拿最新号
                let from_detail = self.detail(slug)?.files;
                if !from_detail.is_empty() {
                    return Ok(from_detail);
                }
                match self.latest_version(slug)? {
                    Some(latest) => latest,
                    None => return Ok(Vec::new()),
                }
            }
        };
        let path = format!(
            "/api/v1/skills/{}/versions/{}",
            encode(slug.trim()),
            encode(&version)
        );
        let json = self.get_json(&path, t!(Msg::MarketOpFiles))?;
        Ok(parse_files(&json, &Value::Null))
    }

    fn archive(&self, slug: &str, version: Option<&str>) -> Result<Vec<u8>> {
        let mut path = format!("/api/v1/download?slug={}", encode(slug.trim()));
        if let Some(v) = version.map(str::trim).filter(|v| !v.is_empty()) {
            path.push_str(&format!("&version={}", encode(v)));
        }
        let resp: HttpResponse = self
            .http
            .get(&self.url(&path), MAX_ARCHIVE_BYTES as usize)?;
        if !resp.ok() {
            // 409 / 415 这类「该技能不提供 ZIP」的情况交给逐文件降级；
            // 限流 / 鉴权 / 不存在才是真错误
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
            // 源可能回一个「去 GitHub 拉」的 JSON 交接单；交给安装层走逐文件降级
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
        resp.text()
    }
}

/// 搜索结果 → 技能列表，自动跳过没有 slug 的脏数据。
///
/// **保持服务端顺序**：分页「加载更多」是把新一页追加到列表尾部，
/// 这里若按 slug 重排，追加进去的顺序就跟榜单排序对不上了。
/// 只按 slug 去重（保留首次出现），防同一批里出现重复项。
fn collect_skills(json: &Value, keys: &[&str]) -> Vec<MarketSkill> {
    for key in keys {
        if let Some(items) = json.get(*key).and_then(Value::as_array) {
            let mut out: Vec<MarketSkill> = items
                .iter()
                .filter_map(|item| parse_skill(item, item))
                .collect();
            let mut seen = std::collections::HashSet::new();
            out.retain(|skill| seen.insert(skill.slug.clone()));
            return out;
        }
    }
    Vec::new()
}

fn parse_skill(primary: &Value, root: &Value) -> Option<MarketSkill> {
    let slug = pick_str2(primary, root, &[&["slug"], &["native", "skill", "slug"]])?;

    let display_name = pick_str2(
        primary,
        root,
        &[
            &["displayName"],
            &["native", "skill", "displayName"],
            &["skill", "displayName"],
        ],
    )
    .unwrap_or_else(|| slug.clone());

    let raw_summary = pick_str2(
        primary,
        root,
        &[
            &["summary"],
            &["description"],
            &["native", "skill", "summary"],
            &["skill", "summary"],
        ],
    )
    .unwrap_or_default();

    let verdict = pick_str2(
        primary,
        root,
        &[&["moderation", "verdict"], &["trust", "clawHubVerdict"]],
    );
    let suspicious = pick_bool2(
        primary,
        root,
        &[
            &["moderation", "isSuspicious"],
            &["native", "skill", "isSuspicious"],
        ],
    )
    .unwrap_or(false)
        || verdict.as_deref() == Some("suspicious");
    let malware_blocked = pick_bool2(
        primary,
        root,
        &[
            &["moderation", "isMalwareBlocked"],
            &["native", "skill", "isMalwareBlocked"],
        ],
    )
    .unwrap_or(false)
        || matches!(verdict.as_deref(), Some("malicious") | Some("blocked"));

    Some(MarketSkill {
        slug,
        display_name,
        summary: one_line(&raw_summary, 320),
        owner: pick_str2(
            primary,
            root,
            &[
                &["ownerHandle"],
                &["owner", "handle"],
                &["publisher", "handle"],
                &["native", "ownerHandle"],
            ],
        )
        .unwrap_or_else(|| "unknown".into()),
        version: pick_str2(
            primary,
            root,
            &[
                &["latestVersion", "version"],
                &["version", "version"],
                &["native", "version", "version"],
                &["tags", "latest"],
            ],
        ),
        downloads: pick_u64(
            primary,
            &[
                &["downloads"],
                &["stats", "downloads"],
                &["native", "skill", "stats", "downloads"],
            ],
        )
        .or_else(|| pick_u64(root, &[&["downloads"], &["stats", "downloads"]]))
        .unwrap_or(0),
        installs: pick_u64(
            primary,
            &[
                &["installs"],
                &["stats", "installs"],
                &["native", "skill", "stats", "installs"],
            ],
        )
        .or_else(|| pick_u64(root, &[&["installs"], &["stats", "installs"]]))
        .unwrap_or(0),
        stars: pick_u64(
            primary,
            &[
                &["stars"],
                &["stats", "stars"],
                &["native", "skill", "stats", "stars"],
            ],
        )
        .or_else(|| pick_u64(root, &[&["stars"], &["stats", "stars"]]))
        .unwrap_or(0),
        updated_at: pick_u64(primary, &[&["updatedAt"]])
            .or_else(|| pick_u64(root, &[&["updatedAt"]]))
            .map(ms_to_secs),
        topics: pick_str_list_multi(
            &[primary, root],
            &[&["topics"], &["native", "skill", "topics"], &["categories"]],
        ),
        suspicious,
        malware_blocked,
        verdict,
        official: pick_bool2(
            primary,
            root,
            &[&["official"], &["native", "owner", "official"]],
        )
        .unwrap_or(false),
    })
}

/// 文件清单的位置随端点变化：详情在顶层（若有），版本接口在 `version.files`。
fn parse_files(primary: &Value, root: &Value) -> Vec<MarketFile> {
    files_of(primary)
        .or_else(|| files_of(root))
        .unwrap_or_default()
}

fn files_of(value: &Value) -> Option<Vec<MarketFile>> {
    const CANDIDATES: &[&[&str]] = &[
        &["files"],
        &["version", "files"],
        &["latestVersion", "files"],
        &["skill", "files"],
    ];
    for path in CANDIDATES {
        let Some(list) = dig(value, path).and_then(Value::as_array) else {
            continue;
        };
        let files: Vec<MarketFile> = list
            .iter()
            .filter_map(|f| {
                let path = f.get("path").and_then(Value::as_str)?.trim().to_string();
                if path.is_empty() {
                    return None;
                }
                Some(MarketFile {
                    path,
                    size: f.get("size").and_then(Value::as_u64).unwrap_or(0),
                    content_type: f
                        .get("contentType")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect();
        if !files.is_empty() {
            return Some(files);
        }
    }
    None
}
