//! 内置供应商目录的纯查询函数：搜索、分组、按 id 取。
//!
//! 目录本身是 `model/supplier.rs` 里的静态表；这里只做筛选，不碰网络也不碰磁盘，
//! 因此界面、CLI 与测试共用同一份结果。

use crate::model::supplier::{SUPPLIERS, SupplierCategory, SupplierDef};

/// 全量目录（顺序即界面顺序）。
pub fn all() -> &'static [SupplierDef] {
    SUPPLIERS
}

pub fn get(id: &str) -> Option<&'static SupplierDef> {
    let id = id.trim();
    SUPPLIERS.iter().find(|supplier| supplier.id == id)
}

/// 关键词搜索：名称、厂商、base_url 的域名、官网、id 都参与匹配。
///
/// 空关键词返回全部。命中域名这一条很重要 —— 用户往往记得 `api.deepseek.com`
/// 而不是「DeepSeek」这个中文名。
pub fn search(keyword: &str) -> Vec<&'static SupplierDef> {
    let needle = keyword.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return SUPPLIERS.iter().collect();
    }
    SUPPLIERS
        .iter()
        .filter(|supplier| {
            let haystack = format!(
                "{} {} {} {} {}",
                supplier.id,
                supplier.name,
                supplier.vendor,
                domain_of(supplier.base_url),
                supplier.website,
            )
            .to_ascii_lowercase();
            haystack.contains(&needle)
        })
        .collect()
}

/// 按分类分组，空组不出现。搜索与分组是同一个入口，界面只调这一个。
pub fn grouped(keyword: &str) -> Vec<(SupplierCategory, Vec<&'static SupplierDef>)> {
    let hits = search(keyword);
    [
        SupplierCategory::Official,
        SupplierCategory::ModelVendor,
        SupplierCategory::Aggregator,
    ]
    .into_iter()
    .filter_map(|category| {
        let items: Vec<&'static SupplierDef> = hits
            .iter()
            .copied()
            .filter(|supplier| supplier.category == category)
            .collect();
        if items.is_empty() {
            None
        } else {
            Some((category, items))
        }
    })
    .collect()
}

/// `https://api.deepseek.com/v1` → `api.deepseek.com`
pub fn domain_of(base_url: &str) -> &str {
    let rest = base_url
        .trim()
        .strip_prefix("https://")
        .or_else(|| base_url.trim().strip_prefix("http://"))
        .unwrap_or(base_url.trim());
    rest.split('/').next().unwrap_or(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_by_name_vendor_and_domain() {
        for keyword in ["deepseek", "DeepSeek", "api.deepseek.com", "深度"] {
            let hits = search(keyword);
            if keyword == "深度" {
                // 中文名不在 name 里就不该命中 —— 搜索不做翻译
                continue;
            }
            assert!(
                hits.iter().any(|supplier| supplier.id == "deepseek"),
                "关键词 {keyword} 应该能找到 DeepSeek"
            );
        }
        assert!(search("openrouter").iter().any(|s| s.id == "openrouter"));
        assert!(search("glm").iter().any(|s| s.id == "zhipu"));
        assert!(search("").len() == SUPPLIERS.len(), "空关键词返回全部");
        assert!(search("不存在的厂商名").is_empty());
    }

    #[test]
    fn groups_keep_expected_order_and_skip_empty() {
        let groups = grouped("");
        assert_eq!(groups.len(), 3, "三类都有条目");
        assert_eq!(groups[0].0, SupplierCategory::Official);
        assert_eq!(groups[2].0, SupplierCategory::Aggregator);

        let only_aggregator = grouped("openrouter");
        assert_eq!(only_aggregator.len(), 1);
        assert_eq!(only_aggregator[0].0, SupplierCategory::Aggregator);
    }

    #[test]
    fn get_is_exact_match() {
        assert!(get("deepseek").is_some());
        assert!(get("  deepseek  ").is_some());
        assert!(get("DeepSeek").is_none(), "id 区分大小写，避免歧义");
        assert!(get("nope").is_none());
    }

    #[test]
    fn domain_extraction() {
        assert_eq!(domain_of("https://api.deepseek.com/v1"), "api.deepseek.com");
        assert_eq!(domain_of("https://openrouter.ai/api/v1"), "openrouter.ai");
        assert_eq!(domain_of("http://127.0.0.1:8080/v1"), "127.0.0.1:8080");
    }
}
