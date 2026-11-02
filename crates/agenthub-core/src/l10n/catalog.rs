//! 后端产出的用户可见文案。
//!
//! 一条 `messages!` 声明同时生成 `Msg` 变体与三语模板：新增一条必须把三种语言
//! 都写齐，否则 `template()` 的 match 不完整、直接编译失败。

/// 一次声明 key + 三语文本。
///
/// 生成：
/// - `enum Msg { Variant, … }`
/// - `Msg::template(locale) -> &'static str`（静态切片，取词零分配）
/// - `Msg::key() -> &'static str`（日志 / 校验脚本用）
macro_rules! messages {
    // 三条文案用 `tt` 捕获（每个字符串字面量就是一个 token tree）：
    // `expr` / `literal` 片段后面不能直接跟 `}`，`tt` 没有这个限制。
    ($($variant:ident => { zh_hans: $hans:tt, zh_hant: $hant:tt, en: $en:tt $(,)? }),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Msg {
            $($variant),*
        }

        impl Msg {
            /// 全部变体，供对齐校验使用。
            pub const ALL: &'static [Msg] = &[$(Msg::$variant),*];

            /// 变体名，等价于 key（`ErrIo` → `"ErrIo"`）。
            pub fn key(self) -> &'static str {
                match self {
                    $(Msg::$variant => stringify!($variant)),*
                }
            }

            /// 某条文案在指定语言下的模板。
            pub fn template(self, locale: crate::l10n::Locale) -> &'static str {
                use crate::l10n::Locale;
                match locale {
                    Locale::ZhHans => match self {
                        $(Msg::$variant => $hans),*
                    },
                    Locale::ZhHant => match self {
                        $(Msg::$variant => $hant),*
                    },
                    Locale::En => match self {
                        $(Msg::$variant => $en),*
                    },
                }
            }
        }
    };
}

messages! {
    // ---------------------------------------------------------------- 错误
    ErrIo => {
        zh_hans: "io 错误: {path}: {source}",
        zh_hant: "io 錯誤：{path}：{source}",
        en: "IO error: {path}: {source}",
    },
    ErrDb => {
        zh_hans: "数据库错误: {source}",
        zh_hant: "資料庫錯誤：{source}",
        en: "Database error: {source}",
    },
    ErrJson => {
        zh_hans: "JSON 解析失败: {source}",
        zh_hant: "JSON 解析失敗：{source}",
        en: "JSON parse failed: {source}",
    },
    ErrToml => {
        zh_hans: "TOML 处理失败: {source}",
        zh_hant: "TOML 處理失敗：{source}",
        en: "TOML handling failed: {source}",
    },
    ErrYaml => {
        zh_hans: "YAML 处理失败: {source}",
        zh_hant: "YAML 處理失敗：{source}",
        en: "YAML handling failed: {source}",
    },
    ErrUnsupported => {
        zh_hans: "不支持的操作: {detail}",
        zh_hant: "不支援的操作：{detail}",
        en: "Unsupported operation: {detail}",
    },
    ErrUnsafePath => {
        zh_hans: "路径被拒绝（越出受管根目录）: {path}",
        zh_hant: "路徑被拒絕（超出受管根目錄）：{path}",
        en: "Path rejected (outside managed roots): {path}",
    },
    ErrNotFound => {
        zh_hans: "未找到: {what}",
        zh_hant: "找不到：{what}",
        en: "Not found: {what}",
    },
    ErrConflict => {
        zh_hans: "冲突: {detail}",
        zh_hant: "衝突：{detail}",
        en: "Conflict: {detail}",
    },
    ErrOther => {
        zh_hans: "{detail}",
        zh_hant: "{detail}",
        en: "{detail}",
    },

    // ------------------------------------------------------------ 变更类型
    KindDedupe => {
        zh_hans: "去重接管",
        zh_hant: "去重接管",
        en: "Dedupe & adopt",
    },
    KindAdopt => {
        zh_hans: "纳入中央仓库",
        zh_hant: "納入中央倉庫",
        en: "Adopt into store",
    },
    KindRepair => {
        zh_hans: "修复链接",
        zh_hant: "修復連結",
        en: "Repair links",
    },
    KindMcpWrite => {
        zh_hans: "MCP 写入",
        zh_hant: "MCP 寫入",
        en: "MCP write",
    },
    KindInstruction => {
        zh_hans: "指令文件写入",
        zh_hant: "指令檔寫入",
        en: "Instruction write",
    },
    KindProjectInit => {
        zh_hans: "项目初始化",
        zh_hant: "專案初始化",
        en: "Project init",
    },
    KindProvider => {
        zh_hans: "Provider 写入",
        zh_hant: "Provider 寫入",
        en: "Provider write",
    },
    KindDelete => {
        zh_hans: "删除技能",
        zh_hant: "刪除技能",
        en: "Delete skills",
    },
    KindMarketInstall => {
        zh_hans: "市场安装",
        zh_hant: "市集安裝",
        en: "Market install",
    },
    KindRollback => {
        zh_hans: "回滚",
        zh_hant: "回滾",
        en: "Rollback",
    },

    // ------------------------------------------------------------ 文件操作
    OpCreateLink => {
        zh_hans: "建立{strategy}",
        zh_hant: "建立{strategy}",
        en: "Create {strategy}",
    },
    OpReplaceLink => {
        zh_hans: "替换为{strategy}",
        zh_hant: "取代為{strategy}",
        en: "Replace with {strategy}",
    },
    OpMoveIntoStore => {
        zh_hans: "移入中央仓库",
        zh_hant: "移入中央倉庫",
        en: "Move into hub store",
    },
    OpRemoveTrash => {
        zh_hans: "移入回收站",
        zh_hant: "移入資源回收筒",
        en: "Move to trash",
    },
    OpRemove => {
        zh_hans: "删除",
        zh_hant: "刪除",
        en: "Delete",
    },
    OpWriteFile => {
        zh_hans: "写入文件",
        zh_hant: "寫入檔案",
        en: "Write file",
    },

    DescCreateLink => {
        zh_hans: "链接 {at} -> {target}",
        zh_hant: "連結 {at} -> {target}",
        en: "Link {at} -> {target}",
    },
    DescReplaceLink => {
        zh_hans: "替换 {at} 为指向 {target} 的链接",
        zh_hant: "取代 {at} 為指向 {target} 的連結",
        en: "Replace {at} with a link to {target}",
    },
    DescMove => {
        zh_hans: "移动 {from} -> {to}",
        zh_hant: "移動 {from} -> {to}",
        en: "Move {from} -> {to}",
    },
    DescRemoveTrash => {
        zh_hans: "移入回收站 {at}",
        zh_hant: "移入資源回收筒 {at}",
        en: "Move to trash {at}",
    },
    DescRemove => {
        zh_hans: "删除 {at}",
        zh_hant: "刪除 {at}",
        en: "Delete {at}",
    },
    DescWriteFile => {
        zh_hans: "写入 {at} ({bytes} 字节)",
        zh_hant: "寫入 {at}（{bytes} 位元組）",
        en: "Write {at} ({bytes} bytes)",
    },

    // ------------------------------------------------------------ 链接策略
    StrategySymlink => {
        zh_hans: "符号链接",
        zh_hant: "符號連結",
        en: "symlink",
    },
    StrategyJunction => {
        zh_hans: "目录联接",
        zh_hant: "目錄聯結",
        en: "junction",
    },
    StrategyHardlink => {
        zh_hans: "硬链接",
        zh_hant: "硬連結",
        en: "hard link",
    },
    StrategyCopy => {
        zh_hans: "复制副本",
        zh_hant: "複製副本",
        en: "copy",
    },

    // ---------------------------------------------------------------- 作用域
    ScopeGlobal => {
        zh_hans: "全局",
        zh_hant: "全域",
        en: "Global",
    },
    ScopeProject => {
        zh_hans: "项目 {name}",
        zh_hant: "專案 {name}",
        en: "Project {name}",
    },

    // ------------------------------------------------------------ 指令文件
    InstrRule => {
        zh_hans: "规则",
        zh_hant: "規則",
        en: "Rules",
    },
    InstrPersona => {
        zh_hans: "人格",
        zh_hant: "人格",
        en: "Persona",
    },
    InstrBridge => {
        zh_hans: "桥接",
        zh_hant: "橋接",
        en: "Bridge",
    },

    // ---------------------------------------------------------------- 市场
    SortRecommended => {
        zh_hans: "推荐",
        zh_hant: "推薦",
        en: "Recommended",
    },
    SortDownloads => {
        zh_hans: "下载量",
        zh_hant: "下載量",
        en: "Downloads",
    },
    SortUpdated => {
        zh_hans: "最近更新",
        zh_hant: "最近更新",
        en: "Recently updated",
    },

    // ------------------------------------------------------------ 网关模式
    PoolDirect => {
        zh_hans: "直连",
        zh_hant: "直連",
        en: "Direct",
    },
    PoolRoute => {
        zh_hans: "路由",
        zh_hant: "路由",
        en: "Route",
    },
    PoolAggregate => {
        zh_hans: "聚合",
        zh_hant: "聚合",
        en: "Aggregate",
    },
    RouteSingle => {
        zh_hans: "单上游",
        zh_hant: "單一上游",
        en: "Single upstream",
    },

    // -------------------------------------------------------------------- CLI
    CliInitFailed => {
        zh_hans: "初始化失败：{error}",
        zh_hant: "初始化失敗：{error}",
        en: "Failed to initialize: {error}",
    },

    // ---------------------------------------------------------------- 功能开关
    FeatureDisabled => {
        zh_hans: "「{name}」已禁用：请在 设置 › 功能管理 中开启",
        zh_hant: "「{name}」已停用：請在 設定 › 功能管理 中開啟",
        en: "“{name}” is disabled. Turn it on in Settings › Features.",
    },
    FeatureProvider => {
        zh_hans: "供应商",
        zh_hant: "供應商",
        en: "Providers",
    },
    FeatureUsage => {
        zh_hans: "Token 统计",
        zh_hant: "Token 統計",
        en: "Token usage",
    },
    FeatureProviderDesc => {
        zh_hans: "上游登记、本地网关路由与按 agent 的绑定写入",
        zh_hant: "上游登記、本機閘道路由與依 agent 的綁定寫入",
        en: "Upstream registry, local gateway routing and per-agent binding writes",
    },
    FeatureUsageDesc => {
        zh_hans: "只读解析本机会话日志，统计 token 消耗与估算花费",
        zh_hant: "唯讀解析本機會話記錄，統計 token 消耗與估算花費",
        en: "Read-only parsing of local session logs for token spend and estimated cost",
    },
    ErrUnknownFeature => {
        zh_hans: "未知的功能模块：{key}",
        zh_hant: "未知的功能模組：{key}",
        en: "Unknown feature module: {key}",
    },
    ErrUnknownLocale => {
        zh_hans: "未知的界面语言：{value}",
        zh_hant: "未知的介面語言：{value}",
        en: "Unknown UI language: {value}",
    },

    // ------------------------------------------------------------ 供应商类别
    SupplierOfficial => {
        zh_hans: "官方",
        zh_hant: "官方",
        en: "Official",
    },
    SupplierVendor => {
        zh_hans: "模型厂商",
        zh_hant: "模型廠商",
        en: "Model vendor",
    },
    SupplierAggregator => {
        zh_hans: "第三方聚合",
        zh_hant: "第三方聚合",
        en: "Aggregator",
    },

    // ------------------------------------------------------ 变更计划：去重 / 修复
    PlanTitleDedupe => {
        zh_hans: "技能去重接管",
        zh_hant: "技能去重接管",
        en: "Dedupe and adopt skills",
    },
    PlanBaselineUnusable => {
        zh_hans: "指定的基线 {at} 不能作为真身（只读 / 仓库 / 链接），{name} 已回退到默认选择",
        zh_hant: "指定的基準 {at} 不能作為真身（唯讀 / 倉庫 / 連結），{name} 已回退到預設選擇",
        en: "Chosen baseline {at} cannot be the hub copy (read-only / store / link); {name} fell back to the default pick",
    },
    PlanSkipNoBaseline => {
        zh_hans: "跳过 {name}：没有可作基线的实体副本（全为外部/失效链接）",
        zh_hant: "跳過 {name}：沒有可作基準的實體副本（全為外部/失效連結）",
        en: "Skipped {name}: no real directory copy to use as a baseline (all foreign or broken links)",
    },
    PlanBaselineChosen => {
        zh_hans: "以 {at} 作为 {name} 的基线",
        zh_hant: "以 {at} 作為 {name} 的基準",
        en: "Using {at} as the baseline for {name}",
    },
    PlanBaselineChosenByUser => {
        zh_hans: "以 {at} 作为 {name} 的基线（由用户指定）",
        zh_hant: "以 {at} 作為 {name} 的基準（由使用者指定）",
        en: "Using {at} as the baseline for {name} (picked by you)",
    },
    PlanCopiesDifferTrash => {
        zh_hans: "{name} 的多份拷贝内容不一致，基线选定后其余版本会进回收站，需要时可用 rollback 恢复",
        zh_hant: "{name} 的多份副本內容不一致，基準選定後其餘版本會進資源回收筒，需要時可用 rollback 還原",
        en: "Copies of {name} differ; once a baseline is picked the other versions go to the trash and can be restored with rollback",
    },
    PlanCopiesDifferBackup => {
        zh_hans: "{name} 的多份拷贝内容不一致，基线选定后其余版本会进备份目录，需要时可用 rollback 恢复",
        zh_hant: "{name} 的多份副本內容不一致，基準選定後其餘版本會進備份目錄，需要時可用 rollback 還原",
        en: "Copies of {name} differ; once a baseline is picked the other versions go to the backup directory and can be restored with rollback",
    },
    PlanBaselineIgnored => {
        zh_hans: "{name} 在中央仓库已有版本，忽略指定的基线（如需换真身请先手动处理仓库内容）",
        zh_hant: "{name} 在中央倉庫已有版本，忽略指定的基準（如需換真身請先手動處理倉庫內容）",
        en: "{name} already exists in the hub store; the chosen baseline is ignored (change the store copy manually first)",
    },
    PlanSkipForeignLink => {
        zh_hans: "跳过 {at}：外部链接（指向受管目录之外），不接管",
        zh_hant: "跳過 {at}：外部連結（指向受管目錄之外），不接管",
        en: "Skipped {at}: foreign link (points outside managed roots), not adopted",
    },
    PlanSkipAgentLocal => {
        zh_hans: "跳过 {name}：已标记为 agent 独有技能，不参与统一管理（技能库里右键可取消标记）",
        zh_hant: "跳過 {name}：已標記為 agent 獨有技能，不參與統一管理（技能庫裡右鍵可取消標記）",
        en: "Skipped {name}: marked as agent-specific, excluded from unified management (right-click in Skills to unmark)",
    },
    PlanNothingToDedupe => {
        zh_hans: "没有需要处理的重复技能",
        zh_hant: "沒有需要處理的重複技能",
        en: "No duplicate skills to handle",
    },
    PlanListSeparator => {
        zh_hans: "、",
        zh_hant: "、",
        en: ", ",
    },
    PlanTitleRepair => {
        zh_hans: "修复链接",
        zh_hant: "修復連結",
        en: "Repair links",
    },
    PlanRepairNoTarget => {
        zh_hans: "{name} 在中央仓库没有对应目录（{target}），无法修复",
        zh_hant: "{name} 在中央倉庫沒有對應目錄（{target}），無法修復",
        en: "{name} has no matching directory in the hub store ({target}); cannot repair",
    },
    PlanRepairAllHealthy => {
        zh_hans: "所有链接都健康",
        zh_hant: "所有連結都健康",
        en: "All links are healthy",
    },
    PlanRepairSkippedLocal => {
        zh_hans: "已跳过 {count} 个 agent 独有技能（不参与自动修复）：{names}",
        zh_hant: "已跳過 {count} 個 agent 獨有技能（不參與自動修復）：{names}",
        en: "Skipped {count} agent-specific skills (excluded from auto repair): {names}",
    },

    // ------------------------------------------------------------ 变更计划：删除
    ErrNoSkillSelected => {
        zh_hans: "没有选中任何技能",
        zh_hant: "沒有選中任何技能",
        en: "No skills selected",
    },
    PlanTitleDelete => {
        zh_hans: "删除 {count} 项技能",
        zh_hant: "刪除 {count} 項技能",
        en: "Delete {count} skills",
    },
    PlanSkipNotScanned => {
        zh_hans: "跳过 {at}：不在当前扫描结果里（可能已被外部改动，请先重新扫描）",
        zh_hant: "跳過 {at}：不在目前掃描結果裡（可能已被外部改動，請先重新掃描）",
        en: "Skipped {at}: not in the current scan (changed externally? rescan first)",
    },
    PlanSkipReadonly => {
        zh_hans: "跳过 {at}：只读内置技能，AgentHub 不会删除",
        zh_hant: "跳過 {at}：唯讀內建技能，AgentHub 不會刪除",
        en: "Skipped {at}: read-only built-in skill, AgentHub will not delete it",
    },
    PlanDeleteSummaryTrash => {
        zh_hans: "将删除 {count} 项：仓库真身 {bodies} / agent 副本 {copies} / 链接 {links}（进回收站，可用批次回滚）",
        zh_hant: "將刪除 {count} 項：倉庫真身 {bodies} / agent 副本 {copies} / 連結 {links}（進資源回收筒，可用批次回滾）",
        en: "Will delete {count} items: {bodies} store copies / {copies} agent copies / {links} links (to the trash, batch rollback available)",
    },
    PlanDeleteSummaryPermanent => {
        zh_hans: "将删除 {count} 项：仓库真身 {bodies} / agent 副本 {copies} / 链接 {links}（永久删除，仅能靠批次备份恢复）",
        zh_hant: "將刪除 {count} 項：倉庫真身 {bodies} / agent 副本 {copies} / 連結 {links}（永久刪除，僅能靠批次備份還原）",
        en: "Will delete {count} items: {bodies} store copies / {copies} agent copies / {links} links (permanent; recoverable only from the batch backup)",
    },
    PlanDeleteDangling => {
        zh_hans: "其中 {count} 条链接指向被删的仓库真身，已一并清理",
        zh_hant: "其中 {count} 條連結指向被刪的倉庫真身，已一併清理",
        en: "{count} links pointed at deleted store copies and were cleaned up too",
    },
    PlanDeleteNothing => {
        zh_hans: "没有可删除的项",
        zh_hant: "沒有可刪除的項",
        en: "Nothing to delete",
    },
    RiskPermanentDelete => {
        zh_hans: "永久删除：不进系统回收站，只能通过批次回滚恢复",
        zh_hant: "永久刪除：不進系統資源回收筒，只能透過批次回滾還原",
        en: "Permanent delete: bypasses the system trash, only batch rollback can restore it",
    },
    RiskKeptDanglingLinks => {
        zh_hans: "删除了 {count} 个仓库真身但保留了指向它们的链接，这些链接会变成失效链接",
        zh_hant: "刪除了 {count} 個倉庫真身但保留了指向它們的連結，這些連結會變成失效連結",
        en: "Deleted {count} store copies but kept the links pointing at them; those links will become broken",
    },
    RiskManyBodies => {
        zh_hans: "一次删除 {count} 个仓库真身，请确认清单",
        zh_hant: "一次刪除 {count} 個倉庫真身，請確認清單",
        en: "Deleting {count} store copies in one batch; please double-check the list",
    },

    // -------------------------------------------------------- 变更计划：MCP / 指令
    McpUnsupportedAgent => {
        zh_hans: "该 agent 不支持 MCP 写入",
        zh_hant: "該 agent 不支援 MCP 寫入",
        en: "This agent does not support MCP writes",
    },
    ErrNoMcpToWrite => {
        zh_hans: "没有要写入的 MCP",
        zh_hant: "沒有要寫入的 MCP",
        en: "No MCP servers to write",
    },
    PlanTitleMcpWrite => {
        zh_hans: "写入 {count} 个 MCP 定义",
        zh_hant: "寫入 {count} 個 MCP 定義",
        en: "Write {count} MCP definitions",
    },
    PlanMcpSkipUnsupported => {
        zh_hans: "{agent} 不支持 MCP，已跳过",
        zh_hant: "{agent} 不支援 MCP，已跳過",
        en: "{agent} does not support MCP; skipped",
    },
    PlanMcpSkipDuplicate => {
        zh_hans: "{at} 已由另一平台处理，跳过 {agent} 的重复写入",
        zh_hant: "{at} 已由另一平台處理，跳過 {agent} 的重複寫入",
        en: "{at} is handled by another platform; skipped the duplicate write for {agent}",
    },
    PlanMcpUpToDate => {
        zh_hans: "{at} 已是最新，无需改动",
        zh_hant: "{at} 已是最新，無需改動",
        en: "{at} is already up to date",
    },
    PlanMcpCreate => {
        zh_hans: "将新建 {at}（{format}）",
        zh_hant: "將新建 {at}（{format}）",
        en: "Will create {at} ({format})",
    },
    PlanMcpAllCurrent => {
        zh_hans: "所有目标配置都已是期望状态",
        zh_hant: "所有目標設定都已是期望狀態",
        en: "All target configs are already in the desired state",
    },
    ErrUnknownAgent => {
        zh_hans: "未知 agent: {slug}",
        zh_hant: "未知 agent：{slug}",
        en: "Unknown agent: {slug}",
    },
    ErrNoRuleFile => {
        zh_hans: "{agent} 没有可写入的规则文件",
        zh_hant: "{agent} 沒有可寫入的規則檔",
        en: "{agent} has no rule file to write",
    },
    PlanTitleInstruction => {
        zh_hans: "写入共享指令片段",
        zh_hant: "寫入共享指令片段",
        en: "Write shared instruction block",
    },
    PlanInstructionWrite => {
        zh_hans: "写入 {at}",
        zh_hant: "寫入 {at}",
        en: "Write {at}",
    },
    ErrBridgeNoSource => {
        zh_hans: "没有找到 {at}，无法建立桥接",
        zh_hant: "找不到 {at}，無法建立橋接",
        en: "Cannot bridge: {at} not found",
    },
    PlanTitleBridge => {
        zh_hans: "生成 AGENTS.md 桥接",
        zh_hant: "產生 AGENTS.md 橋接",
        en: "Create AGENTS.md bridge",
    },
    PlanBridgeImport => {
        zh_hans: "{at} 将导入 @AGENTS.md（{agent}）",
        zh_hant: "{at} 將匯入 @AGENTS.md（{agent}）",
        en: "{at} will import @AGENTS.md ({agent})",
    },
    PlanBridgeExists => {
        zh_hans: "桥接已存在",
        zh_hant: "橋接已存在",
        en: "Bridge already in place",
    },
    PlanTitleProjectInit => {
        zh_hans: "初始化项目作用域",
        zh_hant: "初始化專案作用域",
        en: "Initialize project scope",
    },
    PlanProjectAlreadyInit => {
        zh_hans: "项目已初始化，无需重复创建",
        zh_hant: "專案已初始化，無需重複建立",
        en: "Project already initialized",
    },
    PlanProjectCreate => {
        zh_hans: "创建 {at}",
        zh_hant: "建立 {at}",
        en: "Create {at}",
    },

    // ------------------------------------------------------------ 执行与回滚日志
    BatchEmptyPlan => {
        zh_hans: "计划为空，无需执行",
        zh_hant: "計畫為空，無需執行",
        en: "Empty plan; nothing to execute",
    },
    BatchFailedAt => {
        zh_hans: "失败于 [{desc}]: {error}",
        zh_hant: "失敗於 [{desc}]：{error}",
        en: "Failed at [{desc}]: {error}",
    },
    BatchCreateLink => {
        zh_hans: "建立{strategy} {at}",
        zh_hant: "建立{strategy} {at}",
        en: "Created {strategy} {at}",
    },
    BatchReplaceLink => {
        zh_hans: "替换为链接 {at}",
        zh_hant: "取代為連結 {at}",
        en: "Replaced with a link {at}",
    },
    ErrStoreExists => {
        zh_hans: "中央仓库已存在同名技能: {name}",
        zh_hant: "中央倉庫已存在同名技能：{name}",
        en: "The hub store already has a skill with this name: {name}",
    },
    ErrMoveLinkRefused => {
        zh_hans: "{at} 是符号链接/目录联接，拒绝移入中央仓库（请先修复链接）",
        zh_hant: "{at} 是符號連結/目錄聯結，拒絕移入中央倉庫（請先修復連結）",
        en: "{at} is a symlink/junction; refusing to move it into the hub store (repair the link first)",
    },
    BatchMove => {
        zh_hans: "移入仓库 {from} -> {to}",
        zh_hant: "移入倉庫 {from} -> {to}",
        en: "Moved into store {from} -> {to}",
    },
    BatchRemoveLink => {
        zh_hans: "删除链接 {at}",
        zh_hant: "刪除連結 {at}",
        en: "Removed link {at}",
    },
    BatchTrashed => {
        zh_hans: "移入回收站 {at}（已留备份，可回滚）",
        zh_hant: "移入資源回收筒 {at}（已留備份，可回滾）",
        en: "Moved to trash {at} (backup kept, rollback available)",
    },
    BatchTrashUnavailable => {
        zh_hans: "回收站不可用（{error}），已改存备份 {at}",
        zh_hant: "資源回收筒不可用（{error}），已改存備份 {at}",
        en: "Trash unavailable ({error}); kept a backup at {at} instead",
    },
    BatchDelete => {
        zh_hans: "删除 {at}",
        zh_hant: "刪除 {at}",
        en: "Deleted {at}",
    },
    BatchWrite => {
        zh_hans: "写入 {at}",
        zh_hant: "寫入 {at}",
        en: "Wrote {at}",
    },
    ErrBatchNoBackup => {
        zh_hans: "批次 {batch} 没有备份目录",
        zh_hant: "批次 {batch} 沒有備份目錄",
        en: "Batch {batch} has no backup directory",
    },
    RollbackRemoveLink => {
        zh_hans: "移除链接 {at}",
        zh_hant: "移除連結 {at}",
        en: "Removed link {at}",
    },
    RollbackRestore => {
        zh_hans: "还原 {at}",
        zh_hant: "還原 {at}",
        en: "Restored {at}",
    },
    RollbackRestoredBackup => {
        zh_hans: "从备份恢复 {at}",
        zh_hant: "從備份還原 {at}",
        en: "Restored {at} from backup",
    },
    RollbackRestoredBackupTrashKept => {
        zh_hans: "从备份恢复 {at}（回收站副本保留）",
        zh_hant: "從備份還原 {at}（資源回收筒副本保留）",
        en: "Restored {at} from backup (trash copy kept)",
    },
    RollbackManualTrash => {
        zh_hans: "{at} 已进系统回收站，请手动恢复",
        zh_hant: "{at} 已進系統資源回收筒，請手動還原",
        en: "{at} is in the system trash; restore it manually",
    },
    RollbackRelink => {
        zh_hans: "重建链接 {at}",
        zh_hant: "重建連結 {at}",
        en: "Recreated link {at}",
    },
    RollbackRestoreContent => {
        zh_hans: "恢复旧内容 {at}",
        zh_hant: "還原舊內容 {at}",
        en: "Restored previous content {at}",
    },
    RollbackRemoveCreated => {
        zh_hans: "删除新建文件 {at}",
        zh_hant: "刪除新建檔案 {at}",
        en: "Removed newly created file {at}",
    },
    RollbackUndoRemoveLink => {
        zh_hans: "回滚: 移除 {at}",
        zh_hant: "回滾：移除 {at}",
        en: "Rollback: removed {at}",
    },
    RollbackUndoRestore => {
        zh_hans: "回滚: 还原 {at}",
        zh_hant: "回滾：還原 {at}",
        en: "Rollback: restored {at}",
    },
    RollbackUndoRecover => {
        zh_hans: "回滚: 恢复 {at}",
        zh_hant: "回滾：還原 {at}",
        en: "Rollback: recovered {at}",
    },
    RollbackUndoRelink => {
        zh_hans: "回滚: 重建链接 {at}",
        zh_hant: "回滾：重建連結 {at}",
        en: "Rollback: recreated link {at}",
    },
    RollbackUndoWrite => {
        zh_hans: "回滚: 写入撤销 {at}",
        zh_hant: "回滾：寫入撤銷 {at}",
        en: "Rollback: undid write {at}",
    },

    // -------------------------------------------------- 常见 Agent 目录：安装步骤标题
    CatalogStepInstallClaudeCode => {
        zh_hans: "安装 Claude Code",
        zh_hant: "安裝 Claude Code",
        en: "Install Claude Code",
    },
    CatalogStepVerify => {
        zh_hans: "验证",
        zh_hant: "驗證",
        en: "Verify",
    },
    CatalogStepUseOfficialScript => {
        zh_hans: "或用官方安装脚本",
        zh_hant: "或用官方安裝腳本",
        en: "Or use the official install script",
    },
    CatalogStepInstallCodex => {
        zh_hans: "安装 Codex CLI",
        zh_hant: "安裝 Codex CLI",
        en: "Install Codex CLI",
    },
    CatalogStepInstallCodebuddy => {
        zh_hans: "安装 CodeBuddy Code",
        zh_hant: "安裝 CodeBuddy Code",
        en: "Install CodeBuddy Code",
    },
    CatalogStepCodebuddySite => {
        zh_hans: "登录站点选择",
        zh_hant: "登入站點選擇",
        en: "Choose a sign-in site",
    },
    CatalogStepCodebuddyIdeIntl => {
        zh_hans: "下载 CodeBuddy IDE（国际版）",
        zh_hant: "下載 CodeBuddy IDE（國際版）",
        en: "Download CodeBuddy IDE (International)",
    },
    CatalogStepCodebuddyIdeCn => {
        zh_hans: "下载 CodeBuddy IDE（国内版）",
        zh_hant: "下載 CodeBuddy IDE（國內版）",
        en: "Download CodeBuddy IDE (China)",
    },
    CatalogStepConfirmInstalled => {
        zh_hans: "确认安装",
        zh_hant: "確認安裝",
        en: "Confirm installation",
    },
    CatalogStepInstallGemini => {
        zh_hans: "安装 Gemini CLI",
        zh_hant: "安裝 Gemini CLI",
        en: "Install Gemini CLI",
    },
    CatalogStepDownloadClaudeDesktop => {
        zh_hans: "下载 Claude Desktop",
        zh_hant: "下載 Claude Desktop",
        en: "Download Claude Desktop",
    },
    CatalogStepRestartAfterLogin => {
        zh_hans: "登录后重启应用",
        zh_hant: "登入後重新啟動應用程式",
        en: "Restart the app after signing in",
    },
    CatalogStepDownloadChatgpt => {
        zh_hans: "下载 ChatGPT 桌面端",
        zh_hant: "下載 ChatGPT 桌面版",
        en: "Download ChatGPT desktop",
    },
    CatalogStepEnableMcp => {
        zh_hans: "开启 MCP 支持",
        zh_hant: "開啟 MCP 支援",
        en: "Enable MCP support",
    },
    CatalogStepDownloadWorkbuddy => {
        zh_hans: "下载 WorkBuddy（国内版）",
        zh_hant: "下載 WorkBuddy（國內版）",
        en: "Download WorkBuddy (China)",
    },
    CatalogStepIntlIsOtherClient => {
        zh_hans: "国际版是另一个客户端",
        zh_hant: "國際版是另一個用戶端",
        en: "The international build is a separate app",
    },
    CatalogStepDownloadWorkbuddyAi => {
        zh_hans: "下载 WorkBuddy AI（国际版）",
        zh_hant: "下載 WorkBuddy AI（國際版）",
        en: "Download WorkBuddy AI (International)",
    },
    CatalogStepCnIsOtherClient => {
        zh_hans: "国内版是另一个客户端",
        zh_hant: "國內版是另一個用戶端",
        en: "The China build is a separate app",
    },
    CatalogStepDownloadTrae => {
        zh_hans: "下载 Trae（国际版）",
        zh_hant: "下載 Trae（國際版）",
        en: "Download Trae (International)",
    },
    CatalogStepCnIsOtherPackage => {
        zh_hans: "国内版是另一个安装包",
        zh_hant: "國內版是另一個安裝包",
        en: "The China build is a separate package",
    },
    CatalogStepProjectMcp => {
        zh_hans: "项目级 MCP 位置",
        zh_hant: "專案級 MCP 位置",
        en: "Project-level MCP location",
    },
    CatalogStepDownloadTraeCn => {
        zh_hans: "下载 Trae CN（国内版）",
        zh_hant: "下載 Trae CN（國內版）",
        en: "Download Trae CN (China)",
    },
    CatalogStepIntlIsOtherPackage => {
        zh_hans: "国际版是另一个安装包",
        zh_hant: "國際版是另一個安裝包",
        en: "The international build is a separate package",
    },
    CatalogStepInstallOpencode => {
        zh_hans: "安装 OpenCode",
        zh_hant: "安裝 OpenCode",
        en: "Install OpenCode",
    },
    CatalogStepHomebrew => {
        zh_hans: "用 Homebrew 安装",
        zh_hant: "用 Homebrew 安裝",
        en: "Install with Homebrew",
    },
    CatalogStepInstallScript => {
        zh_hans: "或用安装脚本",
        zh_hant: "或用安裝腳本",
        en: "Or use the install script",
    },
    CatalogStepInstallPi => {
        zh_hans: "安装 Pi",
        zh_hant: "安裝 Pi",
        en: "Install Pi",
    },
    CatalogStepConfirmPackage => {
        zh_hans: "确认包名后再安装",
        zh_hant: "確認套件名後再安裝",
        en: "Check the package name before installing",
    },
    CatalogStepDownloadManus => {
        zh_hans: "下载 Manus 桌面端",
        zh_hant: "下載 Manus 桌面版",
        en: "Download Manus desktop",
    },

    // -------------------------------------------------- 常见 Agent 目录：步骤风险说明
    CatalogRiskNpm => {
        zh_hans: "需要 Node.js 18+；全局安装可能要管理员权限",
        zh_hant: "需要 Node.js 18+；全域安裝可能需要系統管理員權限",
        en: "Requires Node.js 18+; a global install may need administrator rights",
    },
    CatalogRiskVerify => {
        zh_hans: "只读验证，不会改动任何文件",
        zh_hant: "唯讀驗證，不會改動任何檔案",
        en: "Read-only check, no files are modified",
    },
    CatalogRiskApp => {
        zh_hans: "官网下载安装包；安装完回来点「检测版本」即可",
        zh_hant: "官網下載安裝包；安裝完回來點「偵測版本」即可",
        en: "Download from the official site, then hit “Detect version” here",
    },
    CatalogRiskBinaryScript => {
        zh_hans: "脚本会下载原生二进制；执行前请先读一遍来源域名",
        zh_hant: "腳本會下載原生二進位檔；執行前請先讀一遍來源網域",
        en: "The script downloads a native binary; read the source domain before running it",
    },
    CatalogRiskCodebuddySites => {
        zh_hans: "国内站（codebuddy.cn）与国际站（codebuddy.ai）账号与额度独立；CLI 两版共用 ~/.codebuddy",
        zh_hant: "國內站（codebuddy.cn）與國際站（codebuddy.ai）帳號與額度獨立；CLI 兩版共用 ~/.codebuddy",
        en: "China (codebuddy.cn) and international (codebuddy.ai) sites have separate accounts and quotas; both CLI builds share ~/.codebuddy",
    },
    CatalogRiskCodebuddyIdeIntl => {
        zh_hans: "国际版配置目录 ~/.codebuddy，与 CodeBuddy CLI 共用同一份用户配置",
        zh_hant: "國際版設定目錄 ~/.codebuddy，與 CodeBuddy CLI 共用同一份使用者設定",
        en: "The international build uses ~/.codebuddy, sharing the same user config as CodeBuddy CLI",
    },
    CatalogRiskCodebuddyIdeCn => {
        zh_hans: "国内版是独立客户端，配置目录 ~/.codebuddycn；IDE 的 MCP/技能约定未核实，先只做检测",
        zh_hant: "國內版是獨立用戶端，設定目錄 ~/.codebuddycn；IDE 的 MCP/技能約定未核實，先只做偵測",
        en: "The China build is a separate app using ~/.codebuddycn; its MCP/skill conventions are unverified, so detection only",
    },
    CatalogRiskHermes => {
        zh_hans: "Hermes 没有公开的包管理器安装方式，按团队内部分发文档安装",
        zh_hant: "Hermes 沒有公開的套件管理器安裝方式，請依團隊內部分發文件安裝",
        en: "Hermes has no public package-manager install; follow your team's internal distribution guide",
    },
    CatalogRiskRestartNeeded => {
        zh_hans: "MCP 配置只在启动时读取",
        zh_hant: "MCP 設定只在啟動時讀取",
        en: "The MCP config is only read at startup",
    },
    CatalogRiskChatgptDevMode => {
        zh_hans: "自定义 MCP 仅在开发者模式下可用，且需要付费套餐",
        zh_hant: "自訂 MCP 僅在開發者模式下可用，且需要付費方案",
        en: "Custom MCP only works in developer mode and needs a paid plan",
    },
    CatalogRiskWorkbuddyClient => {
        zh_hans: "桌面客户端，与 CodeBuddy 不是同一个产品（同账号体系）；国内版数据根 ~/.workbuddy",
        zh_hant: "桌面用戶端，與 CodeBuddy 不是同一個產品（同帳號體系）；國內版資料根 ~/.workbuddy",
        en: "Desktop app, not the same product as CodeBuddy (same account system); the China build uses ~/.workbuddy",
    },
    CatalogRiskWorkbuddyAiCoexist => {
        zh_hans: "国际版 WorkBuddy AI 数据根 ~/.workbuddy-ai，与国内版互不覆盖，可同机共存",
        zh_hant: "國際版 WorkBuddy AI 資料根 ~/.workbuddy-ai，與國內版互不覆蓋，可同機共存",
        en: "WorkBuddy AI uses ~/.workbuddy-ai, independent of the China build; both can coexist",
    },
    CatalogRiskWorkbuddyAiPaths => {
        zh_hans: "Windows 装在 %LOCALAPPDATA%/Programs/WorkBuddyAI，macOS 是 /Applications/WorkBuddy AI.app",
        zh_hant: "Windows 裝在 %LOCALAPPDATA%/Programs/WorkBuddyAI，macOS 是 /Applications/WorkBuddy AI.app",
        en: "Installs to %LOCALAPPDATA%/Programs/WorkBuddyAI on Windows and /Applications/WorkBuddy AI.app on macOS",
    },
    CatalogRiskWorkbuddyPaths => {
        zh_hans: "国内版 WorkBuddy 数据根 ~/.workbuddy，与 ~/.workbuddy-ai 各自独立",
        zh_hant: "國內版 WorkBuddy 資料根 ~/.workbuddy，與 ~/.workbuddy-ai 各自獨立",
        en: "WorkBuddy (China) uses ~/.workbuddy, independent of ~/.workbuddy-ai",
    },
    CatalogRiskTraePaths => {
        zh_hans: "国际版安装目录 Program Files 外的 %LOCALAPPDATA%/Programs/Trae，配置目录 %APPDATA%/Trae",
        zh_hant: "國際版安裝目錄在 Program Files 外的 %LOCALAPPDATA%/Programs/Trae，設定目錄 %APPDATA%/Trae",
        en: "Installs to %LOCALAPPDATA%/Programs/Trae (outside Program Files) with config in %APPDATA%/Trae",
    },
    CatalogRiskTraeCnPaths => {
        zh_hans: "国内版目录带 CN 后缀（%APPDATA%/Trae CN、~/.trae-cn），与国际版可同机共存",
        zh_hant: "國內版目錄帶 CN 後綴（%APPDATA%/Trae CN、~/.trae-cn），與國際版可同機共存",
        en: "The China build uses CN-suffixed paths (%APPDATA%/Trae CN, ~/.trae-cn) and can coexist with the international build",
    },
    CatalogRiskTraeNoUserMcp => {
        zh_hans: "Trae 没有核实过的用户级 MCP 约定，只能按项目接管",
        zh_hant: "Trae 沒有核實過的使用者層級 MCP 約定，只能依專案接管",
        en: "Trae has no verified user-level MCP convention; only project scope is managed",
    },
    CatalogRiskTraeCnPathsDetail => {
        zh_hans: "国内版配置目录 %APPDATA%/Trae CN、数据根 ~/.trae-cn",
        zh_hant: "國內版設定目錄 %APPDATA%/Trae CN、資料根 ~/.trae-cn",
        en: "The China build uses %APPDATA%/Trae CN and ~/.trae-cn",
    },
    CatalogRiskTraeIntlNoCnSuffix => {
        zh_hans: "国际版目录不带 CN 后缀（%APPDATA%/Trae、~/.trae），与国际版可同机共存",
        zh_hant: "國際版目錄不帶 CN 後綴（%APPDATA%/Trae、~/.trae），兩版可同機共存",
        en: "The international build uses unsuffixed paths (%APPDATA%/Trae, ~/.trae); both builds can coexist",
    },
    CatalogRiskTraeCnSharedFile => {
        zh_hans: "两版是否共用同名项目文件未核实，登记后请确认 Trae CN 能读到",
        zh_hant: "兩版是否共用同名專案檔案未核實，登記後請確認 Trae CN 讀得到",
        en: "It is unverified whether both builds share the same project file; confirm Trae CN can read it after registering",
    },
    CatalogRiskOpencodeTap => {
        zh_hans: "官方 tap 更新更及时；社区 tap 版本可能滞后",
        zh_hant: "官方 tap 更新較即時；社群 tap 版本可能落後",
        en: "The official tap updates faster; community taps may lag behind",
    },
    CatalogRiskOpencodeScript => {
        zh_hans: "脚本会写到 ~/.opencode/bin，执行前请先读一遍来源域名",
        zh_hant: "腳本會寫到 ~/.opencode/bin，執行前請先讀一遍來源網域",
        en: "The script writes to ~/.opencode/bin; read the source domain before running it",
    },
    CatalogRiskPiNode => {
        zh_hans: "官方文档要求 Node.js 22.19+；--ignore-scripts 是为了不在安装期跑脚本",
        zh_hant: "官方文件要求 Node.js 22.19+；--ignore-scripts 是為了不在安裝期跑腳本",
        en: "The docs require Node.js 22.19+; --ignore-scripts avoids running install-time scripts",
    },
    CatalogRiskOpenworkPackage => {
        zh_hans: "包名来自第三方教程，官方仓库未确证；npm 上还有同名不同产品，装前先核对仓库",
        zh_hant: "套件名來自第三方教學，官方儲存庫未確證；npm 上還有同名不同產品，裝前先核對儲存庫",
        en: "The package name comes from third-party tutorials, unconfirmed by the official repo; verify the repo before installing",
    },
    // 步骤里那一行「可复制内容」是说明文字时也走词典（真命令/URL 保持原样，见 StepText）
    CatalogCommandCodebuddySite => {
        zh_hans: "codebuddy 首次启动时选 Chinese Site / International Site",
        zh_hant: "codebuddy 首次啟動時選 Chinese Site / International Site",
        en: "Pick Chinese Site / International Site on codebuddy's first launch",
    },
    CatalogCommandClaudeDesktopRestart => {
        zh_hans: "claude_desktop_config.json 由 AgentHub 填写后需要完全退出并重开",
        zh_hant: "claude_desktop_config.json 由 AgentHub 填好後需要完全結束並重開",
        en: "After AgentHub writes claude_desktop_config.json, quit the app completely and reopen it",
    },
    CatalogCommandTraeProjectMcp => {
        zh_hans: ".trae/mcp.json（顶层 mcpServers）",
        zh_hant: ".trae/mcp.json（頂層 mcpServers）",
        en: ".trae/mcp.json (top-level mcpServers)",
    },
    CatalogRiskManusCloud => {
        zh_hans: "云端 agent + 本地 My Computer；没有本地技能或 MCP 配置文件约定",
        zh_hant: "雲端 agent + 本機 My Computer；沒有本機技能或 MCP 設定檔約定",
        en: "Cloud agent plus a local My Computer; no local skill or MCP config convention",
    },

    // -------------------------------------------------- 常见 Agent 目录：受管说明
    CatalogNoteClaudeDesktop => {
        zh_hans: "只接管 MCP：Claude Desktop 没有技能目录约定",
        zh_hant: "只接管 MCP：Claude Desktop 沒有技能目錄約定",
        en: "MCP only: Claude Desktop has no skill directory convention",
    },
    CatalogNoteTrae => {
        zh_hans: "只接管项目级 MCP（.trae/mcp.json），对已登记的项目生效；Trae 没有用户级配置约定",
        zh_hant: "只接管專案級 MCP（.trae/mcp.json），對已登記的專案生效；Trae 沒有使用者層級設定約定",
        en: "Manages project-level MCP only (.trae/mcp.json) for registered projects; Trae has no user-level convention",
    },
    CatalogNoteTraeCn => {
        zh_hans: "只接管项目级 MCP（.trae/mcp.json）；国内版目录带 CN 后缀（%APPDATA%/Trae CN、~/.trae-cn）",
        zh_hant: "只接管專案級 MCP（.trae/mcp.json）；國內版目錄帶 CN 後綴（%APPDATA%/Trae CN、~/.trae-cn）",
        en: "Manages project-level MCP only (.trae/mcp.json); the China build uses CN-suffixed paths (%APPDATA%/Trae CN, ~/.trae-cn)",
    },
    CatalogNoteWorkbuddy => {
        zh_hans: "接管 ~/.workbuddy 下的技能、MCP 与人格文件；新写入的 MCP server 可能还要在 WorkBuddy 内批准一次",
        zh_hant: "接管 ~/.workbuddy 下的技能、MCP 與人格檔；新寫入的 MCP server 可能還要在 WorkBuddy 內核准一次",
        en: "Manages skills, MCP and persona files under ~/.workbuddy; newly written MCP servers may need approval inside WorkBuddy",
    },
    CatalogNoteWorkbuddyAi => {
        zh_hans: "接管 ~/.workbuddy-ai 下的技能与 MCP；国际版写入的第三方 server 需要在连接器中心手动信任一次",
        zh_hant: "接管 ~/.workbuddy-ai 下的技能與 MCP；國際版寫入的第三方 server 需要在連接器中心手動信任一次",
        en: "Manages skills and MCP under ~/.workbuddy-ai; third-party servers need manual trust in the connector center",
    },
    CatalogNoteShared => {
        zh_hans: "配置与内置 {agent} 共用，已由它接管，不需要重复登记",
        zh_hant: "設定與內建 {agent} 共用，已由它接管，不需要重複登記",
        en: "Shares its config with the built-in {agent}, which already manages it; no need to register it again",
    },

    // ------------------------------------------------ 常见 Agent 目录：检测与错误
    CatalogEvidencePathHit => {
        zh_hans: "PATH 命中 {value}",
        zh_hant: "PATH 命中 {value}",
        en: "Found on PATH: {value}",
    },
    CatalogEvidenceConfigDirExists => {
        zh_hans: "配置目录 {path} 存在",
        zh_hant: "設定目錄 {path} 存在",
        en: "Config directory exists: {path}",
    },
    CatalogEvidenceAppExists => {
        zh_hans: "应用存在 {path}",
        zh_hant: "應用程式存在 {path}",
        en: "App found: {path}",
    },
    CatalogEvidenceStoreInstalled => {
        zh_hans: "Store 包 {name} 已安装",
        zh_hant: "Store 套件 {name} 已安裝",
        en: "Store package installed: {name}",
    },
    CatalogEvidenceConfigExists => {
        zh_hans: "配置存在 {path}",
        zh_hant: "設定存在 {path}",
        en: "Config found: {path}",
    },
    CatalogTargetSkills => {
        zh_hans: "技能目录",
        zh_hant: "技能目錄",
        en: "Skills directory",
    },
    CatalogTargetProjectSkills => {
        zh_hans: "项目技能目录",
        zh_hant: "專案技能目錄",
        en: "Project skills directory",
    },
    CatalogTargetMcpFile => {
        zh_hans: "MCP 文件",
        zh_hant: "MCP 檔",
        en: "MCP file",
    },
    CatalogTargetProjectMcpFile => {
        zh_hans: "项目 MCP 文件",
        zh_hant: "專案 MCP 檔",
        en: "Project MCP file",
    },
    CatalogTargetInstructions => {
        zh_hans: "指令文件",
        zh_hant: "指令檔",
        en: "Instruction files",
    },
    ErrCatalogNoEntry => {
        zh_hans: "目录里没有 {key}",
        zh_hant: "目錄裡沒有 {key}",
        en: "The catalog has no entry named {key}",
    },
    ErrCatalogBuiltinManaged => {
        zh_hans: "{name} 是内置平台，已受管，不需要登记",
        zh_hant: "{name} 是內建平台，已受管，不需要登記",
        en: "{name} is a built-in platform and already managed; no registration needed",
    },
    ErrCatalogShared => {
        zh_hans: "{name} 的配置与内置 {agent} 共用，已由它接管，不需要重复登记",
        zh_hant: "{name} 的設定與內建 {agent} 共用，已由它接管，不需要重複登記",
        en: "{name} shares its config with the built-in {agent}, which already manages it; no need to register it again",
    },
    ErrCatalogUnsupported => {
        zh_hans: "该产品暂不支持纳入受管",
        zh_hant: "該產品暫不支援納入受管",
        en: "This product cannot be managed yet",
    },
    ErrCatalogNoPaths => {
        zh_hans: "{name} 没有可接管的路径",
        zh_hant: "{name} 沒有可接管的路径",
        en: "{name} has no manageable paths",
    },
    ErrCatalogBuiltinNoRemove => {
        zh_hans: "{name} 是内置平台，不能移出受管",
        zh_hant: "{name} 是內建平台，不能移出受管",
        en: "{name} is a built-in platform and cannot be unmanaged",
    },
    CatalogBlockedCodebuddyIde => {
        zh_hans: "IDE 的 MCP / 技能配置位置未实测（App Support 下只有 VSCode 派生目录，settings.json 无 mcpServers），先只做检测",
        zh_hant: "IDE 的 MCP / 技能設定位置未實測（App Support 下只有 VSCode 衍生目錄，settings.json 無 mcpServers），先只做偵測",
        en: "The IDE's MCP/skill config location is unverified (only VSCode-derived dirs under App Support, no mcpServers in settings.json); detection only",
    },
    CatalogBlockedPi => {
        zh_hans: "配置目录与技能目录约定未确证，先只展示安装与验证命令",
        zh_hant: "設定目錄與技能目錄約定未確證，先只顯示安裝與驗證命令",
        en: "Config and skill directory conventions are unconfirmed; only install and verify commands are shown",
    },
    CatalogBlockedOpenwork => {
        zh_hans: "包名与配置目录均未获官方确证，先只给仓库入口",
        zh_hant: "套件名與設定目錄均未獲官方確證，先只給儲存庫入口",
        en: "Neither the package name nor the config directory is officially confirmed; only the repo link is shown",
    },
    CatalogBlockedManus => {
        zh_hans: "云端 agent，没有本地技能或 MCP 配置文件约定，只能展示安装入口",
        zh_hant: "雲端 agent，沒有本機技能或 MCP 設定檔約定，只能顯示安裝入口",
        en: "Cloud agent with no local skill or MCP config convention; only the download entry is shown",
    },
    // ------------------------------------------------------------ 供应商写入（纯函数层）
    ProviderContextSize => {
        zh_hans: "{tokens} 上下文",
        zh_hant: "{tokens} 上下文",
        en: "{tokens} context",
    },
    ProviderMaxOutput => {
        zh_hans: "{tokens} 输出",
        zh_hant: "{tokens} 輸出",
        en: "{tokens} output",
    },
    ProviderClaudeChatDirect => {
        zh_hans: "Claude Code 只会说 Anthropic 协议；接 Chat Completions 上游需要在「高级 › 路由」里勾选经网关（网关会做协议转换）",
        zh_hant: "Claude Code 只會說 Anthropic 協議；接 Chat Completions 上游需要在「進階 › 路由」裡勾選經閘道（閘道會做協議轉換）",
        en: "Claude Code only speaks the Anthropic protocol; to use a Chat Completions upstream, enable “via gateway” under Advanced › Routes (the gateway converts protocols)",
    },
    ProviderClaudeResponses => {
        zh_hans: "Claude Code 不支持 Responses 协议的上游：本期网关只做了 Anthropic ⇄ Chat Completions 转换",
        zh_hant: "Claude Code 不支援 Responses 協議的上游：本期閘道只做了 Anthropic ⇄ Chat Completions 轉換",
        en: "Claude Code cannot use a Responses upstream: this release only converts between Anthropic and Chat Completions",
    },
    ProviderCodexAnthropic => {
        zh_hans: "Codex 只有 chat / responses 两种接口，接不了 Anthropic 协议的上游；本期也没做 Responses ⇄ Anthropic 转换",
        zh_hant: "Codex 只有 chat / responses 兩種介面，接不了 Anthropic 協議的上游；本期也沒做 Responses ⇄ Anthropic 轉換",
        en: "Codex only supports chat/responses, so it cannot use an Anthropic upstream; Responses ⇄ Anthropic conversion is not implemented",
    },
    ProviderStatusGemini => {
        zh_hans: "Gemini CLI 的模型配置键名尚未核实（本机无 ~/.gemini，仓库里也没有任何键名证据），本轮不写入",
        zh_hant: "Gemini CLI 的模型設定鍵名尚未核實（本機無 ~/.gemini，儲存庫裡也沒有任何鍵名證據），本輪不寫入",
        en: "Gemini CLI's model config keys are unverified (no ~/.gemini here and no key evidence in the repo); not written in this release",
    },
    ProviderStatusCodebuddy => {
        zh_hans: "CodeBuddy 的模型配置是 models[] 数组（每条自带 url / apiKey），没有单一的「当前 provider」概念，本轮不写入",
        zh_hant: "CodeBuddy 的模型設定是 models[] 陣列（每筆自帶 url / apiKey），沒有單一的「目前 provider」概念，本輪不寫入",
        en: "CodeBuddy stores models as a models[] array (each with its own url/apiKey); there is no single “current provider”, so it is not written",
    },
    ProviderStatusHermes => {
        zh_hans: "Hermes 的 config.yaml 需要块级 YAML 改写，且 base_url / api、model / default_model 存在别名，待核实后接入",
        zh_hant: "Hermes 的 config.yaml 需要區塊層級 YAML 改寫，且 base_url / api、model / default_model 存在別名，待核實後接入",
        en: "Hermes needs block-level YAML editing for config.yaml, and base_url/api plus model/default_model are aliased; pending verification",
    },
    ProviderStatusCustom => {
        zh_hans: "自定义平台没有声明 provider 配置格式，无法安全写入",
        zh_hant: "自訂平台沒有宣告 provider 設定格式，無法安全寫入",
        en: "Custom platforms declare no provider config format, so writing is unsafe",
    },
    ProviderStatusOther => {
        zh_hans: "{agent} 的 API 配置格式尚未接入",
        zh_hant: "{agent} 的 API 設定格式尚未接入",
        en: "{agent}'s API config format is not supported yet",
    },
    ErrProviderNoConfigDir => {
        zh_hans: "{agent} 没有配置目录，无法定位 provider 配置文件",
        zh_hant: "{agent} 沒有設定目錄，無法定位 provider 設定檔",
        en: "{agent} has no config directory; cannot locate its provider config file",
    },
    ErrSettingsNotJson => {
        zh_hans: "settings.json 不是合法 JSON：{error}",
        zh_hant: "settings.json 不是合法 JSON：{error}",
        en: "settings.json is not valid JSON: {error}",
    },
    ErrSettingsNotObject => {
        zh_hans: "settings.json 顶层不是对象，已拒绝改写",
        zh_hant: "settings.json 頂層不是物件，已拒絕改寫",
        en: "settings.json is not a top-level object; refusing to rewrite it",
    },
    ErrSettingsEnvNotObject => {
        zh_hans: "settings.json 的 env 不是对象，已拒绝改写",
        zh_hant: "settings.json 的 env 不是物件，已拒絕改寫",
        en: "settings.json has a non-object env; refusing to rewrite it",
    },
    ErrTomlParse => {
        zh_hans: "config.toml 解析失败：{error}",
        zh_hant: "config.toml 解析失敗：{error}",
        en: "Failed to parse config.toml: {error}",
    },
    ErrProviderIdEmpty => {
        zh_hans: "Provider id 不能为空",
        zh_hant: "Provider id 不能為空",
        en: "Provider id cannot be empty",
    },
    ErrModelProvidersNotTable => {
        zh_hans: "config.toml 的 model_providers 不是表，已拒绝改写",
        zh_hant: "config.toml 的 model_providers 不是表格，已拒絕改寫",
        en: "config.toml has a non-table model_providers; refusing to rewrite it",
    },
    ErrOpencodeNotJson => {
        zh_hans: "opencode.json 不是合法 JSON：{error}",
        zh_hant: "opencode.json 不是合法 JSON：{error}",
        en: "opencode.json is not valid JSON: {error}",
    },
    ErrOpencodeNotObject => {
        zh_hans: "opencode.json 顶层不是对象，已拒绝改写",
        zh_hant: "opencode.json 頂層不是物件，已拒絕改寫",
        en: "opencode.json is not a top-level object; refusing to rewrite it",
    },
    ErrConfigNodeNotObject => {
        zh_hans: "配置节点不是对象，已拒绝改写",
        zh_hant: "設定節點不是物件，已拒絕改寫",
        en: "A config node is not an object; refusing to rewrite it",
    },
    ErrConfigNotJson => {
        zh_hans: "配置文件不是合法 JSON：{error}",
        zh_hant: "設定檔不是合法 JSON：{error}",
        en: "The config file is not valid JSON: {error}",
    },
    ErrConfigNotObject => {
        zh_hans: "配置顶层不是对象，已拒绝改写",
        zh_hant: "設定頂層不是物件，已拒絕改寫",
        en: "The config is not a top-level object; refusing to rewrite it",
    },
    // ------------------------------------------------------------ 供应商：输入校验
    ErrProviderIdChars => {
        zh_hans: "Provider id `{id}` 只能用小写字母、数字、`-`、`_`（它同时是 config.toml 里的表名）",
        zh_hant: "Provider id `{id}` 只能用小寫字母、數字、`-`、`_`（它同時是 config.toml 裡的表名）",
        en: "Provider id `{id}` may only contain lowercase letters, digits, `-` and `_` (it is also the table name in config.toml)",
    },
    ErrProviderNameEmpty => {
        zh_hans: "Provider 名称不能为空",
        zh_hant: "Provider 名稱不能為空",
        en: "Provider name cannot be empty",
    },
    ErrBaseUrlEmpty => {
        zh_hans: "base_url 不能为空",
        zh_hant: "base_url 不能為空",
        en: "base_url cannot be empty",
    },
    ErrBaseUrlScheme => {
        zh_hans: "base_url 必须以 http:// 或 https:// 开头",
        zh_hant: "base_url 必須以 http:// 或 https:// 開頭",
        en: "base_url must start with http:// or https://",
    },
    ErrApiKeyEmpty => {
        zh_hans: "密钥不能为空：请填 env:VAR、keychain:service/account 或明文",
        zh_hant: "金鑰不能為空：請填 env:VAR、keychain:service/account 或明文",
        en: "API key cannot be empty: use env:VAR, keychain:service/account or a literal key",
    },
    ErrLiteralKeyUnacknowledged => {
        zh_hans: "明文密钥需要先勾选确认（会写进目标 agent 的配置文件）；更推荐 env:VAR 或 keychain: 引用",
        zh_hant: "明文金鑰需要先勾選確認（會寫進目標 agent 的設定檔）；更推薦 env:VAR 或 keychain: 參照",
        en: "A literal key needs explicit confirmation (it will be written into the target agent's config); env:VAR or keychain: references are preferred",
    },
    ErrEnvVarEmpty => {
        zh_hans: "env: 后面的变量名不能为空",
        zh_hant: "env: 後面的變數名不能為空",
        en: "The variable name after env: cannot be empty",
    },
    ErrKeychainFormat => {
        zh_hans: "keychain: 后面必须写成 service/account",
        zh_hant: "keychain: 後面必須寫成 service/account",
        en: "keychain: must be written as service/account",
    },

    // ------------------------------------------------------------ 供应商：模型探测
    ProbeErrScheme => {
        zh_hans: "API 请求地址必须是 http:// 或 https:// 开头",
        zh_hant: "API 請求位址必須以 http:// 或 https:// 開頭",
        en: "The API endpoint must start with http:// or https://",
    },
    ProbeErrSecret => {
        zh_hans: "读取密钥失败：{error}",
        zh_hant: "讀取金鑰失敗：{error}",
        en: "Failed to read the API key: {error}",
    },
    ProbeErrRequest => {
        zh_hans: "请求失败：{error}",
        zh_hant: "請求失敗：{error}",
        en: "Request failed: {error}",
    },
    ProbeErrAuth => {
        zh_hans: "密钥无效或没有权限（HTTP 401/403）",
        zh_hant: "金鑰無效或沒有權限（HTTP 401/403）",
        en: "Invalid key or missing permission (HTTP 401/403)",
    },
    ProbeErrNotFound => {
        zh_hans: "地址不对（HTTP 404）：API 请求地址通常要带 /v1 之类的版本段",
        zh_hant: "位址不對（HTTP 404）：API 請求位址通常要帶 /v1 之類的版本段",
        en: "Wrong URL (HTTP 404): API endpoints usually include a version segment such as /v1",
    },
    ProbeErrRateLimit => {
        zh_hans: "请求过于频繁（HTTP 429），稍后再试",
        zh_hant: "請求過於頻繁（HTTP 429），稍後再試",
        en: "Rate limited (HTTP 429); try again later",
    },
    ProbeErrStatus => {
        zh_hans: "上游返回 HTTP {code}",
        zh_hant: "上游回傳 HTTP {code}",
        en: "Upstream returned HTTP {code}",
    },
    ProbeErrNotJson => {
        zh_hans: "返回的不是 JSON，可能地址指向了网页而不是 API",
        zh_hant: "回傳的不是 JSON，可能位址指向了網頁而不是 API",
        en: "The response is not JSON; the URL may point at a web page instead of an API",
    },
    ProbeErrNoModels => {
        zh_hans: "返回里没有模型列表（期望 data[] 数组）",
        zh_hant: "回傳裡沒有模型清單（期望 data[] 陣列）",
        en: "No model list in the response (expected a data[] array)",
    },
    ProbeErrEmptyModels => {
        zh_hans: "上游没有返回任何模型",
        zh_hant: "上游沒有回傳任何模型",
        en: "The upstream returned no models",
    },
    ProbeOk => {
        zh_hans: "连接正常 · {count} 个模型",
        zh_hant: "連線正常 · {count} 個模型",
        en: "Connected · {count} models",
    },

    // ------------------------------------------------------------ 供应商：绑定与计划
    ErrProviderMissing => {
        zh_hans: "provider `{id}` 不存在",
        zh_hant: "provider `{id}` 不存在",
        en: "Provider `{id}` does not exist",
    },
    ErrSupplierMissing => {
        zh_hans: "供应商 `{id}` 不存在",
        zh_hant: "供應商 `{id}` 不存在",
        en: "Supplier `{id}` does not exist",
    },
    ErrEnvKeyOnlyBind => {
        zh_hans: "{agent} 只支持 env: 形式的密钥引用（当前是 {masked}）：该格式只能写变量名，无法绑定",
        zh_hant: "{agent} 只支援 env: 形式的金鑰參照（目前是 {masked}）：該格式只能寫變數名，無法綁定",
        en: "{agent} only supports env: key references (currently {masked}); that format writes a variable name only, so binding is impossible",
    },
    ErrEnvKeyOnlyPlan => {
        zh_hans: "{agent} 只支持 env: 形式的密钥引用（当前是 {masked}）：该格式只能写变量名",
        zh_hant: "{agent} 只支援 env: 形式的金鑰參照（目前是 {masked}）：該格式只能寫變數名",
        en: "{agent} only supports env: key references (currently {masked}); that format writes a variable name only",
    },
    ErrModelNotInProvider => {
        zh_hans: "模型 `{model}` 不在 provider「{name}」的模型列表里（可选：{models}）",
        zh_hant: "模型 `{model}` 不在 provider「{name}」的模型清單裡（可選：{models}）",
        en: "Model `{model}` is not in the model list of provider “{name}” (available: {models})",
    },
    ErrProviderNoModels => {
        zh_hans: "provider「{name}」没有登记模型，绑定时必须显式指定一个",
        zh_hant: "provider「{name}」沒有登記模型，綁定時必須明確指定一個",
        en: "Provider “{name}” has no models registered; a model must be given explicitly",
    },
    ErrSupplierReason => {
        zh_hans: "供应商「{name}」：{reason}",
        zh_hant: "供應商「{name}」：{reason}",
        en: "Supplier “{name}”: {reason}",
    },
    ErrAgentNoBinding => {
        zh_hans: "{agent} 还没有绑定 provider",
        zh_hant: "{agent} 還沒有綁定 provider",
        en: "{agent} has no provider bound yet",
    },
    ErrAgentNoSupplier => {
        zh_hans: "{agent} 还没有选择供应商",
        zh_hant: "{agent} 還沒有選擇供應商",
        en: "{agent} has not selected a supplier yet",
    },
    ErrPoolMissing => {
        zh_hans: "供应商池 `{route}` 不存在",
        zh_hant: "供應商池 `{route}` 不存在",
        en: "Supplier pool `{route}` does not exist",
    },
    ErrPoolDisabled => {
        zh_hans: "供应商池 `{route}` 已停用",
        zh_hant: "供應商池 `{route}` 已停用",
        en: "Supplier pool `{route}` is disabled",
    },
    ErrPoolInit => {
        zh_hans: "供应商池初始化失败",
        zh_hant: "供應商池初始化失敗",
        en: "Failed to initialize the supplier pool",
    },
    ErrSlotsClaudeOnly => {
        zh_hans: "模型槽位映射只对 Claude Code 生效：其它 agent 在各自界面里直接选模型",
        zh_hant: "模型槽位對應只對 Claude Code 生效：其它 agent 在各自介面裡直接選模型",
        en: "Model slot mapping only applies to Claude Code; other agents pick models directly in their own panel",
    },
    ErrClaudeNoSupplier => {
        zh_hans: "Claude Code 还没有选择供应商",
        zh_hant: "Claude Code 還沒有選擇供應商",
        en: "Claude Code has not selected a supplier yet",
    },
    ErrUnknownSlot => {
        zh_hans: "未知的模型槽位 `{slot}`：只能是 {slots}",
        zh_hant: "未知的模型槽位 `{slot}`：只能是 {slots}",
        en: "Unknown model slot `{slot}`; must be one of {slots}",
    },
    ErrSlotEmpty => {
        zh_hans: "槽位 `{slot}` 的模型名不能为空",
        zh_hant: "槽位 `{slot}` 的模型名不能為空",
        en: "The model name for slot `{slot}` cannot be empty",
    },
    ErrSlotModelMissing => {
        zh_hans: "槽位 `{slot}` 指定的模型 `{model}` 不在供应商「{name}」的模型列表里",
        zh_hant: "槽位 `{slot}` 指定的模型 `{model}` 不在供應商「{name}」的模型清單裡",
        en: "The model `{model}` for slot `{slot}` is not in the model list of supplier “{name}”",
    },
    NotePoolCleared => {
        zh_hans: "{agent} 的供应商池已清空，绑定一并移除",
        zh_hant: "{agent} 的供應商池已清空，綁定一併移除",
        en: "The supplier pool for {agent} is now empty; the binding was removed too",
    },
    GatewayPoolName => {
        zh_hans: "{agent} 供应商池",
        zh_hant: "{agent} 供應商池",
        en: "{agent} supplier pool",
    },
    PlanTitleProvider => {
        zh_hans: "写入 Provider「{name}」到 {agent}",
        zh_hant: "寫入 Provider「{name}」到 {agent}",
        en: "Write provider “{name}” into {agent}",
    },
    PlanProviderTargetExisting => {
        zh_hans: "目标 {at}（已存在，按原格式合并）",
        zh_hant: "目標 {at}（已存在，依原格式合併）",
        en: "Target {at} (exists; merging into the current format)",
    },
    PlanProviderTargetCreate => {
        zh_hans: "目标 {at}（将被创建）",
        zh_hant: "目標 {at}（將被建立）",
        en: "Target {at} (will be created)",
    },
    PlanProviderViaGateway => {
        zh_hans: "经本地网关 {url}，令牌 {token}（上游密钥不会写进 agent 配置）",
        zh_hant: "經本機閘道 {url}，權杖 {token}（上游金鑰不會寫進 agent 設定）",
        en: "Via the local gateway {url}, token {token} (upstream keys are never written into agent configs)",
    },
    PlanProviderClaudeNote => {
        zh_hans: "写 env.ANTHROPIC_BASE_URL / ANTHROPIC_MODEL / AUTH_TOKEN 与 4 个 ANTHROPIC_DEFAULT_*_MODEL 别名；其余键（hooks、permissions…）保留",
        zh_hant: "寫 env.ANTHROPIC_BASE_URL / ANTHROPIC_MODEL / AUTH_TOKEN 與 4 個 ANTHROPIC_DEFAULT_*_MODEL 別名；其餘鍵（hooks、permissions…）保留",
        en: "Writes env.ANTHROPIC_BASE_URL / ANTHROPIC_MODEL / AUTH_TOKEN and the four ANTHROPIC_DEFAULT_*_MODEL aliases; other keys (hooks, permissions…) are kept",
    },
    PlanProviderCodexNote => {
        zh_hans: "写 model / model_provider 与 [model_providers.{id}]（含 wire_api = {wire}），其它 provider 子表与注释保留",
        zh_hant: "寫 model / model_provider 與 [model_providers.{id}]（含 wire_api = {wire}），其它 provider 子表與註解保留",
        en: "Writes model / model_provider and [model_providers.{id}] (with wire_api = {wire}); other provider tables and comments are kept",
    },
    PlanProviderOpencodeNote => {
        zh_hans: "写 provider.{id} 的 npm（{npm}）/ name / options / models；其它 provider 与 mcp 段保留。写完请在 OpenCode 里确认已选中该模型",
        zh_hant: "寫 provider.{id} 的 npm（{npm}）/ name / options / models；其它 provider 與 mcp 段保留。寫完請在 OpenCode 裡確認已選中該模型",
        en: "Writes provider.{id} with npm ({npm}) / name / options / models; other providers and the mcp block are kept. Confirm the model is selected in OpenCode afterwards",
    },
    PlanProviderJsonKeysNote => {
        zh_hans: "只改 {base} 与 {key}，其它 provider 与配置项保留",
        zh_hant: "只改 {base} 與 {key}，其它 provider 與設定項保留",
        en: "Only {base} and {key} are changed; other providers and settings are kept",
    },
    PlanProviderUpToDate => {
        zh_hans: "配置已是最新，无需改动",
        zh_hant: "設定已是最新，無需改動",
        en: "The config is already up to date",
    },
    RiskProviderLiteralKey => {
        zh_hans: "该 provider 用的是明文密钥：确认后会把密钥写进目标配置文件",
        zh_hant: "該 provider 用的是明文金鑰：確認後會把金鑰寫進目標設定檔",
        en: "This provider uses a literal key: after confirming, the key will be written into the target config file",
    },
    RiskProviderDefaultModel => {
        zh_hans: "会把「{model}」设为 {agent} 的默认模型",
        zh_hant: "會把「{model}」設為 {agent} 的預設模型",
        en: "“{model}” will become the default model for {agent}",
    },
    // ------------------------------------------------------------ 用量统计
    UsageRangeToday => {
        zh_hans: "当日",
        zh_hant: "當日",
        en: "Today",
    },
    UsageRangeLast24h => {
        zh_hans: "24 小时",
        zh_hant: "24 小時",
        en: "Last 24 hours",
    },
    UsageRangeDays7 => {
        zh_hans: "7 天",
        zh_hant: "7 天",
        en: "7 days",
    },
    UsageRangeDays15 => {
        zh_hans: "15 天",
        zh_hant: "15 天",
        en: "15 days",
    },
    UsageRangeDays30 => {
        zh_hans: "30 天",
        zh_hant: "30 天",
        en: "30 days",
    },
    UsageRangeAll => {
        zh_hans: "全部",
        zh_hant: "全部",
        en: "All time",
    },
    UsageUnregisteredProvider => {
        zh_hans: "未登记",
        zh_hant: "未登記",
        en: "Unregistered",
    },
    UsageUnknownModel => {
        zh_hans: "未知模型",
        zh_hant: "未知模型",
        en: "Unknown model",
    },
    UsageTotalsNote => {
        zh_hans: "合计 = 非缓存输入 + 命中缓存 + 输出（新建缓存本机恒为 0，不单列）；按本地时区切分时间；CodeBuddy / WorkBuddy 日志中的积分（credit）字段不计入",
        zh_hant: "合計 = 非快取輸入 + 命中快取 + 輸出（新建快取本機恆為 0，不單列）；依本機時區切分時間；CodeBuddy / WorkBuddy 記錄中的積分（credit）欄位不計入",
        en: "Total = uncached input + cache hits + output (cache writes are always 0 here and are not listed separately); buckets use your local time zone; credit fields in CodeBuddy / WorkBuddy logs are excluded",
    },
    UsageFormatCodex => {
        zh_hans: "Codex 事件流",
        zh_hant: "Codex 事件流",
        en: "Codex event stream",
    },
    UsageFormatOpenAi => {
        zh_hans: "OpenAI 风格 JSONL",
        zh_hant: "OpenAI 風格 JSONL",
        en: "OpenAI-style JSONL",
    },
    UsageFormatCodebuddyIde => {
        zh_hans: "CodeBuddy IDE 索引",
        zh_hant: "CodeBuddy IDE 索引",
        en: "CodeBuddy IDE index",
    },
    UsageFormatNone => {
        zh_hans: "无用量字段",
        zh_hant: "無用量欄位",
        en: "No usage fields",
    },
    UsageSourceChatgptDesktop => {
        zh_hans: "ChatGPT / OpenAI 桌面端",
        zh_hant: "ChatGPT / OpenAI 桌面版",
        en: "ChatGPT / OpenAI desktop",
    },
    UsageWhyHermes => {
        zh_hans: "请求日志（sessions/request_dump_*.json）只存请求与响应，不含 usage 字段",
        zh_hant: "請求記錄（sessions/request_dump_*.json）只存請求和回應，不含 usage 欄位",
        en: "Request logs (sessions/request_dump_*.json) only contain requests and responses, with no usage fields",
    },
    UsageWhyOpencode => {
        zh_hans: "opencode.db 里没有用量表（token 命中的都是 access_token）",
        zh_hant: "opencode.db 裡沒有用量表（token 命中的都是 access_token）",
        en: "opencode.db has no usage table (token hits are all access_token)",
    },
    UsageWhyGemini => {
        zh_hans: "本机未安装，且 chats 日志的用量字段未实测确认",
        zh_hant: "本機未安裝，且 chats 記錄的用量欄位未實測確認",
        en: "Not installed here, and the usage fields in chats logs are unverified",
    },
    UsageWhyTrae => {
        zh_hans: "VSCode 派生，没有公开的本地用量约定",
        zh_hant: "VSCode 衍生，沒有公開的本機用量約定",
        en: "VSCode-derived with no public local usage convention",
    },
    UsageWhyTraeCn => {
        zh_hans: "同 Trae，未确认本地用量文件",
        zh_hant: "同 Trae，未確認本機用量檔案",
        en: "Same as Trae; local usage files are unconfirmed",
    },
    UsageWhyCursor => {
        zh_hans: "用量只在账号后台，本地无明细",
        zh_hant: "用量只在帳號後台，本機無明細",
        en: "Usage only lives in the account dashboard; nothing local",
    },
    UsageWhyCopilot => {
        zh_hans: "session-store.db 不含用量",
        zh_hant: "session-store.db 不含用量",
        en: "session-store.db contains no usage data",
    },
    UsageWhyClaudeDesktop => {
        zh_hans: "桌面端不落 token 明细",
        zh_hant: "桌面版不落 token 明細",
        en: "The desktop app keeps no token details",
    },
    UsageWhyChatgptDesktop => {
        zh_hans: "与 Codex 共用 ~/.codex，用量已计入 Codex",
        zh_hant: "與 Codex 共用 ~/.codex，用量已計入 Codex",
        en: "Shares ~/.codex with Codex, so its usage is already counted there",
    },
    UsageWhyManus => {
        zh_hans: "云端 agent，本地无用法记录",
        zh_hant: "雲端 agent，本機無使用記錄",
        en: "Cloud agent with no local usage records",
    },
    UsageWhyPi => {
        zh_hans: "本地无用量文件约定",
        zh_hant: "本機無用量檔案約定",
        en: "No local usage file convention",
    },
    UsageWhyOpenwork => {
        zh_hans: "本地无用量文件约定",
        zh_hant: "本機無用量檔案約定",
        en: "No local usage file convention",
    },
    ErrBadKeyPath => {
        zh_hans: "非法的配置键路径：{path}",
        zh_hant: "非法的設定鍵路徑：{path}",
        en: "Invalid config key path: {path}",
    },
    CatalogInstallNoGuide => {
        zh_hans: "自定义平台没有内置安装指引，请参考该产品的官方文档",
        zh_hant: "自訂平台沒有內建安裝指引，請參考該產品的官方文件",
        en: "Custom platforms have no built-in install guide; see the product's official docs",
    },

    // ------------------------------------------------ 常见 Agent 目录：产品名与厂商名
    // 产品名里带中文限定词（国内版 / 国际版 / 桌面）时要跟着界面语言走；
    // 纯英文产品名（Claude Code、Trae…）留在静态表里，不进词典。
    CatalogNameChatgptDesktop => {
        zh_hans: "ChatGPT / Codex 桌面",
        zh_hant: "ChatGPT / Codex 桌面版",
        en: "ChatGPT / Codex desktop",
    },
    CatalogNameWorkbuddy => {
        zh_hans: "WorkBuddy（国内版）",
        zh_hant: "WorkBuddy（國內版）",
        en: "WorkBuddy (China)",
    },
    CatalogNameWorkbuddyAi => {
        zh_hans: "WorkBuddy AI（国际版）",
        zh_hant: "WorkBuddy AI（國際版）",
        en: "WorkBuddy AI (International)",
    },
    CatalogNameTrae => {
        zh_hans: "Trae（国际版）",
        zh_hant: "Trae（國際版）",
        en: "Trae (International)",
    },
    CatalogNameTraeCn => {
        zh_hans: "Trae CN（国内版）",
        zh_hant: "Trae CN（國內版）",
        en: "Trae CN (China)",
    },
    CatalogVendorTencent => {
        zh_hans: "腾讯云",
        zh_hant: "騰訊雲",
        en: "Tencent Cloud",
    },
    CatalogVendorBytedance => {
        zh_hans: "字节跳动",
        zh_hant: "位元組跳動",
        en: "ByteDance",
    },
    CatalogVendorInternal => {
        zh_hans: "内部",
        zh_hant: "內部",
        en: "Internal",
    },

    // ------------------------------------------------------------ 链接与文件系统
    ErrLinkExists => {
        zh_hans: "{path} 已存在，拒绝覆盖",
        zh_hant: "{path} 已存在，拒絕覆寫",
        en: "{path} already exists; refusing to overwrite",
    },
    ErrSymlinkUnsupported => {
        zh_hans: "{path} 无法建立目录符号链接（Windows 需开发者模式或管理员权限）: {error}",
        zh_hant: "{path} 無法建立目錄符號連結（Windows 需開發人員模式或系統管理員權限）: {error}",
        en: "Cannot create a directory symlink at {path} (Windows needs developer mode or admin rights): {error}",
    },
    ErrJunctionNotSupported => {
        zh_hans: "目录联接仅 Windows 支持（{path}）",
        zh_hant: "目錄聯結僅 Windows 支援（{path}）",
        en: "Directory junctions are Windows-only ({path})",
    },
    ErrWalkFailed => {
        zh_hans: "遍历失败: {error}",
        zh_hant: "遍歷失敗: {error}",
        en: "Directory walk failed: {error}",
    },
    ErrStripPrefix => {
        zh_hans: "路径剥离失败",
        zh_hant: "路徑剝離失敗",
        en: "Failed to strip the path prefix",
    },
    ErrCopyLink => {
        zh_hans: "复制链接失败 {from} -> {to}",
        zh_hant: "複製連結失敗 {from} -> {to}",
        en: "Failed to copy the link {from} -> {to}",
    },
    ErrTrashFailed => {
        zh_hans: "移入回收站失败 {path}: {error}",
        zh_hant: "移入資源回收筒失敗 {path}: {error}",
        en: "Failed to move to the trash {path}: {error}",
    },

    // ------------------------------------------------------------ 路径门禁与读写
    ErrPathOutsideRoots => {
        zh_hans: "{path} 不在受管根目录内（允许: {roots}）",
        zh_hant: "{path} 不在受管根目錄內（允許: {roots}）",
        en: "{path} is outside the managed roots (allowed: {roots})",
    },
    ErrPathNoParent => {
        zh_hans: "{path} 没有父目录",
        zh_hant: "{path} 沒有父目錄",
        en: "{path} has no parent directory",
    },
    ErrFileTooLarge => {
        zh_hans: "{path} 超过 {limit} 字节上限，已跳过",
        zh_hant: "{path} 超過 {limit} 位元組上限，已跳過",
        en: "{path} exceeds the {limit}-byte limit and was skipped",
    },
    ErrNotUtf8 => {
        zh_hans: "{path} 不是合法 UTF-8",
        zh_hant: "{path} 不是合法 UTF-8",
        en: "{path} is not valid UTF-8",
    },

    // ------------------------------------------------------------ MCP 合并与密钥
    ErrMcpJsonTopNotObject => {
        zh_hans: "目标 JSON 顶层不是对象，已拒绝改写以避免破坏配置",
        zh_hant: "目標 JSON 頂層不是物件，已拒絕改寫以避免破壞設定",
        en: "The target JSON is not a top-level object; refusing to rewrite it to avoid breaking the config",
    },
    ErrMcpKeyNotObject => {
        zh_hans: "键路径 {key} 上不是对象，已拒绝改写",
        zh_hant: "鍵路徑 {key} 上不是物件，已拒絕改寫",
        en: "The value at key path {key} is not an object; refusing to rewrite it",
    },
    ErrMcpNodeNotObject => {
        zh_hans: "MCP 节点不是对象",
        zh_hant: "MCP 節點不是物件",
        en: "The MCP node is not an object",
    },
    ErrTomlParseFailed => {
        zh_hans: "解析 TOML 失败: {error}",
        zh_hant: "解析 TOML 失敗: {error}",
        en: "Failed to parse TOML: {error}",
    },
    ErrYamlInvalidAfterWrite => {
        zh_hans: "写入后的 YAML 无法解析，已中止: {error}",
        zh_hant: "寫入後的 YAML 無法解析，已中止: {error}",
        en: "The YAML written is not parseable; aborted: {error}",
    },
    ErrSecretEnvUnset => {
        zh_hans: "环境变量 {var} 未设置，无法写入密钥（请先 export {var}）",
        zh_hant: "環境變數 {var} 未設定，無法寫入金鑰（請先 export {var}）",
        en: "Environment variable {var} is not set, so the key cannot be written (export {var} first)",
    },
    ErrSecretLiteralUnconfirmed => {
        zh_hans: "明文密钥未经确认，已拒绝写入（请在 UI 中确认风险或改用 env:/keychain: 引用）",
        zh_hant: "明文金鑰未經確認，已拒絕寫入（請在 UI 中確認風險或改用 env:/keychain: 參照）",
        en: "The literal key was not confirmed; write refused (confirm the risk in the UI or use an env:/keychain: reference)",
    },
    ErrSecretSecurityCall => {
        zh_hans: "调用 security 失败: {error}",
        zh_hant: "呼叫 security 失敗: {error}",
        en: "Failed to run security: {error}",
    },
    ErrSecretUserMismatch => {
        zh_hans: "凭据 {service} 里存的用户名是 {stored}，与期望的 {account} 不一致，请确认引用是否写错",
        zh_hant: "憑證 {service} 裡存的使用者名稱是 {stored}，與期望的 {account} 不一致，請確認參照是否寫錯",
        en: "Credential {service} stores user {stored}, which does not match {account}; check the reference",
    },
    ErrSecretEmptyValue => {
        zh_hans: "凭据 {service}/{account} 存在但内容为空，请重新添加",
        zh_hant: "憑證 {service}/{account} 存在但內容為空，請重新加入",
        en: "Credential {service}/{account} exists but is empty; add it again",
    },
    ErrSecretNoKeychain => {
        zh_hans: "当前平台尚未接入系统钥匙串（{service}/{account}），请改用 env:VAR 引用",
        zh_hant: "目前平台尚未接入系統鑰匙圈（{service}/{account}），請改用 env:VAR 參照",
        en: "This platform has no system keychain integration yet ({service}/{account}); use an env:VAR reference",
    },
    ErrSecretMissing => {
        zh_hans: "系统凭据库中没有 {service}；可用 `cmdkey /generic:{service} /user:{account} /pass:<密钥>` 添加后重试，或改用 env:VAR 引用",
        zh_hant: "系統憑證庫中沒有 {service}；可用 `cmdkey /generic:{service} /user:{account} /pass:<金鑰>` 加入後重試，或改用 env:VAR 參照",
        en: "The system credential store has no {service}; add it with `cmdkey /generic:{service} /user:{account} /pass:<key>` and retry, or use an env:VAR reference",
    },
    ErrSecretDomainPassword => {
        zh_hans: "凭据 {service} 是「域密码」类型（`cmdkey /add:` 不带 `/generic` 时建的就是它），Windows 不保存可读的密码；请用 `cmdkey /generic:{service} /user:{account} /pass:<密钥>` 重新添加",
        zh_hant: "憑證 {service} 是「網域密碼」類型（`cmdkey /add:` 不帶 `/generic` 時建的就是它），Windows 不保存可讀的密碼；請用 `cmdkey /generic:{service} /user:{account} /pass:<金鑰>` 重新加入",
        en: "Credential {service} is a domain-password entry (`cmdkey /add:` without `/generic` creates one) and Windows does not store a readable password; re-add it with `cmdkey /generic:{service} /user:{account} /pass:<key>`",
    },
    LabelInstructionFile => {
        zh_hans: "指令文件",
        zh_hant: "指令檔",
        en: "Instruction file",
    },

    // ------------------------------------------------------------ 指令文件
    ErrManagedBlockTooLarge => {
        zh_hans: "受控区块内容超过 {limit} 字节上限",
        zh_hant: "受控區塊內容超過 {limit} 位元組上限",
        en: "The managed block exceeds the {limit}-byte limit",
    },
    ErrInstructionTooLarge => {
        zh_hans: "{path} 超过 {limit} 字节上限，拒绝改写",
        zh_hant: "{path} 超過 {limit} 位元組上限，拒絕改寫",
        en: "{path} exceeds the {limit}-byte limit; refusing to rewrite it",
    },
    ErrFileChangedSincePlan => {
        zh_hans: "文件在计划生成后被外部修改，已中止写入（请重新扫描后再试）",
        zh_hant: "檔案在計畫產生後被外部修改，已中止寫入（請重新掃描後再試）",
        en: "The file changed externally after the plan was built; write aborted (rescan and try again)",
    },
    StatusFileMissing => {
        zh_hans: "文件不存在（AgentHub 可创建）",
        zh_hant: "檔案不存在（AgentHub 可建立）",
        en: "File missing (AgentHub can create it)",
    },

    // ------------------------------------------------------------ 项目作用域
    ErrProjectRootRelative => {
        zh_hans: "项目根必须是绝对路径: {path}",
        zh_hant: "專案根必須是絕對路徑: {path}",
        en: "The project root must be an absolute path: {path}",
    },

    // ------------------------------------------------------------ 变更感知
    WatchDeltaSummary => {
        zh_hans: "新增 {added} · 删除 {removed} · 变更 {changed}",
        zh_hant: "新增 {added} · 刪除 {removed} · 變更 {changed}",
        en: "{added} added · {removed} removed · {changed} changed",
    },

    // ------------------------------------------------------------ 去重原因
    DedupeReasonStoreExists => {
        zh_hans: "中央仓库已有版本，可将其余副本统一为链接",
        zh_hant: "中央倉庫已有版本，可將其餘副本統一為連結",
        en: "The hub store already has a copy; the other copies can become links",
    },
    DedupeReasonIdentical => {
        zh_hans: "多份内容完全一致，收敛为单份即可释放空间",
        zh_hant: "多份內容完全一致，收斂為單份即可釋放空間",
        en: "All copies are identical; collapsing them frees space",
    },
    DedupeReasonDrift => {
        zh_hans: "多份内容不一致（版本漂移），需选定基线后统一",
        zh_hant: "多份內容不一致（版本漂移），需選定基準後統一",
        en: "Copies differ (version drift); pick a baseline before unifying",
    },

    // ------------------------------------------------------------ 链接状态与来源徽标
    LabelLinkSymlink => {
        zh_hans: "软链接",
        zh_hant: "軟連結",
        en: "Symlink",
    },
    LabelLinkJunction => {
        zh_hans: "目录联接",
        zh_hant: "目錄聯結",
        en: "Junction",
    },
    LabelLinkHardlink => {
        zh_hans: "硬链接",
        zh_hant: "硬連結",
        en: "Hardlink",
    },
    LabelLinkRealDir => {
        zh_hans: "独立副本",
        zh_hant: "獨立副本",
        en: "Standalone copy",
    },
    LabelLinkBroken => {
        zh_hans: "链接失效",
        zh_hant: "連結失效",
        en: "Broken link",
    },
    LabelLinkForeign => {
        zh_hans: "外部链接",
        zh_hant: "外部連結",
        en: "External link",
    },
    LabelSourceStore => {
        zh_hans: "中央仓库",
        zh_hant: "中央倉庫",
        en: "Hub store",
    },
    LabelSourceAgentCopy => {
        zh_hans: "agent 副本",
        zh_hant: "agent 副本",
        en: "Agent copy",
    },
    LabelSourceReadonly => {
        zh_hans: "只读内置",
        zh_hant: "唯讀內建",
        en: "Read-only built-in",
    },
    ErrMarketMalicious => {
        zh_hans: "{name} 已被技能市场标记为恶意，AgentHub 拒绝安装",
        zh_hant: "{name} 已被技能市場標記為惡意，AgentHub 拒絕安裝",
        en: "{name} is flagged as malicious by the skill market; AgentHub refuses to install it",
    },

    // ------------------------------------------------------------ 体检（doctor）
    DoctorBrokenLink => {
        zh_hans: "链接失效（目标不存在）: {target}",
        zh_hant: "連結失效（目標不存在）: {target}",
        en: "Broken link (target missing): {target}",
    },
    DoctorAgentLocalSuffix => {
        zh_hans: "（已标记为 agent 独有，不参与自动修复）",
        zh_hant: "（已標記為 agent 獨有，不參與自動修復）",
        en: " (marked as agent-only, so auto-repair skips it)",
    },
    DoctorForeignLink => {
        zh_hans: "外部链接，AgentHub 不会改写它",
        zh_hant: "外部連結，AgentHub 不會改寫它",
        en: "External link; AgentHub will not rewrite it",
    },
    DoctorDuplicateCopy => {
        zh_hans: "存在同名多份副本，可一键收敛到中央仓库",
        zh_hant: "存在同名多份副本，可一鍵收斂到中央倉庫",
        en: "Multiple copies share this name and can be collapsed into the hub store",
    },
    DoctorDuplicateGroup => {
        zh_hans: "技能 {name} 有 {copies} 份拷贝，可释放 {size}",
        zh_hant: "技能 {name} 有 {copies} 份複本，可釋放 {size}",
        en: "Skill {name} has {copies} copies; {size} can be freed",
    },
    DoctorStoreMissing => {
        zh_hans: "中央仓库目录不存在",
        zh_hant: "中央倉庫目錄不存在",
        en: "The hub store directory does not exist",
    },

    // ------------------------------------------------------------ 项目级技能
    PlanTitleEnableProjectSkills => {
        zh_hans: "启用项目级技能",
        zh_hant: "啟用專案級技能",
        en: "Enable project-level skills",
    },
    PlanSkipNotInStore => {
        zh_hans: "中央仓库没有 {name}，已跳过",
        zh_hant: "中央倉庫沒有 {name}，已跳過",
        en: "{name} is not in the hub store; skipped",
    },

    // ------------------------------------------------------------ 设置页校验
    ErrStoreRootEmpty => {
        zh_hans: "仓库目录不能为空",
        zh_hant: "倉庫目錄不能為空",
        en: "The store directory cannot be empty",
    },
    ErrBackupKeepRange => {
        zh_hans: "备份保留份数必须在 1..=100 之间",
        zh_hant: "備份保留份數必須在 1..=100 之間",
        en: "Backup retention must be between 1 and 100",
    },
    ErrCustomAgentIdEmpty => {
        zh_hans: "自定义平台的 id 不能为空",
        zh_hant: "自訂平台的 id 不能為空",
        en: "A custom platform id cannot be empty",
    },
    ErrCustomAgentIdPrefix => {
        zh_hans: "自定义平台 id `{id}` 不要带 `custom:` 前缀",
        zh_hant: "自訂平台 id `{id}` 不要帶 `custom:` 前綴",
        en: "Custom platform id `{id}` must not use the `custom:` prefix",
    },
    ErrCustomAgentIdConflict => {
        zh_hans: "自定义平台 id `{id}` 与内置平台冲突",
        zh_hant: "自訂平台 id `{id}` 與內建平台衝突",
        en: "Custom platform id `{id}` conflicts with a built-in platform",
    },
    ErrCustomAgentIdDuplicate => {
        zh_hans: "自定义平台 id `{id}` 重复",
        zh_hant: "自訂平台 id `{id}` 重複",
        en: "Custom platform id `{id}` is duplicated",
    },
    ErrCustomAgentNoTarget => {
        zh_hans: "自定义平台 `{id}` 至少要填一个目标（技能目录 / MCP 文件 / 指令文件）",
        zh_hant: "自訂平台 `{id}` 至少要填一個目標（技能目錄 / MCP 檔案 / 指令檔案）",
        en: "Custom platform `{id}` needs at least one target (skills dir / MCP file / instruction file)",
    },
    ErrUnknownBuiltinAgent => {
        zh_hans: "未知的内置 agent：{agent}",
        zh_hant: "未知的內建 agent：{agent}",
        en: "Unknown built-in agent: {agent}",
    },
    ErrUnknownMarketSource => {
        zh_hans: "未知的技能市场数据源：{value}（可选 skillhub / clawhub）",
        zh_hant: "未知的技能市場資料來源：{value}（可選 skillhub / clawhub）",
        en: "Unknown skill market source: {value} (use skillhub or clawhub)",
    },

    // ------------------------------------------------------------ 价格表
    PricingNote => {
        zh_hans: "单价为每 100 万 token；未定价的模型显示「未定价」而不是 0；从公开源导入只在你点击时联网",
        zh_hant: "單價為每 100 萬 token；未定價的模型顯示「未定價」而不是 0；從公開來源匯入只在你點擊時連網",
        en: "Prices are per million tokens; unpriced models show as “unpriced” rather than 0; importing from a public source only goes online when you click",
    },
    ErrPriceInvalid => {
        zh_hans: "单价必须是不小于 0 的有限数",
        zh_hant: "單價必須是不小於 0 的有限數",
        en: "The price must be a finite number greater than or equal to 0",
    },
    ErrPriceTooHigh => {
        zh_hans: "单价 {value} 超过上限（每百万 token {max}），请检查单位",
        zh_hant: "單價 {value} 超過上限（每百萬 token {max}），請檢查單位",
        en: "Price {value} exceeds the limit ({max} per million tokens); check the unit",
    },
    ErrModelNameEmpty => {
        zh_hans: "模型名不能为空",
        zh_hant: "模型名不能為空",
        en: "The model name cannot be empty",
    },
    ErrPricingImportStatus => {
        zh_hans: "{source} 返回 HTTP {status}，暂时无法导入",
        zh_hant: "{source} 回傳 HTTP {status}，暫時無法匯入",
        en: "{source} returned HTTP {status}; import is unavailable right now",
    },
    ErrPricingNoFields => {
        zh_hans: "{source} 的响应里没有识别到可用的价格字段",
        zh_hant: "{source} 的回應裡沒有辨識到可用的價格欄位",
        en: "No usable price fields were found in the response from {source}",
    },
    ErrUnknownPriceSource => {
        zh_hans: "未知的价格源：{value}",
        zh_hant: "未知的價格來源：{value}",
        en: "Unknown price source: {value}",
    },

    // ------------------------------------------------------------ 命令行输出
    CliAbout => {
        zh_hans: "AgentHub：多 agent 的 skill / MCP / 指令文件统一管理（与 GUI 共用核心）",
        zh_hant: "AgentHub：多 agent 的 skill / MCP / 指令檔統一管理（與 GUI 共用核心）",
        en: "AgentHub: one place for skills, MCP and instruction files across agents (shares the GUI core)",
    },
    CliScanDuration => {
        zh_hans: "扫描耗时 {ms} ms",
        zh_hant: "掃描耗時 {ms} ms",
        en: "Scan took {ms} ms",
    },
    CliScanSkills => {
        zh_hans: "技能 {total} 个（仓库 {store} / agent 目录 {agent} / 只读 {readonly}）",
        zh_hant: "技能 {total} 個（倉庫 {store} / agent 目錄 {agent} / 唯讀 {readonly}）",
        en: "{total} skills (store {store} / agent dirs {agent} / read-only {readonly})",
    },
    CliScanLinks => {
        zh_hans: "已链接 {linked} · 独立副本 {copies} · 失效 {broken} · 跨 agent 共享 {shared} · 可省 {wasted}",
        zh_hant: "已連結 {linked} · 獨立副本 {copies} · 失效 {broken} · 跨 agent 共用 {shared} · 可省 {wasted}",
        en: "linked {linked} · standalone {copies} · broken {broken} · shared {shared} · reclaimable {wasted}",
    },
    CliAgentsHeader => {
        zh_hans: "== agent 检出 ==",
        zh_hant: "== agent 偵測 ==",
        en: "== Agent detection ==",
    },
    CliInstalled => {
        zh_hans: "已安装",
        zh_hant: "已安裝",
        en: "Installed",
    },
    CliNotDetected => {
        zh_hans: "未检出",
        zh_hant: "未偵測到",
        en: "Not detected",
    },
    CliDuplicatesHeader => {
        zh_hans: "== 重复技能 Top ==",
        zh_hant: "== 重複技能 Top ==",
        en: "== Top duplicate skills ==",
    },
    CliDuplicateRow => {
        zh_hans: "  {name} {count} 份  可省 {size} {state}",
        zh_hant: "  {name} {count} 份  可省 {size} {state}",
        en: "  {name} x{count}  reclaim {size} {state}",
    },
    CliIdentical => {
        zh_hans: "内容一致",
        zh_hant: "內容一致",
        en: "identical",
    },
    CliDrifting => {
        zh_hans: "内容不一致",
        zh_hant: "內容不一致",
        en: "drifted",
    },
    CliBrokenLinksHeader => {
        zh_hans: "== 失效链接 ==",
        zh_hant: "== 失效連結 ==",
        en: "== Broken links ==",
    },
    CliPlanSummary => {
        zh_hans: "计划 #{id} · {title} · 共 {count} 个操作",
        zh_hant: "計畫 #{id} · {title} · 共 {count} 個操作",
        en: "Plan #{id} · {title} · {count} operations",
    },
    CliNeedsConfirm => {
        zh_hans: "{pad}· 需确认：安装方式或配置路径尚未被官方资料确证",
        zh_hant: "{pad}· 需確認：安裝方式或設定路徑尚未被官方資料確證",
        en: "{pad}· Needs confirmation: the install method or config path is not officially verified",
    },
    CliBlocked => {
        zh_hans: "{pad}· 不可受管：{note}",
        zh_hant: "{pad}· 不可受管：{note}",
        en: "{pad}· Cannot be managed: {note}",
    },
    CliProvidersEmpty => {
        zh_hans: "还没有登记供应商。用 `agenthub provider add <id> --name … --base-url … --key env:VAR` 加一条。",
        zh_hant: "還沒有登記供應商。用 `agenthub provider add <id> --name … --base-url … --key env:VAR` 加一筆。",
        en: "No suppliers registered yet. Add one with `agenthub provider add <id> --name … --base-url … --key env:VAR`.",
    },
    CliProvidersHeader => {
        zh_hans: "== 供应商 ==",
        zh_hant: "== 供應商 ==",
        en: "== Suppliers ==",
    },
    CliProtocolUndeclared => {
        zh_hans: "未声明",
        zh_hant: "未宣告",
        en: "undeclared",
    },
    CliModelsLine => {
        zh_hans: "{pad} 模型：{models}{limits}",
        zh_hant: "{pad} 模型：{models}{limits}",
        en: "{pad} Models: {models}{limits}",
    },
    CliBindingsHeader => {
        zh_hans: "== 绑定 ==",
        zh_hant: "== 綁定 ==",
        en: "== Bindings ==",
    },
    CliBindingUnsupported => {
        zh_hans: "{name} 暂不支持写入：{reason}",
        zh_hant: "{name} 暫不支援寫入：{reason}",
        en: "{name} writing is not supported yet: {reason}",
    },
    CliConfigFormatMissing => {
        zh_hans: "配置格式未接入",
        zh_hant: "設定格式未接入",
        en: "config format not supported",
    },
    CliDefaultBadge => {
        zh_hans: "  [默认]",
        zh_hant: "  [預設]",
        en: "  [default]",
    },
    CliViaGateway => {
        zh_hans: "  [经网关 {route}]",
        zh_hant: "  [經閘道 {route}]",
        en: "  [via gateway {route}]",
    },
    CliNoRouteSelected => {
        zh_hans: "未选路由",
        zh_hant: "未選路由",
        en: "no route",
    },
    CliUnbound => {
        zh_hans: "{name} 未绑定",
        zh_hant: "{name} 未綁定",
        en: "{name} not bound",
    },
    CliDataDir => {
        zh_hans: "数据目录   {path}",
        zh_hant: "資料目錄   {path}",
        en: "Data dir   {path}",
    },
    CliStoreLine => {
        zh_hans: "中央仓库   {path}（{skills} 个技能 / {size}）",
        zh_hant: "中央倉庫   {path}（{skills} 個技能 / {size}）",
        en: "Hub store   {path} ({skills} skills / {size})",
    },
    CliStrategyManual => {
        zh_hans: "{label}（手动）",
        zh_hant: "{label}（手動）",
        en: "{label} (manual)",
    },
    CliStrategyDefault => {
        zh_hans: "跟随平台默认（{label}）",
        zh_hant: "跟隨平台預設（{label}）",
        en: "Platform default ({label})",
    },
    CliStrategyLine => {
        zh_hans: "链接策略   {value}",
        zh_hant: "連結策略   {value}",
        en: "Link mode   {value}",
    },
    CliBackupKeep => {
        zh_hans: "备份保留   {count} 个批次",
        zh_hant: "備份保留   {count} 個批次",
        en: "Backups   keep {count} batches",
    },
    CliDedupeMode => {
        zh_hans: "去重       {mode}{suffix}",
        zh_hant: "去重       {mode}{suffix}",
        en: "Dedupe   {mode}{suffix}",
    },
    CliUseTrash => {
        zh_hans: "使用系统回收站",
        zh_hant: "使用系統資源回收筒",
        en: "system trash",
    },
    CliUseBackupDir => {
        zh_hans: "移入 AgentHub 备份目录",
        zh_hant: "移入 AgentHub 備份目錄",
        en: "AgentHub backup dir",
    },
    CliBackupConfigSuffix => {
        zh_hans: " · 改写配置前备份",
        zh_hant: " · 改寫設定前備份",
        en: " · backs up configs before writing",
    },
    CliExcluded => {
        zh_hans: "排除路径   {paths}",
        zh_hant: "排除路徑   {paths}",
        en: "Excluded   {paths}",
    },
    CliAgentPathsHeader => {
        zh_hans: "== agent 生效路径 ==",
        zh_hant: "== agent 生效路徑 ==",
        en: "== Effective agent paths ==",
    },
    CliOverrideBadge => {
        zh_hans: "  [有覆盖]",
        zh_hant: "  [有覆寫]",
        en: "  [overridden]",
    },
    CliConfigDir => {
        zh_hans: "  配置目录",
        zh_hant: "  設定目錄",
        en: "  Config dir",
    },
    CliSkillsDir => {
        zh_hans: "  技能目录",
        zh_hant: "  技能目錄",
        en: "  Skills dir",
    },
    CliCompatDir => {
        zh_hans: "  兼容目录",
        zh_hant: "  相容目錄",
        en: "  Compat dir",
    },
    CliMcpFile => {
        zh_hans: "  MCP 文件",
        zh_hant: "  MCP 檔",
        en: "  MCP file",
    },
    CliInstructionsFile => {
        zh_hans: "  指令文件",
        zh_hant: "  指令檔",
        en: "  Instruction file",
    },
    CliCustomAgentsHeader => {
        zh_hans: "== 自定义平台 ==",
        zh_hant: "== 自訂平台 ==",
        en: "== Custom platforms ==",
    },
    CliPathMissing => {
        zh_hans: "  (不存在)",
        zh_hant: "  (不存在)",
        en: "  (missing)",
    },
    CliNoMcpServers => {
        zh_hans: "没有发现 MCP 服务器定义",
        zh_hant: "沒有發現 MCP 伺服器定義",
        en: "No MCP server definitions found",
    },
    CliDisabledBadge => {
        zh_hans: "  (已禁用)",
        zh_hant: "  (已停用)",
        en: "  (disabled)",
    },
    CliPartiallyUnpriced => {
        zh_hans: "（部分模型未定价）",
        zh_hant: "（部分模型未定價）",
        en: "(some models unpriced)",
    },
    CliUnpriced => {
        zh_hans: "未定价",
        zh_hant: "未定價",
        en: "unpriced",
    },
    CliRangeFrom => {
        zh_hans: "{label} · {start} 起",
        zh_hant: "{label} · {start} 起",
        en: "{label} · since {start}",
    },
    CliRangeEarliest => {
        zh_hans: "{label}（最早一条起）",
        zh_hant: "{label}（最早一筆起）",
        en: "{label} (from the earliest record)",
    },
    CliUsageHeader => {
        zh_hans: "== Token 用量（{range}）==",
        zh_hant: "== Token 用量（{range}）==",
        en: "== Token usage ({range}) ==",
    },
    CliUsageEmpty => {
        zh_hans: "这段时间没有可解析的用量记录。",
        zh_hant: "這段時間沒有可解析的用量記錄。",
        en: "No parseable usage records in this period.",
    },
    CliUsageTotalsHead => {
        zh_hans: "非缓存输入 {input} 命中缓存 {cache_read} 新建缓存 {cache_write} 输出 {output}",
        zh_hant: "非快取輸入 {input} 命中快取 {cache_read} 新建快取 {cache_write} 輸出 {output}",
        en: "uncached in {input} cache hit {cache_read} cache write {cache_write} output {output}",
    },
    CliUsageTotalsRow => {
        zh_hans: "合计       {total} 调用 {calls} 估算 {cost}",
        zh_hant: "合計       {total} 呼叫 {calls} 估算 {cost}",
        en: "Total       {total} calls {calls} est. {cost}",
    },
    CliUsageGroupHeader => {
        zh_hans: "== 按{kind} ==",
        zh_hant: "== 依{kind} ==",
        en: "== By {kind} ==",
    },
    CliGranularityHour => {
        zh_hans: "小时",
        zh_hant: "小時",
        en: "hour",
    },
    CliGranularityDay => {
        zh_hans: "天",
        zh_hant: "天",
        en: "day",
    },
    CliUsageTrendRow => {
        zh_hans: "{label}  输入 {input} 缓存 {cache} 新建 {write} 输出 {output} 合计 {total}",
        zh_hant: "{label}  輸入 {input} 快取 {cache} 新建 {write} 輸出 {output} 合計 {total}",
        en: "{label}  in {input} cache {cache} write {write} out {output} total {total}",
    },
    CliByAgent => {
        zh_hans: "== 按 agent ==",
        zh_hant: "== 依 agent ==",
        en: "== By agent ==",
    },
    CliByProvider => {
        zh_hans: "== 按 provider ==",
        zh_hant: "== 依 provider ==",
        en: "== By provider ==",
    },
    CliByModel => {
        zh_hans: "== 按模型 ==",
        zh_hant: "== 依模型 ==",
        en: "== By model ==",
    },
    CliSourcesHeader => {
        zh_hans: "== 数据源 ==",
        zh_hant: "== 資料來源 ==",
        en: "== Data sources ==",
    },
    CliSourceNoUsage => {
        zh_hans: "{label} 无用量：{reason}",
        zh_hant: "{label} 無用量：{reason}",
        en: "{label} no usage: {reason}",
    },
    CliSourceNotDetected => {
        zh_hans: "{label} 未检测到",
        zh_hant: "{label} 未偵測到",
        en: "{label} not detected",
    },
    CliSourceStats => {
        zh_hans: "{label} {files} 文件 · {records} 条 · 坏行 {bad}{extra}",
        zh_hant: "{label} {files} 檔案 · {records} 筆 · 壞行 {bad}{extra}",
        en: "{label} {files} files · {records} records · bad lines {bad}{extra}",
    },
    CliScanStats => {
        zh_hans: "本次解析 {files} 个文件（缓存命中 {cached}）· {ms} ms · 剪除 {pruned} 条陈旧缓存",
        zh_hant: "本次解析 {files} 個檔案（快取命中 {cached}）· {ms} ms · 剪除 {pruned} 筆陳舊快取",
        en: "Parsed {files} files (cache hits {cached}) · {ms} ms · pruned {pruned} stale cache rows",
    },
    CliUsageNote => {
        zh_hans: "口径：{note}",
        zh_hant: "口徑：{note}",
        en: "Basis: {note}",
    },
    CliCostPartial => {
        zh_hans: "{value}（部分未定价）",
        zh_hant: "{value}（部分未定價）",
        en: "{value} (partly unpriced)",
    },
    CliPricingHeader => {
        zh_hans: "== 模型价格（每 100 万 token）==",
        zh_hant: "== 模型價格（每 100 萬 token）==",
        en: "== Model pricing (per million tokens) ==",
    },
    CliPricingEmpty => {
        zh_hans: "还没有登记模型价格。用 `agenthub pricing set <model> --input 3 --output 15` 加一条。",
        zh_hant: "還沒有登記模型價格。用 `agenthub pricing set <model> --input 3 --output 15` 加一筆。",
        en: "No model prices registered yet. Add one with `agenthub pricing set <model> --input 3 --output 15`.",
    },
    CliPriceRow => {
        zh_hans: "{model} 输入 {input} 缓存 {cache} 新建 {write} 输出 {output} 合计 {total} 调用 {calls} {source}",
        zh_hant: "{model} 輸入 {input} 快取 {cache} 新建 {write} 輸出 {output} 合計 {total} 呼叫 {calls} {source}",
        en: "{model} in {input} cache {cache} write {write} out {output} total {total} calls {calls} {source}",
    },
    CliPriceTableRow => {
        zh_hans: "{model} {provider} 输入 {input} 输出 {output} 缓存读 {cache_read} 缓存写 {cache_write} {source}",
        zh_hant: "{model} {provider} 輸入 {input} 輸出 {output} 快取讀 {cache_read} 快取寫 {cache_write} {source}",
        en: "{model} {provider} in {input} out {output} cache read {cache_read} cache write {cache_write} {source}",
    },
    CliNoDuplicates => {
        zh_hans: "没有重复技能",
        zh_hant: "沒有重複技能",
        en: "No duplicate skills",
    },
    CliDuplicateLine => {
        zh_hans: "{name} ({count} 份, 可省 {size}): {members}",
        zh_hant: "{name} ({count} 份, 可省 {size}): {members}",
        en: "{name} ({count} copies, reclaim {size}): {members}",
    },
    CliPlanWritten => {
        zh_hans: "计划已写入 {path}",
        zh_hant: "計畫已寫入 {path}",
        en: "Plan written to {path}",
    },
    CliApplied => {
        zh_hans: "已执行 {count} 个操作，批次 {batch}，备份 {backup}",
        zh_hant: "已執行 {count} 個操作，批次 {batch}，備份 {backup}",
        en: "Applied {count} operations, batch {batch}, backup {backup}",
    },
    CliBatchRow => {
        zh_hans: "{id}  {kind} {when} {count} 个操作 {undone}",
        zh_hant: "{id}  {kind} {when} {count} 個操作 {undone}",
        en: "{id}  {kind} {when} {count} operations {undone}",
    },
    CliBatchUndone => {
        zh_hans: "(已回滚)",
        zh_hant: "(已回滾)",
        en: "(rolled back)",
    },
    CliDoctorOk => {
        zh_hans: "一切正常",
        zh_hant: "一切正常",
        en: "All good",
    },
    CliManagedAdded => {
        zh_hans: "已纳入受管：{name}（只写入 config.json，未创建任何目录）",
        zh_hant: "已納入受管：{name}（只寫入 config.json，未建立任何目錄）",
        en: "Now managed: {name} (config.json only, no directories created)",
    },
    CliManagedAddedHint => {
        zh_hans: "下次 scan / mcp list 起生效；真正的文件改动仍要生成计划并确认执行。",
        zh_hant: "下次 scan / mcp list 起生效；真正的檔案改動仍要產生計畫並確認執行。",
        en: "Takes effect from the next scan / mcp list; real file changes still need a plan and confirmation.",
    },
    CliManagedRemoved => {
        zh_hans: "已移出受管：{name}（只删除配置登记，目录与已建链接保持原样）",
        zh_hant: "已移出受管：{name}（只刪除設定登記，目錄與已建連結保持原樣）",
        en: "No longer managed: {name} (only the config entry was removed; directories and links are untouched)",
    },
    ErrMcpServersMissing => {
        zh_hans: "文件里没有 mcpServers 对象",
        zh_hant: "檔案裡沒有 mcpServers 物件",
        en: "The file has no mcpServers object",
    },
    CliMcpWritten => {
        zh_hans: "已写入 {count} 个文件，批次 {batch}",
        zh_hant: "已寫入 {count} 個檔案，批次 {batch}",
        en: "Wrote {count} files, batch {batch}",
    },
    CliExists => {
        zh_hans: "存在",
        zh_hant: "存在",
        en: "present",
    },
    CliMissing => {
        zh_hans: "缺失",
        zh_hant: "缺少",
        en: "missing",
    },
    CliChainHeader => {
        zh_hans: "加载链路:",
        zh_hant: "載入鏈路:",
        en: "Load chain:",
    },
    CliInstructionInjected => {
        zh_hans: "已注入，批次 {batch}",
        zh_hant: "已注入，批次 {batch}",
        en: "Injected, batch {batch}",
    },
    CliBridgeDone => {
        zh_hans: "桥接完成，批次 {batch}",
        zh_hant: "橋接完成，批次 {batch}",
        en: "Bridge done, batch {batch}",
    },
    CliProjectInit => {
        zh_hans: "项目已初始化，批次 {batch}",
        zh_hant: "專案已初始化，批次 {batch}",
        en: "Project initialized, batch {batch}",
    },
    CliProjectSkillsEnabled => {
        zh_hans: "已启用 {count} 个技能，批次 {batch}",
        zh_hant: "已啟用 {count} 個技能，批次 {batch}",
        en: "Enabled {count} skills, batch {batch}",
    },
    ErrUnknownProtocol => {
        zh_hans: "未知的协议：{value}（可选 anthropic / chat_completions / responses）",
        zh_hant: "未知的協議：{value}（可選 anthropic / chat_completions / responses）",
        en: "Unknown protocol: {value} (use anthropic / chat_completions / responses)",
    },
    CliProviderSaved => {
        zh_hans: "已保存（只写 config.json，尚未改动任何 agent 配置）",
        zh_hant: "已儲存（只寫 config.json，尚未改動任何 agent 設定）",
        en: "Saved (config.json only; no agent config has been touched)",
    },
    CliProbeEndpoint => {
        zh_hans: "请求地址：{endpoint}",
        zh_hant: "請求位址：{endpoint}",
        en: "Endpoint: {endpoint}",
    },
    CliProbeFailed => {
        zh_hans: "拉取模型列表失败",
        zh_hant: "取得模型清單失敗",
        en: "Failed to fetch the model list",
    },
    CliProbeOk => {
        zh_hans: "{count} 个模型（{endpoint}）",
        zh_hant: "{count} 個模型（{endpoint}）",
        en: "{count} models ({endpoint})",
    },
    CliProviderDeleted => {
        zh_hans: "已删除 provider `{id}` 及其绑定",
        zh_hant: "已刪除 provider `{id}` 及其綁定",
        en: "Deleted provider `{id}` and its bindings",
    },
    CliFeatureOn => {
        zh_hans: "启用",
        zh_hant: "啟用",
        en: "on",
    },
    CliFeatureOff => {
        zh_hans: "禁用",
        zh_hant: "停用",
        en: "off",
    },
    CliFeatureEnabled => {
        zh_hans: "已启用 {key}",
        zh_hant: "已啟用 {key}",
        en: "Enabled {key}",
    },
    CliFeatureDisabled => {
        zh_hans: "已禁用 {key}：入口隐藏，配置与历史数据保留",
        zh_hant: "已停用 {key}：入口隱藏，設定與歷史資料保留",
        en: "Disabled {key}: the entry is hidden, config and history are kept",
    },
    CliGatewayListen => {
        zh_hans: "监听 127.0.0.1:{port}  {auth}",
        zh_hant: "監聽 127.0.0.1:{port}  {auth}",
        en: "Listening on 127.0.0.1:{port}  {auth}",
    },
    CliGatewayNeedToken => {
        zh_hans: "（需本地令牌）",
        zh_hant: "（需本機權杖）",
        en: "(local token required)",
    },
    CliGatewayNoToken => {
        zh_hans: "（无需令牌）",
        zh_hant: "（無需權杖）",
        en: "(no token needed)",
    },
    CliNoRoutes => {
        zh_hans: "还没有网关路由",
        zh_hant: "還沒有閘道路由",
        en: "No gateway routes yet",
    },
    CliRouteLine => {
        zh_hans: "{id} [{protocol}] {name}  {members} 个成员 / {rules} 条规则  {disabled}",
        zh_hant: "{id} [{protocol}] {name}  {members} 個成員 / {rules} 條規則  {disabled}",
        en: "{id} [{protocol}] {name}  {members} members / {rules} rules  {disabled}",
    },
    CliRouteDisabled => {
        zh_hans: "（已停用）",
        zh_hant: "（已停用）",
        en: "(disabled)",
    },
    CliRouteUrl => {
        zh_hans: "  接入地址 {url}",
        zh_hant: "  接入位址 {url}",
        en: "  Endpoint {url}",
    },
    CliRouteMember => {
        zh_hans: "  · {id}（权重 {weight} / 优先级 {priority}）",
        zh_hant: "  · {id}（權重 {weight} / 優先順序 {priority}）",
        en: "  · {id} (weight {weight} / priority {priority})",
    },
    CliGatewayHint => {
        zh_hans: "提示：{note}",
        zh_hant: "提示：{note}",
        en: "Note: {note}",
    },
    CliPricingPreview => {
        zh_hans: "从 {source} 解析到 {count} 条模型价格（尚未写入）。加 --yes 应用。",
        zh_hant: "從 {source} 解析到 {count} 筆模型價格（尚未寫入）。加 --yes 套用。",
        en: "Parsed {count} model prices from {source} (not written yet). Add --yes to apply.",
    },
    CliPricingPreviewRow => {
        zh_hans: "  {model} {provider} 输入 {input} 输出 {output}",
        zh_hant: "  {model} {provider} 輸入 {input} 輸出 {output}",
        en: "  {model} {provider} in {input} out {output}",
    },
    CliPricingMore => {
        zh_hans: "… 共 {count} 条",
        zh_hant: "… 共 {count} 筆",
        en: "… {count} total",
    },
    CliPricingImported => {
        zh_hans: "已导入 {count} 条模型价格",
        zh_hant: "已匯入 {count} 筆模型價格",
        en: "Imported {count} model prices",
    },
    CliPricingClearHint => {
        zh_hans: "这会清空全部模型价格登记。确认请加 --yes。",
        zh_hant: "這會清空全部模型價格登記。確認請加 --yes。",
        en: "This clears every registered model price. Add --yes to confirm.",
    },
    CliConfirmApply => {
        zh_hans: "确认执行 {count} 个操作？[y/N] ",
        zh_hant: "確認執行 {count} 個操作？[y/N] ",
        en: "Apply {count} operations? [y/N] ",
    },
    CliCancelled => {
        zh_hans: "已取消",
        zh_hant: "已取消",
        en: "Cancelled",
    },

    // ------------------------------------------------ 本地网关进程（agenthub-gateway）
    GatewayListenFailed => {
        zh_hans: "无法监听 {host}:{port} —— {error}",
        zh_hant: "無法監聽 {host}:{port} —— {error}",
        en: "Cannot listen on {host}:{port} — {error}",
    },
    GatewayLocalAddrFailed => {
        zh_hans: "无法读取监听地址：{error}",
        zh_hant: "無法讀取監聽位址：{error}",
        en: "Cannot read the listening address: {error}",
    },
    GatewayExited => {
        zh_hans: "[agenthub-gateway] 服务异常退出：{error}",
        zh_hant: "[agenthub-gateway] 服務異常結束：{error}",
        en: "[agenthub-gateway] the service exited unexpectedly: {error}",
    },
    GatewayMissingRouteId => {
        zh_hans: "缺少路由 id：/gw/<route>/...",
        zh_hant: "缺少路由 id：/gw/<route>/...",
        en: "Missing route id: /gw/<route>/...",
    },
    GatewayUnknownToken => {
        zh_hans: "未知令牌 `{token}`：/v1 用令牌指定供应商池，可用值为 {tokens}",
        zh_hant: "未知權杖 `{token}`：/v1 用權杖指定供應商池，可用值為 {tokens}",
        en: "Unknown token `{token}`: /v1 uses the token to pick a supplier pool; available: {tokens}",
    },
    GatewayMissingToken => {
        zh_hans: "缺少令牌：请用 Authorization: Bearer <agent 名>（如 claude / codex）",
        zh_hant: "缺少權杖：請用 Authorization: Bearer <agent 名稱>（如 claude / codex）",
        en: "Missing token: use Authorization: Bearer <agent name> (for example claude / codex)",
    },
    GatewayTokenRequired => {
        zh_hans: "网关需要本地令牌：Authorization: Bearer <token> 或 x-api-key: <token>",
        zh_hant: "閘道需要本機權杖：Authorization: Bearer <token> 或 x-api-key: <token>",
        en: "The gateway needs a local token: Authorization: Bearer <token> or x-api-key: <token>",
    },
    GatewayRouteUnknown => {
        zh_hans: "网关里没有路由 `{route}`",
        zh_hant: "閘道裡沒有路由 `{route}`",
        en: "The gateway has no route `{route}`",
    },
    GatewayRouteDisabled => {
        zh_hans: "该路由已停用",
        zh_hant: "該路由已停用",
        en: "This route is disabled",
    },
    GatewayNoUpstream => {
        zh_hans: "没有可用的上游：检查路由成员是否启用、模型白名单是否匹配、密钥能否解析",
        zh_hant: "沒有可用的上游：檢查路由成員是否啟用、模型白名單是否相符、金鑰能否解析",
        en: "No usable upstream: check that route members are enabled, model allowlists match, and keys resolve",
    },
    GatewayFirstByteTimeout => {
        zh_hans: "{name} 首字节超时（{ms}ms）",
        zh_hant: "{name} 首位元組逾時（{ms}ms）",
        en: "{name} timed out waiting for the first byte ({ms}ms)",
    },
    GatewayConnectFailed => {
        zh_hans: "{name} 连接失败：{error}",
        zh_hant: "{name} 連線失敗：{error}",
        en: "{name} connection failed: {error}",
    },
    GatewayUpstreamStatus => {
        zh_hans: "{name} 返回 {status}，切下一个上游",
        zh_hant: "{name} 回傳 {status}，切換下一個上游",
        en: "{name} returned {status}; switching to the next upstream",
    },
    GatewayAllUpstreamsFailed => {
        zh_hans: "所有上游都失败了",
        zh_hant: "所有上游都失敗了",
        en: "All upstreams failed",
    },
    GatewayReadUpstreamFailed => {
        zh_hans: "读取上游响应失败：{error}",
        zh_hant: "讀取上游回應失敗：{error}",
        en: "Failed to read the upstream response: {error}",
    },
    GatewayReadUpstreamFailedShort => {
        zh_hans: "读取上游响应失败",
        zh_hant: "讀取上游回應失敗",
        en: "Failed to read the upstream response",
    },
    GatewayResponseBuildFailed => {
        zh_hans: "响应构造失败",
        zh_hant: "回應建構失敗",
        en: "Failed to build the response",
    },
    GatewayRouteIdInvalid => {
        zh_hans: "路由 `{route}` 的 id 非法，已跳过",
        zh_hant: "路由 `{route}` 的 id 非法，已跳過",
        en: "Route `{route}` has an invalid id and was skipped",
    },
    GatewayRouteDuplicate => {
        zh_hans: "路由 `{route}` 重复，保留先出现的一条",
        zh_hant: "路由 `{route}` 重複，保留先出現的一條",
        en: "Route `{route}` is duplicated; the first one wins",
    },
    GatewayRouteMissingProvider => {
        zh_hans: "路由 `{route}` 引用了不存在的 provider `{provider}`",
        zh_hant: "路由 `{route}` 參照了不存在的 provider `{provider}`",
        en: "Route `{route}` references an unknown provider `{provider}`",
    },
    GatewayKeyResolveFailed => {
        zh_hans: "密钥解析失败（{masked}）：{error}",
        zh_hant: "金鑰解析失敗（{masked}）：{error}",
        en: "Failed to resolve the key ({masked}): {error}",
    },
    GatewayRouteNoMembers => {
        zh_hans: "路由 `{route}` 没有可用成员，已跳过",
        zh_hant: "路由 `{route}` 沒有可用成員，已跳過",
        en: "Route `{route}` has no usable members and was skipped",
    },
    GatewayStreamBroken => {
        zh_hans: "上游流中断：{error}",
        zh_hant: "上游串流中斷：{error}",
        en: "The upstream stream broke: {error}",
    },

    // ------------------------------------------------------------ 技能市场
    ErrMarketParse => {
        zh_hans: "{what}：市场返回的数据格式无法解析（{error}）",
        zh_hant: "{what}：市場回傳的資料格式無法解析（{error}）",
        en: "{what}: the market response could not be parsed ({error})",
    },
    MarketUnknownError => {
        zh_hans: "未知错误",
        zh_hant: "未知錯誤",
        en: "Unknown error",
    },
    ErrMarketApi => {
        zh_hans: "{what}：{message}（code {code}）",
        zh_hant: "{what}：{message}（code {code}）",
        en: "{what}: {message} (code {code})",
    },
    MarketOpVersions => {
        zh_hans: "读取技能版本列表",
        zh_hant: "讀取技能版本清單",
        en: "Read the skill version list",
    },
    MarketOpSearch => {
        zh_hans: "搜索技能",
        zh_hant: "搜尋技能",
        en: "Search skills",
    },
    MarketOpList => {
        zh_hans: "读取技能列表",
        zh_hant: "讀取技能清單",
        en: "Read the skill list",
    },
    MarketOpDetail => {
        zh_hans: "读取技能详情",
        zh_hant: "讀取技能詳情",
        en: "Read skill details",
    },
    MarketOpFiles => {
        zh_hans: "读取技能文件清单",
        zh_hant: "讀取技能檔案清單",
        en: "Read the skill file list",
    },
    MarketOpDownload => {
        zh_hans: "下载技能包",
        zh_hant: "下載技能包",
        en: "Download the skill package",
    },
    MarketOpReadFile => {
        zh_hans: "读取文件 {path}",
        zh_hant: "讀取檔案 {path}",
        en: "Read file {path}",
    },
    ErrSearchQueryEmpty => {
        zh_hans: "请输入要搜索的关键词",
        zh_hant: "請輸入要搜尋的關鍵字",
        en: "Enter a search keyword",
    },
    ErrSkillSlugEmpty => {
        zh_hans: "技能标识为空",
        zh_hant: "技能標識為空",
        en: "The skill identifier is empty",
    },
    ErrMarketSkillMissing => {
        zh_hans: "技能市场里没有 {slug} 这个技能",
        zh_hant: "技能市場裡沒有 {slug} 這個技能",
        en: "The skill market has no skill named {slug}",
    },
    ErrMarketZipUnavailableStatus => {
        zh_hans: "该技能包没有可用的 ZIP 下载（HTTP {status}），改用逐文件读取",
        zh_hant: "該技能包沒有可用的 ZIP 下載（HTTP {status}），改用逐檔讀取",
        en: "No ZIP download is available for this skill package (HTTP {status}); falling back to per-file reads",
    },
    ErrMarketZipUnavailable => {
        zh_hans: "该技能包没有提供 ZIP 下载，改用逐文件读取",
        zh_hant: "該技能包沒有提供 ZIP 下載，改用逐檔讀取",
        en: "This skill package offers no ZIP download; falling back to per-file reads",
    },
    ErrMarketReturnedPage => {
        zh_hans: "读取文件 {path}：市场返回了网页而不是文件内容",
        zh_hant: "讀取檔案 {path}：市場回傳了網頁而不是檔案內容",
        en: "Read file {path}: the market returned a web page instead of file content",
    },
    ErrMarketStatusAuth => {
        zh_hans: "{what}：该内容不可访问（可能需要登录或已下架）",
        zh_hant: "{what}：該內容無法存取（可能需要登入或已下架）",
        en: "{what}: the content is not accessible (sign-in required or it was delisted)",
    },
    ErrMarketStatusMissing => {
        zh_hans: "{what}：技能不存在或已下架",
        zh_hant: "{what}：技能不存在或已下架",
        en: "{what}: the skill does not exist or was delisted",
    },
    ErrMarketStatusConflict => {
        zh_hans: "{what}：该技能名在市场中不唯一或暂不可下载（HTTP 409），换一个技能试试",
        zh_hant: "{what}：該技能名在市場中不唯一或暫不可下載（HTTP 409），換一個技能試試",
        en: "{what}: the skill name is ambiguous or temporarily unavailable (HTTP 409); try another skill",
    },
    ErrMarketStatusRateLimit => {
        zh_hans: "{what}：请求过于频繁，请稍后再试",
        zh_hant: "{what}：請求過於頻繁，請稍後再試",
        en: "{what}: too many requests; try again later",
    },
    ErrMarketStatusServer => {
        zh_hans: "{what}：技能市场暂时不可用（HTTP {code}）",
        zh_hant: "{what}：技能市場暫時無法使用（HTTP {code}）",
        en: "{what}: the skill market is temporarily unavailable (HTTP {code})",
    },
    ErrMarketStatusFailed => {
        zh_hans: "{what}：请求失败（HTTP {code}）",
        zh_hant: "{what}：請求失敗（HTTP {code}）",
        en: "{what}: request failed (HTTP {code})",
    },
    ErrMarketUtf8 => {
        zh_hans: "市场返回的内容不是合法 UTF-8",
        zh_hant: "市場回傳的內容不是合法 UTF-8",
        en: "The market response is not valid UTF-8",
    },
    ErrMarketTimeout => {
        zh_hans: "连接技能市场超时，请检查网络或代理设置后重试",
        zh_hant: "連線技能市場逾時，請檢查網路或代理設定後重試",
        en: "Timed out connecting to the skill market; check your network or proxy and retry",
    },
    ErrMarketHostNotFound => {
        zh_hans: "无法解析技能市场域名，请检查网络或代理设置",
        zh_hant: "無法解析技能市場網域，請檢查網路或代理設定",
        en: "Cannot resolve the skill market host; check your network or proxy",
    },
    ErrMarketConnectionFailed => {
        zh_hans: "无法连接技能市场（可能是网络或代理问题）",
        zh_hant: "無法連線技能市場（可能是網路或代理問題）",
        en: "Cannot connect to the skill market (possibly a network or proxy issue)",
    },
    ErrMarketTls => {
        zh_hans: "与技能市场建立 TLS 连接失败",
        zh_hant: "與技能市場建立 TLS 連線失敗",
        en: "Failed to establish a TLS connection with the skill market",
    },
    ErrMarketBodyTooLarge => {
        zh_hans: "市场返回的内容超过 {limit} 字节上限，已中止",
        zh_hant: "市場回傳的內容超過 {limit} 位元組上限，已中止",
        en: "The market response exceeds the {limit}-byte limit; aborted",
    },
    ErrNetworkRequest => {
        zh_hans: "网络请求失败：{error}",
        zh_hant: "網路請求失敗：{error}",
        en: "Network request failed: {error}",
    },
    ErrMarketSourceHttps => {
        zh_hans: "市场源地址必须是 https:// 开头：{value}",
        zh_hant: "市場來源位址必須是 https:// 開頭：{value}",
        en: "The market source URL must start with https://: {value}",
    },
    ErrMarketSourceTooLong => {
        zh_hans: "市场源地址过长",
        zh_hant: "市場來源位址過長",
        en: "The market source URL is too long",
    },
    MarketAgentNotDetected => {
        zh_hans: "本机未检测到该 agent",
        zh_hant: "本機未偵測到該 agent",
        en: "This agent was not detected on this machine",
    },
    MarketAgentNoSkillsDir => {
        zh_hans: "该 agent 没有可写的技能目录",
        zh_hant: "該 agent 沒有可寫的技能目錄",
        en: "This agent has no writable skills directory",
    },
    MarketAgentUnavailable => {
        zh_hans: "不可用",
        zh_hant: "不可用",
        en: "Unavailable",
    },
    ErrMarketSelectAgent => {
        zh_hans: "请至少选择一个目标 agent",
        zh_hant: "請至少選擇一個目標 agent",
        en: "Select at least one target agent",
    },
    PlanTitleMarketInstall => {
        zh_hans: "从市场安装 {name}",
        zh_hant: "從市場安裝 {name}",
        en: "Install {name} from the market",
    },
    PlanSkipAgent => {
        zh_hans: "跳过 {agent}：{reason}",
        zh_hant: "跳過 {agent}：{reason}",
        en: "Skipped {agent}: {reason}",
    },
    ErrMarketNoTargets => {
        zh_hans: "所选 agent 在本机都不可用，没有可建立的链接",
        zh_hant: "所選 agent 在本機都不可用，沒有可建立的連結",
        en: "None of the selected agents are usable here, so no links can be created",
    },
    PlanStoreReplaced => {
        zh_hans: "中央仓库已有同名技能 {name}，旧版本将进回收站（可用批次回滚恢复）",
        zh_hant: "中央倉庫已有同名技能 {name}，舊版本將進資源回收筒（可用批次回滾恢復）",
        en: "The hub store already has a skill named {name}; the old version goes to the trash (recoverable by batch rollback)",
    },
    PlanLinkSkipped => {
        zh_hans: "{path} 目录已存在同名技能，已跳过建链（如需替换请先在技能库里删除）",
        zh_hant: "{path} 目錄已存在同名技能，已跳過建鏈（如需替換請先在技能庫裡刪除）",
        en: "{path} already has a skill with this name; link creation skipped (delete it in the skills view to replace)",
    },
    PlanPackageSummary => {
        zh_hans: "技能包：{files} 个文件 · {size} · {mode}",
        zh_hant: "技能包：{files} 個檔案 · {size} · {mode}",
        en: "Skill package: {files} files · {size} · {mode}",
    },
    PlanModePerFile => {
        zh_hans: "逐文件下载",
        zh_hant: "逐檔下載",
        en: "per-file download",
    },
    PlanVersion => {
        zh_hans: "版本 {version}",
        zh_hant: "版本 {version}",
        en: "Version {version}",
    },
    PlanSkippedFiles => {
        zh_hans: "跳过 {count} 个文件：{list}",
        zh_hant: "跳過 {count} 個檔案：{list}",
        en: "Skipped {count} files: {list}",
    },
    RiskMarketSuspicious => {
        zh_hans: "技能市场把 {name} 标为可疑：{summary}。安装后请先读一遍 SKILL.md 再让 agent 使用",
        zh_hant: "技能市場把 {name} 標為可疑：{summary}。安裝後請先讀一遍 SKILL.md 再讓 agent 使用",
        en: "The market flags {name} as suspicious: {summary}. Read SKILL.md before letting an agent use it",
    },
    MarketNoDescription => {
        zh_hans: "未提供说明",
        zh_hant: "未提供說明",
        en: "No description provided",
    },
    ErrZipInvalid => {
        zh_hans: "技能包不是合法的 ZIP：{error}",
        zh_hant: "技能包不是合法的 ZIP：{error}",
        en: "The skill package is not a valid ZIP: {error}",
    },
    ErrZipTooManyEntries => {
        zh_hans: "技能包里有 {count} 个条目，超过 {limit} 上限，已中止",
        zh_hant: "技能包裡有 {count} 個項目，超過 {limit} 上限，已中止",
        en: "The skill package has {count} entries, over the {limit} limit; aborted",
    },
    ErrZipReadEntry => {
        zh_hans: "读取技能包条目失败：{error}",
        zh_hant: "讀取技能包項目失敗：{error}",
        en: "Failed to read a skill package entry: {error}",
    },
    ZipSkipUnreadableName => {
        zh_hans: "条目名无法读取：{error}",
        zh_hant: "項目名無法讀取：{error}",
        en: "Entry name unreadable: {error}",
    },
    ZipSkipEscaped => {
        zh_hans: "{name}（路径越界）",
        zh_hant: "{name}（路徑越界）",
        en: "{name} (path escapes the target)",
    },
    ZipSkipSymlink => {
        zh_hans: "{path}（符号链接）",
        zh_hant: "{path}（符號連結）",
        en: "{path} (symlink)",
    },
    ErrZipTooLargeExtract => {
        zh_hans: "技能包解压后超过 {limit} 上限，已中止（疑似压缩炸弹）",
        zh_hant: "技能包解壓後超過 {limit} 上限，已中止（疑似壓縮炸彈）",
        en: "The extracted skill package exceeds {limit}; aborted (possible zip bomb)",
    },
    ErrZipExtractFailed => {
        zh_hans: "解压 {path} 失败：{error}",
        zh_hant: "解壓 {path} 失敗：{error}",
        en: "Failed to extract {path}: {error}",
    },
    ErrZipTooLarge => {
        zh_hans: "技能包解压后超过体积上限，已中止",
        zh_hant: "技能包解壓後超過大小上限，已中止",
        en: "The extracted skill package exceeds the size limit; aborted",
    },
    ErrZipNoSkillMd => {
        zh_hans: "技能包根目录缺少 SKILL.md，这不是一个可安装的技能",
        zh_hant: "技能包根目錄缺少 SKILL.md，這不是一個可安裝的技能",
        en: "The package root has no SKILL.md; this is not an installable skill",
    },
    ErrZipPathEscape => {
        zh_hans: "技能包内路径越界：{path}",
        zh_hant: "技能包內路徑越界：{path}",
        en: "Path escapes the package root: {path}",
    },
    // 市场模块的开发者日志（只进终端，但同样走词典，避免英文界面下中英混排）
    LogMarketSkillMdFailed => {
        zh_hans: "[market] 读取 {slug} 的 SKILL.md 失败，回退摘要：{error}",
        zh_hant: "[market] 讀取 {slug} 的 SKILL.md 失敗，回退摘要：{error}",
        en: "[market] failed to read SKILL.md for {slug}, falling back to the summary: {error}",
    },
    LogMarketFilesFailed => {
        zh_hans: "[market] 读取 {slug} 的文件清单失败：{error}",
        zh_hant: "[market] 讀取 {slug} 的檔案清單失敗：{error}",
        en: "[market] failed to read the file list for {slug}: {error}",
    },
    LogMarketRequestFailed => {
        zh_hans: "[market] 请求失败 {url}: {error}",
        zh_hant: "[market] 請求失敗 {url}: {error}",
        en: "[market] request failed {url}: {error}",
    },
    ErrMarketFileList => {
        zh_hans: "无法获取 {name} 的文件清单：{error}。该技能可能只支持通过官方 CLI 安装",
        zh_hant: "無法取得 {name} 的檔案清單：{error}。該技能可能只支援透過官方 CLI 安裝",
        en: "Cannot fetch the {name} file list: {error}. This skill may only be installable via its official CLI",
    },
    ErrMarketNoFiles => {
        zh_hans: "{name} 没有可下载的文件（该源既不给 ZIP 也不给文件清单）",
        zh_hant: "{name} 沒有可下載的檔案（該來源既不給 ZIP 也不給檔案清單）",
        en: "{name} has no downloadable files (the source offers neither a ZIP nor a file list)",
    },

    // ------------------------------------------------------------ 网关
    ErrConvertResponsesAnthropic => {
        zh_hans: "Responses 与 Anthropic Messages 之间的转换本期不支持：请把路由或供应商改成一致的协议",
        zh_hant: "Responses 與 Anthropic Messages 之間的轉換本期不支援：請把路由或供應商改成一致的協議",
        en: "Converting between Responses and Anthropic Messages is not supported yet; align the route or supplier protocol",
    },
    ErrConvertResponsesChat => {
        zh_hans: "Responses 与 Chat Completions 之间的转换本期不支持：请把路由或供应商改成一致的协议",
        zh_hant: "Responses 與 Chat Completions 之間的轉換本期不支援：請把路由或供應商改成一致的協議",
        en: "Converting between Responses and Chat Completions is not supported yet; align the route or supplier protocol",
    },
    ErrProtocolComboUnsupported => {
        zh_hans: "该协议组合本期不支持",
        zh_hant: "該協議組合本期不支援",
        en: "This protocol combination is not supported yet",
    },
    ErrRouteIdChars => {
        zh_hans: "路由 id 只能用小写字母、数字、`-`、`_`，长度 1..=32",
        zh_hant: "路由 id 只能用小寫字母、數字、`-`、`_`，長度 1..=32",
        en: "A route id may only contain lowercase letters, digits, `-` and `_`, length 1..=32",
    },
    ErrRouteNameEmpty => {
        zh_hans: "路由名称不能为空",
        zh_hant: "路由名稱不能為空",
        en: "The route name cannot be empty",
    },
    ErrRouteNoMembers => {
        zh_hans: "路由至少要有一个上游成员",
        zh_hant: "路由至少要有一個上游成員",
        en: "A route needs at least one upstream member",
    },
    ErrRouteMemberMissingProvider => {
        zh_hans: "路由成员引用了不存在的 provider：{ids}",
        zh_hant: "路由成員參照了不存在的 provider：{ids}",
        en: "Route members reference unknown providers: {ids}",
    },
    ErrRouteDuplicateProvider => {
        zh_hans: "同一个 provider 不能在路由里出现两次：{ids}",
        zh_hant: "同一個 provider 不能在路由裡出現兩次：{ids}",
        en: "The same provider cannot appear twice in a route: {ids}",
    },
    ErrRouteSingleMember => {
        zh_hans: "单上游模式只能有一个成员",
        zh_hant: "單上游模式只能有一個成員",
        en: "Single-upstream mode allows exactly one member",
    },
    ErrRuleMatchEmpty => {
        zh_hans: "模型规则的匹配串不能为空",
        zh_hant: "模型規則的比對字串不能為空",
        en: "A model rule pattern cannot be empty",
    },
    ErrRuleMissingProvider => {
        zh_hans: "模型规则引用了不在成员里的 provider：{id}",
        zh_hant: "模型規則參照了不在成員裡的 provider：{id}",
        en: "A model rule references a provider that is not a member: {id}",
    },
    GatewayNoteLine => {
        zh_hans: "{agent}：{note}",
        zh_hant: "{agent}：{note}",
        en: "{agent}: {note}",
    },
    GatewayRouteCheckFailed => {
        zh_hans: "路由 `{route}`：{error}",
        zh_hant: "路由 `{route}`：{error}",
        en: "Route `{route}`: {error}",
    },
    GatewayBindingMissingRoute => {
        zh_hans: "{agent} 绑定指向不存在的路由 `{route}`",
        zh_hant: "{agent} 綁定指向不存在的路由 `{route}`",
        en: "{agent} is bound to a route that does not exist: `{route}`",
    },
    GatewayBindingNoRoute => {
        zh_hans: "{agent} 标记了「经网关」但没有选路由",
        zh_hant: "{agent} 標記了「經閘道」但沒有選路由",
        en: "{agent} is marked as “via gateway” but has no route selected",
    },
    ErrGatewayPortReserved => {
        zh_hans: "端口必须大于 1023：1024 以下是系统保留区",
        zh_hant: "連接埠必須大於 1023：1024 以下是系統保留區",
        en: "The port must be above 1023; lower ports are reserved by the system",
    },
    ErrGatewayRouteMissing => {
        zh_hans: "网关路由 `{route}` 不存在",
        zh_hant: "閘道路由 `{route}` 不存在",
        en: "Gateway route `{route}` does not exist",
    },
    PlanTitleGatewayToDirect => {
        zh_hans: "把经网关的 agent 退回直连",
        zh_hant: "把經閘道的 agent 退回直連",
        en: "Switch gateway-backed agents back to direct",
    },
    PlanGatewayNothingToRevert => {
        zh_hans: "没有指向网关的绑定，无需回退",
        zh_hant: "沒有指向閘道的綁定，無需回退",
        en: "No bindings point at the gateway; nothing to revert",
    },
    ErrTranslateServiceDisabled => {
        zh_hans: "翻译服务尚未启用（Phase 5）：当前只支持读取本地缓存",
        zh_hant: "翻譯服務尚未啟用（Phase 5）：目前只支援讀取本機快取",
        en: "The translation service is not enabled yet (Phase 5); only the local cache can be read",
    },
}
