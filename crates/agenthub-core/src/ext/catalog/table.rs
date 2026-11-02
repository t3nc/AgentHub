//! 「常见 Agent 目录」静态表。
//!
//! 三条硬规则：
//! 1. 命令与路径只在核实过官方资料后才写进来；没核实的标 `needs_confirm`，并在
//!    `blocked` 里说明为什么还不能纳入受管，绝不编造包名。
//! 2. 这里只有字符串，没有任何 IO；检测逻辑在同目录的 `mod.rs`。
//! 3. 内置 5 个 agent 的目录不在这里复制一份——它们的真实路径由
//!    `BuiltinAdapter` 提供，表里只放 key / 名称 / 安装步骤。

use super::McpFormatSpec;
use crate::l10n::Msg;
use crate::model::agent::AgentId;

/// 步骤里那一行「可复制内容」。
///
/// 真的 shell 命令与 URL 用 `Raw`（翻译它会让用户复制出跑不通的东西），
/// 纯说明文字用 `Msg`（例如「首次启动时选哪个站点」）。
pub enum TableText {
    Raw(&'static str),
    Msg(Msg),
}

impl TableText {
    pub fn resolve(&self) -> String {
        match self {
            TableText::Raw(raw) => (*raw).to_string(),
            TableText::Msg(msg) => crate::l10n::str_of(*msg).to_string(),
        }
    }
}

/// 一条可复制的安装/验证步骤。`command` 对桌面产品是下载页地址。
///
/// `title` / `risk` 存的是词典 key 而不是成品文案：这张表是 `static`，
/// 取词必须发生在构造 `CatalogEntry` / `InstallPlan` 的那一刻（跟着当前界面语言）。
pub struct Step {
    pub title: Msg,
    pub command: TableText,
    pub risk: Msg,
    pub url: Option<&'static str>,
}

/// 「纳入受管」时写进 `config.custom_agents` 的模板。
pub struct Template {
    pub id: &'static str,
    pub display_name: &'static str,
    pub skills_dir: Option<&'static str>,
    pub project_skills_dir: Option<&'static str>,
    pub mcp_file: Option<&'static str>,
    /// `mcp_file` 之外还要尝试的路径：Store（MSIX）虚拟目录、旧版 Roaming 等。
    /// 登记与预览都选「第一个存在的」，都不存在时用 `mcp_file` 并标成尚未创建。
    pub mcp_file_alternates: &'static [&'static str],
    pub mcp_format: Option<McpFormatSpec>,
    pub project_mcp_file: Option<&'static str>,
    pub instructions: &'static [&'static str],
    /// 写进 UI 的说明：这次登记只接管了什么（存 key，序列化前取词）。
    pub note: Msg,
}

pub struct Spec {
    pub key: &'static str,
    /// 产品族：成对产品（claude / codex / tencent …）在 UI 里相邻展示。
    pub family: &'static str,
    pub name: TableText,
    pub vendor: TableText,
    pub kind: super::AgentKind,
    pub official: &'static str,
    pub needs_confirm: bool,
    /// 对应的内置适配器；有值时路径与 CLI 名以适配器为准。
    pub builtin: Option<AgentId>,
    pub cli: &'static [&'static str],
    pub win_apps: &'static [&'static str],
    pub mac_apps: &'static [&'static str],
    /// Windows MSIX（Microsoft Store）包家族名。
    ///
    /// Store 版应用装在 `%LOCALAPPDATA%\Packages\<PFN>`，不写 `%APPDATA%` 也不在
    /// `Programs` 下；只做目录存在性判断，不调 `Get-AppxPackage`（保持检测零执行）。
    pub win_msix: &'static [&'static str],
    pub win_config: &'static [&'static str],
    pub mac_config: &'static [&'static str],
    /// 配置由这个内置适配器接管（如 OpenAI 桌面端与 Codex CLI 共用 `~/.codex`）。
    /// 有值时状态是 `Managed::Shared`，不再显示「不可受管」。
    pub shared_agent: Option<AgentId>,
    pub win_steps: &'static [Step],
    pub mac_steps: &'static [Step],
    pub win_managed: Option<Template>,
    pub mac_managed: Option<Template>,
    /// 不能纳入受管时的原因（没有模板时展示；存 key，取词在构造条目时）。
    pub blocked: Option<Msg>,
}

const NPM_RISK: Msg = Msg::CatalogRiskNpm;
const VERIFY_RISK: Msg = Msg::CatalogRiskVerify;
const APP_RISK: Msg = Msg::CatalogRiskApp;

// ---- 内置 5 个 ----------------------------------------------------------

const CLAUDE_WIN: &[Step] = &[
    Step {
        title: Msg::CatalogStepInstallClaudeCode,
        command: TableText::Raw("npm install -g @anthropic-ai/claude-code"),
        risk: NPM_RISK,
        url: Some("https://docs.claude.com/en/docs/claude-code/setup"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("claude --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

const CLAUDE_MAC: &[Step] = &[
    Step {
        title: Msg::CatalogStepInstallClaudeCode,
        command: TableText::Raw("npm install -g @anthropic-ai/claude-code"),
        risk: NPM_RISK,
        url: Some("https://docs.claude.com/en/docs/claude-code/setup"),
    },
    Step {
        title: Msg::CatalogStepUseOfficialScript,
        command: TableText::Raw("curl -fsSL https://claude.ai/install.sh | bash"),
        risk: Msg::CatalogRiskBinaryScript,
        url: Some("https://claude.ai/install.sh"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("claude --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

const CODEX: &[Step] = &[
    Step {
        title: Msg::CatalogStepInstallCodex,
        command: TableText::Raw("npm install -g @openai/codex"),
        risk: NPM_RISK,
        url: Some("https://github.com/openai/codex"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("codex --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

const CODEBUDDY_WIN: &[Step] = &[
    Step {
        title: Msg::CatalogStepInstallCodebuddy,
        command: TableText::Raw("npm install -g @tencent-ai/codebuddy-code"),
        risk: NPM_RISK,
        url: Some("https://www.codebuddy.cn/docs/cli/quickstart"),
    },
    Step {
        title: Msg::CatalogStepCodebuddySite,
        command: TableText::Msg(Msg::CatalogCommandCodebuddySite),
        risk: Msg::CatalogRiskCodebuddySites,
        url: Some("https://www.codebuddy.ai/docs/zh/cli/quickstart"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("codebuddy --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

/// CodeBuddy IDE：两版是不同安装包，配置目录也不同。
///
/// 本机实测（2026-10-30）：国内版数据目录 `~/.codebuddycn`（只有 settings.json / argv.json），
/// 国际版沿用 `~/.codebuddy`（与 CLI 共用）。IDE 的 MCP / 技能位置尚未核实，因此只做检测。
const CODEBUDDY_IDE_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepCodebuddyIdeIntl,
        command: TableText::Raw("https://www.codebuddy.ai/ide"),
        risk: Msg::CatalogRiskCodebuddyIdeIntl,
        url: Some("https://www.codebuddy.ai/ide"),
    },
    Step {
        title: Msg::CatalogStepCodebuddyIdeCn,
        command: TableText::Raw("https://www.codebuddy.cn/ide/"),
        risk: Msg::CatalogRiskCodebuddyIdeCn,
        url: Some("https://www.codebuddy.cn/ide/"),
    },
];

const HERMES: &[Step] = &[Step {
    title: Msg::CatalogStepConfirmInstalled,
    command: TableText::Raw("hermes --version"),
    risk: Msg::CatalogRiskHermes,
    url: None,
}];

const GEMINI: &[Step] = &[
    Step {
        title: Msg::CatalogStepInstallGemini,
        command: TableText::Raw("npm install -g @google/gemini-cli"),
        risk: NPM_RISK,
        url: Some("https://github.com/google-gemini/gemini-cli"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("gemini --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

// ---- 常见桌面 / CLI 产品 ------------------------------------------------

const CLAUDE_DESKTOP_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepDownloadClaudeDesktop,
        command: TableText::Raw("https://claude.ai/download"),
        risk: APP_RISK,
        url: Some("https://claude.ai/download"),
    },
    Step {
        title: Msg::CatalogStepRestartAfterLogin,
        command: TableText::Msg(Msg::CatalogCommandClaudeDesktopRestart),
        risk: Msg::CatalogRiskRestartNeeded,
        url: None,
    },
];

const CHATGPT_DESKTOP_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepDownloadChatgpt,
        command: TableText::Raw("https://chatgpt.com/download"),
        risk: APP_RISK,
        url: Some("https://chatgpt.com/download"),
    },
    Step {
        title: Msg::CatalogStepEnableMcp,
        command: TableText::Raw("Settings → Beta Features → Developer Mode"),
        risk: Msg::CatalogRiskChatgptDevMode,
        url: Some("https://learn.chatgpt.com/docs/extend/mcp"),
    },
];

const WORKBUDDY_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepDownloadWorkbuddy,
        command: TableText::Raw("https://www.workbuddy.cn/"),
        risk: Msg::CatalogRiskWorkbuddyClient,
        url: Some("https://www.workbuddy.cn/"),
    },
    Step {
        title: Msg::CatalogStepIntlIsOtherClient,
        command: TableText::Raw("https://www.workbuddy.ai/"),
        risk: Msg::CatalogRiskWorkbuddyAiCoexist,
        url: Some("https://www.workbuddy.ai/"),
    },
];

/// WorkBuddy AI（国际版）：exe 是 `WorkBuddyAI.exe`，dataFolderName 为 `.workbuddy-ai`。
const WORKBUDDY_AI_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepDownloadWorkbuddyAi,
        command: TableText::Raw("https://www.workbuddy.ai/"),
        risk: Msg::CatalogRiskWorkbuddyAiPaths,
        url: Some("https://www.workbuddy.ai/"),
    },
    Step {
        title: Msg::CatalogStepCnIsOtherClient,
        command: TableText::Raw("https://www.workbuddy.cn/"),
        risk: Msg::CatalogRiskWorkbuddyPaths,
        url: Some("https://www.workbuddy.cn/"),
    },
];

const TRAE_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepDownloadTrae,
        command: TableText::Raw("https://www.trae.ai/"),
        risk: Msg::CatalogRiskTraePaths,
        url: Some("https://www.trae.ai/"),
    },
    Step {
        title: Msg::CatalogStepCnIsOtherPackage,
        command: TableText::Raw("https://www.trae.cn/"),
        risk: Msg::CatalogRiskTraeCnPaths,
        url: Some("https://www.trae.cn/"),
    },
    Step {
        title: Msg::CatalogStepProjectMcp,
        command: TableText::Msg(Msg::CatalogCommandTraeProjectMcp),
        risk: Msg::CatalogRiskTraeNoUserMcp,
        url: None,
    },
];

/// Trae CN（国内版）：目录带 `CN` 后缀，本机实测数据根为 `~/.trae-cn`。
const TRAE_CN_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepDownloadTraeCn,
        command: TableText::Raw("https://www.trae.cn/"),
        risk: Msg::CatalogRiskTraeCnPathsDetail,
        url: Some("https://www.trae.cn/"),
    },
    Step {
        title: Msg::CatalogStepIntlIsOtherPackage,
        command: TableText::Raw("https://www.trae.ai/"),
        risk: Msg::CatalogRiskTraeIntlNoCnSuffix,
        url: Some("https://www.trae.ai/"),
    },
    Step {
        title: Msg::CatalogStepProjectMcp,
        command: TableText::Msg(Msg::CatalogCommandTraeProjectMcp),
        risk: Msg::CatalogRiskTraeCnSharedFile,
        url: None,
    },
];

const OPENCODE_STEPS_WIN: &[Step] = &[
    Step {
        title: Msg::CatalogStepInstallOpencode,
        command: TableText::Raw("npm install -g opencode-ai"),
        risk: NPM_RISK,
        url: Some("https://opencode.ai/docs/"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("opencode --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

const OPENCODE_STEPS_MAC: &[Step] = &[
    Step {
        title: Msg::CatalogStepHomebrew,
        command: TableText::Raw("brew install anomalyco/tap/opencode"),
        risk: Msg::CatalogRiskOpencodeTap,
        url: Some("https://opencode.ai/docs/"),
    },
    Step {
        title: Msg::CatalogStepInstallScript,
        command: TableText::Raw("curl -fsSL https://opencode.ai/install | bash"),
        risk: Msg::CatalogRiskOpencodeScript,
        url: Some("https://opencode.ai/install"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("opencode --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

const PI_STEPS: &[Step] = &[
    Step {
        title: Msg::CatalogStepInstallPi,
        command: TableText::Raw("npm install -g --ignore-scripts @earendil-works/pi-coding-agent"),
        risk: Msg::CatalogRiskPiNode,
        url: Some("https://pi.dev/"),
    },
    Step {
        title: Msg::CatalogStepVerify,
        command: TableText::Raw("pi --version"),
        risk: VERIFY_RISK,
        url: None,
    },
];

const OPENWORK_STEPS: &[Step] = &[Step {
    title: Msg::CatalogStepConfirmPackage,
    command: TableText::Raw("npm install -g @different-ai/openwork"),
    risk: Msg::CatalogRiskOpenworkPackage,
    url: Some("https://github.com/different-ai/openwork"),
}];

const MANUS_STEPS: &[Step] = &[Step {
    title: Msg::CatalogStepDownloadManus,
    command: TableText::Raw("https://manus.im/"),
    risk: Msg::CatalogRiskManusCloud,
    url: Some("https://manus.im/"),
}];

// ---- 受管模板 ----------------------------------------------------------

const CLAUDE_DESKTOP_MCP: McpFormatSpec = McpFormatSpec::Json(&["mcpServers"]);

const CLAUDE_DESKTOP_TEMPLATE: Template = Template {
    id: "claude-desktop",
    display_name: "Claude Desktop",
    skills_dir: None,
    project_skills_dir: None,
    mcp_file: None,
    mcp_file_alternates: &[],
    mcp_format: None,
    project_mcp_file: None,
    instructions: &[],
    note: Msg::CatalogNoteClaudeDesktop,
};

const TRAE_TEMPLATE: Template = Template {
    id: "trae",
    display_name: "Trae",
    skills_dir: None,
    project_skills_dir: None,
    mcp_file: None,
    mcp_file_alternates: &[],
    mcp_format: Some(McpFormatSpec::Json(&["mcpServers"])),
    project_mcp_file: Some(".trae/mcp.json"),
    instructions: &[],
    note: Msg::CatalogNoteTrae,
};

/// Trae CN（国内版）：沿用同一项目级文件约定，但两版是否同名尚未核实。
///
/// 单独一个 id（`trae-cn`）而不是复用 `trae`，这样两版同机安装时可以各写各的实验项目；
/// `api::plan_mcp_write` 会按路径去重，两版都登记时同一个 `.trae/mcp.json` 只写一次。
const TRAE_CN_TEMPLATE: Template = Template {
    id: "trae-cn",
    display_name: "Trae CN",
    skills_dir: None,
    project_skills_dir: None,
    mcp_file: None,
    mcp_file_alternates: &[],
    mcp_format: Some(McpFormatSpec::Json(&["mcpServers"])),
    project_mcp_file: Some(".trae/mcp.json"),
    instructions: &[],
    note: Msg::CatalogNoteTraeCn,
};

/// WorkBuddy（国内版）：实测 `~/.workbuddy/skills`（标准 SKILL.md）与 `~/.workbuddy/mcp.json`
///（顶层 `mcpServers`）。它另外有 `mcp-approvals.json`，新写入的 server 可能要再批准一次。
const WORKBUDDY_TEMPLATE: Template = Template {
    id: "workbuddy",
    display_name: "WorkBuddy",
    skills_dir: Some("~/.workbuddy/skills"),
    project_skills_dir: None,
    mcp_file: Some("~/.workbuddy/mcp.json"),
    mcp_file_alternates: &[],
    mcp_format: Some(McpFormatSpec::Json(&["mcpServers"])),
    project_mcp_file: None,
    instructions: &["~/.workbuddy/SOUL.md", "~/.workbuddy/USER.md"],
    note: Msg::CatalogNoteWorkbuddy,
};

/// WorkBuddy AI（国际版）：dataFolderName 是 `.workbuddy-ai`，配置形状与国内版逐字相同。
///
/// 本机 2026-10-30 探测到 `~/.workbuddy-ai/skills` 与 `mcp-approvals.json`，但顶层 `mcp.json`
/// 当时不存在（首次写入由 AgentHub 创建）；`SOUL.md` / `USER.md` 也未出现，故不接管人格文件。
const WORKBUDDY_AI_TEMPLATE: Template = Template {
    id: "workbuddy-ai",
    display_name: "WorkBuddy AI",
    skills_dir: Some("~/.workbuddy-ai/skills"),
    project_skills_dir: None,
    mcp_file: Some("~/.workbuddy-ai/mcp.json"),
    mcp_file_alternates: &[],
    mcp_format: Some(McpFormatSpec::Json(&["mcpServers"])),
    project_mcp_file: None,
    instructions: &[],
    note: Msg::CatalogNoteWorkbuddyAi,
};

// ---- 表 ----------------------------------------------------------------

pub fn specs() -> &'static [Spec] {
    SPECS
}

static SPECS: &[Spec] = &[
    Spec {
        key: "claude-code",
        family: "claude",
        name: TableText::Raw("Claude Code"),
        vendor: TableText::Raw("Anthropic"),
        kind: super::AgentKind::Cli,
        official: "https://docs.claude.com/en/docs/claude-code/setup",
        needs_confirm: false,
        builtin: Some(AgentId::Claude),
        cli: &["claude"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        win_config: &[],
        mac_config: &[],
        shared_agent: None,
        win_steps: CLAUDE_WIN,
        mac_steps: CLAUDE_MAC,
        win_managed: None,
        mac_managed: None,
        blocked: None,
    },
    Spec {
        key: "claude-desktop",
        family: "claude",
        name: TableText::Raw("Claude Desktop"),
        vendor: TableText::Raw("Anthropic"),
        kind: super::AgentKind::Desktop,
        official: "https://claude.ai/download",
        needs_confirm: false,
        builtin: None,
        cli: &[],
        win_apps: &[],
        mac_apps: &["/Applications/Claude.app"],
        // Microsoft Store 版：装在 Packages 下，配置走 MSIX 虚拟化目录（本机实测）
        win_msix: &["Claude_pzs8sxrjxfjjc"],
        win_config: &[
            "%LOCALAPPDATA%/Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude/claude_desktop_config.json",
            "%APPDATA%/Claude/claude_desktop_config.json",
            "%LOCALAPPDATA%/Claude/claude_desktop_config.json",
        ],
        mac_config: &["~/Library/Application Support/Claude/claude_desktop_config.json"],
        shared_agent: None,
        win_steps: CLAUDE_DESKTOP_STEPS,
        mac_steps: CLAUDE_DESKTOP_STEPS,
        win_managed: Some(Template {
            // 首选 Store 路径；装了桌面安装版时回退到 %APPDATA%
            mcp_file: Some(
                "%LOCALAPPDATA%/Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude/claude_desktop_config.json",
            ),
            mcp_file_alternates: &[
                "%APPDATA%/Claude/claude_desktop_config.json",
                "%LOCALAPPDATA%/Claude/claude_desktop_config.json",
            ],
            mcp_format: Some(CLAUDE_DESKTOP_MCP),
            ..CLAUDE_DESKTOP_TEMPLATE
        }),
        mac_managed: Some(Template {
            mcp_file: Some("~/Library/Application Support/Claude/claude_desktop_config.json"),
            mcp_format: Some(CLAUDE_DESKTOP_MCP),
            ..CLAUDE_DESKTOP_TEMPLATE
        }),
        blocked: None,
    },
    Spec {
        key: "codex",
        family: "codex",
        name: TableText::Raw("Codex CLI"),
        vendor: TableText::Raw("OpenAI"),
        kind: super::AgentKind::Cli,
        official: "https://github.com/openai/codex",
        needs_confirm: false,
        builtin: Some(AgentId::Codex),
        cli: &["codex"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        win_config: &[],
        mac_config: &[],
        shared_agent: None,
        win_steps: CODEX,
        mac_steps: CODEX,
        win_managed: None,
        mac_managed: None,
        blocked: None,
    },
    Spec {
        key: "chatgpt-desktop",
        family: "codex",
        // 本机实测：Store 里没有独立的 ChatGPT 包，OpenAI 桌面端就是 OpenAI.Codex，
        // 它和 Codex CLI 共用 ~/.codex（config.toml 里已有 [mcp_servers.*]），
        // 所以这里不重复接管，直接标成「由内置 Codex 接管」。
        name: TableText::Msg(Msg::CatalogNameChatgptDesktop),
        vendor: TableText::Raw("OpenAI"),
        kind: super::AgentKind::Desktop,
        official: "https://chatgpt.com/download",
        needs_confirm: false,
        builtin: None,
        cli: &[],
        win_apps: &[],
        mac_apps: &["/Applications/ChatGPT.app", "/Applications/Codex.app"],
        win_msix: &["OpenAI.Codex", "OpenAI.ChatGPT"],
        win_config: &["~/.codex/config.toml"],
        mac_config: &["~/.codex/config.toml"],
        shared_agent: Some(AgentId::Codex),
        win_steps: CHATGPT_DESKTOP_STEPS,
        mac_steps: CHATGPT_DESKTOP_STEPS,
        win_managed: None,
        mac_managed: None,
        blocked: None,
    },
    Spec {
        key: "codebuddy",
        family: "tencent",
        name: TableText::Raw("CodeBuddy Code"),
        vendor: TableText::Msg(Msg::CatalogVendorTencent),
        kind: super::AgentKind::Cli,
        official: "https://www.codebuddy.cn/docs/cli/quickstart",
        needs_confirm: false,
        builtin: Some(AgentId::CodeBuddy),
        cli: &["codebuddy"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        win_config: &[],
        mac_config: &[],
        shared_agent: None,
        win_steps: CODEBUDDY_WIN,
        mac_steps: CODEBUDDY_WIN,
        win_managed: None,
        mac_managed: None,
        blocked: None,
    },
    Spec {
        key: "codebuddy-ide",
        family: "tencent",
        // 与 CLI 分开：CLI 两版共用 ~/.codebuddy，IDE 两版目录不同（国际版 ~/.codebuddy、
        // 国内版 ~/.codebuddycn，本机 2026-10-30 实测后者只有 settings.json / argv.json）。
        // IDE 的 MCP / 技能位置没有核实来源，先只做检测，不提供受管模板。
        name: TableText::Raw("CodeBuddy IDE"),
        vendor: TableText::Msg(Msg::CatalogVendorTencent),
        kind: super::AgentKind::Ide,
        official: "https://www.codebuddy.ai/ide",
        needs_confirm: true,
        builtin: None,
        cli: &[],
        win_apps: &[
            "%LOCALAPPDATA%/Programs/CodeBuddy",
            "%LOCALAPPDATA%/Programs/CodeBuddy CN",
        ],
        mac_apps: &[
            "/Applications/CodeBuddy.app",
            "/Applications/CodeBuddy CN.app",
        ],
        win_msix: &[],
        win_config: &[
            "%APPDATA%/CodeBuddy",
            "%APPDATA%/CodeBuddy CN",
            "~/.codebuddycn",
        ],
        mac_config: &[
            "~/Library/Application Support/CodeBuddy",
            "~/Library/Application Support/CodeBuddy CN",
            "~/.codebuddycn",
        ],
        shared_agent: None,
        win_steps: CODEBUDDY_IDE_STEPS,
        mac_steps: CODEBUDDY_IDE_STEPS,
        win_managed: None,
        mac_managed: None,
        // macOS 2026-11-02 复核（本机装了 CodeBuddy CN.app）：数据根在
        // `~/Library/Application Support/CodeBuddy CN`（VSCode 派生：User/settings.json、
        // globalStorage、workspaceStorage），`~/.codebuddycn` 只有 argv.json 与 extensions；
        // settings.json 里没有任何 `mcpServers` 键。MCP / 技能的文件约定仍未找到，继续只做检测。
        blocked: Some(Msg::CatalogBlockedCodebuddyIde),
    },
    Spec {
        key: "workbuddy",
        family: "tencent",
        name: TableText::Msg(Msg::CatalogNameWorkbuddy),
        vendor: TableText::Msg(Msg::CatalogVendorTencent),
        kind: super::AgentKind::Desktop,
        official: "https://www.workbuddy.cn/",
        // 两平台实测：~/.workbuddy/skills（标准 SKILL.md）与 ~/.workbuddy/mcp.json（mcpServers）。
        // Windows 于 2026-10-28 实测；macOS 于 2026-10-29 实测（`~/.workbuddy/skills` 30 个
        // 技能目录、`~/.workbuddy/mcp.json` 存在，布局与 Windows 同构）。
        // 国内版数据根是 `.workbuddy`；国际版（WorkBuddy AI）是另一个条目 `workbuddy-ai`。
        needs_confirm: false,
        builtin: None,
        cli: &[],
        win_apps: &["%LOCALAPPDATA%/Programs/WorkBuddy", "%APPDATA%/WorkBuddy"],
        mac_apps: &["/Applications/WorkBuddy.app"],
        // WorkBuddy 主程序是 Electron 安装包；MSIX 只有 Share Target，仅作辅助证据
        win_msix: &["WorkBuddyShareTarget"],
        win_config: &["~/.workbuddy/mcp.json"],
        mac_config: &["~/.workbuddy/mcp.json"],
        shared_agent: None,
        win_steps: WORKBUDDY_STEPS,
        mac_steps: WORKBUDDY_STEPS,
        win_managed: Some(WORKBUDDY_TEMPLATE),
        // macOS 实测（2026-10-29）：`~/.workbuddy/skills` 与 `~/.workbuddy/mcp.json` 都存在，
        // 路径与 Windows 完全同构（都在 `~/.workbuddy` 下，不落 `~/Library`），直接复用同一模板。
        mac_managed: Some(WORKBUDDY_TEMPLATE),
        blocked: None,
    },
    Spec {
        key: "workbuddy-ai",
        family: "tencent",
        name: TableText::Msg(Msg::CatalogNameWorkbuddyAi),
        vendor: TableText::Msg(Msg::CatalogVendorTencent),
        kind: super::AgentKind::Desktop,
        official: "https://www.workbuddy.ai/",
        // 国内版与国际版是同一份代码换 dataFolderName：国内 `.workbuddy`、国际 `.workbuddy-ai`。
        // 本机 2026-10-30 探测：~/.workbuddy-ai 有 skills/ 与 mcp-approvals.json，顶层 mcp.json
        // 当时不存在；安装目录名与 macOS .app 名来自公开实测资料，尚未在本机两版共存验证。
        needs_confirm: true,
        builtin: None,
        cli: &[],
        win_apps: &["%LOCALAPPDATA%/Programs/WorkBuddyAI"],
        mac_apps: &["/Applications/WorkBuddy AI.app"],
        win_msix: &[],
        // 顶层 mcp.json 由第一次写入创建；目录本身也列上，纯配置残留时能显示「仅有安装痕迹」
        win_config: &["~/.workbuddy-ai/mcp.json", "~/.workbuddy-ai"],
        mac_config: &["~/.workbuddy-ai/mcp.json", "~/.workbuddy-ai"],
        shared_agent: None,
        win_steps: WORKBUDDY_AI_STEPS,
        mac_steps: WORKBUDDY_AI_STEPS,
        win_managed: Some(WORKBUDDY_AI_TEMPLATE),
        mac_managed: Some(WORKBUDDY_AI_TEMPLATE),
        blocked: None,
    },
    Spec {
        key: "trae",
        family: "bytedance",
        name: TableText::Msg(Msg::CatalogNameTrae),
        vendor: TableText::Msg(Msg::CatalogVendorBytedance),
        kind: super::AgentKind::Ide,
        official: "https://www.trae.ai/",
        // 国际版目录不带 CN 后缀；国内版是独立安装包，见 `trae-cn`。
        needs_confirm: true,
        builtin: None,
        cli: &[],
        win_apps: &["%LOCALAPPDATA%/Programs/Trae", "%APPDATA%/Trae"],
        mac_apps: &["/Applications/Trae.app"],
        win_msix: &[],
        // `~/.trae` 是国际版数据根（技能与插件），Windows 上配置同时在 %APPDATA%/Trae
        win_config: &["%APPDATA%/Trae", "~/.trae"],
        mac_config: &["~/Library/Application Support/Trae", "~/.trae"],
        shared_agent: None,
        win_steps: TRAE_STEPS,
        mac_steps: TRAE_STEPS,
        win_managed: Some(TRAE_TEMPLATE),
        mac_managed: Some(TRAE_TEMPLATE),
        blocked: None,
    },
    Spec {
        key: "trae-cn",
        family: "bytedance",
        name: TableText::Msg(Msg::CatalogNameTraeCn),
        vendor: TableText::Msg(Msg::CatalogVendorBytedance),
        kind: super::AgentKind::Ide,
        official: "https://www.trae.cn/",
        // Windows 2026-10-30 探测：%APPDATA%/Trae CN、%LOCALAPPDATA%/Programs/Trae CN、~/.trae-cn
        // 都存在；当时未见 skills/ 与 mcp.json，故不接管技能。
        // macOS 2026-11-02 复核（本机装了 Trae CN.app）：`~/.trae-cn/skills` **确实存在**，里面是
        // 用户自建的 symlink（指向 `~/.agents/skills/*`），另有 `~/.trae-cn/mcps/<项目前缀>` 存
        // 项目级 MCP；但用户级 mcp.json 仍无约定，模板继续只接管项目级 `.trae/mcp.json`。
        needs_confirm: true,
        builtin: None,
        cli: &[],
        win_apps: &["%LOCALAPPDATA%/Programs/Trae CN", "%APPDATA%/Trae CN"],
        mac_apps: &["/Applications/Trae CN.app"],
        win_msix: &[],
        win_config: &["%APPDATA%/Trae CN", "~/.trae-cn"],
        mac_config: &["~/Library/Application Support/Trae CN", "~/.trae-cn"],
        shared_agent: None,
        win_steps: TRAE_CN_STEPS,
        mac_steps: TRAE_CN_STEPS,
        win_managed: Some(TRAE_CN_TEMPLATE),
        mac_managed: Some(TRAE_CN_TEMPLATE),
        blocked: None,
    },
    Spec {
        key: "opencode",
        family: "opensource",
        name: TableText::Raw("OpenCode"),
        vendor: TableText::Raw("SST"),
        kind: super::AgentKind::Cli,
        official: "https://opencode.ai/docs/",
        // 内置：自己的 skills 目录通常为空，技能实际来自 .agents/skills 与 .claude/skills
        needs_confirm: false,
        builtin: Some(AgentId::Opencode),
        cli: &["opencode"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        // 实测（macOS 2026-10-29）：真实配置文件是 `opencode.jsonc`（带注释的 JSONC），
        // `opencode.json` 反而常常不存在；两处都列上，检测取第一个存在的。
        win_config: &[
            "~/.config/opencode/opencode.json",
            "~/.config/opencode/opencode.jsonc",
        ],
        mac_config: &[
            "~/.config/opencode/opencode.json",
            "~/.config/opencode/opencode.jsonc",
        ],
        shared_agent: None,
        win_steps: OPENCODE_STEPS_WIN,
        mac_steps: OPENCODE_STEPS_MAC,
        win_managed: None,
        mac_managed: None,
        blocked: None,
    },
    Spec {
        key: "pi",
        family: "opensource",
        name: TableText::Raw("Pi"),
        vendor: TableText::Raw("Earendil Works"),
        kind: super::AgentKind::Cli,
        official: "https://pi.dev/",
        needs_confirm: true,
        builtin: None,
        cli: &["pi"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        win_config: &["~/.pi"],
        mac_config: &["~/.pi"],
        shared_agent: None,
        win_steps: PI_STEPS,
        mac_steps: PI_STEPS,
        win_managed: None,
        mac_managed: None,
        blocked: Some(Msg::CatalogBlockedPi),
    },
    Spec {
        key: "openwork",
        family: "opensource",
        name: TableText::Raw("OpenWork"),
        vendor: TableText::Raw("Different AI"),
        kind: super::AgentKind::Cli,
        official: "https://github.com/different-ai/openwork",
        needs_confirm: true,
        builtin: None,
        cli: &["openwork"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        win_config: &["~/.openwork"],
        mac_config: &["~/.openwork"],
        shared_agent: None,
        win_steps: OPENWORK_STEPS,
        mac_steps: OPENWORK_STEPS,
        win_managed: None,
        mac_managed: None,
        blocked: Some(Msg::CatalogBlockedOpenwork),
    },
    Spec {
        key: "hermes",
        family: "other",
        name: TableText::Raw("Hermes"),
        vendor: TableText::Msg(Msg::CatalogVendorInternal),
        kind: super::AgentKind::Cli,
        // 没有可核实的公开官网：UI 在空字符串时不渲染链接
        official: "",
        needs_confirm: true,
        builtin: Some(AgentId::Hermes),
        cli: &["hermes"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        win_config: &[],
        mac_config: &[],
        shared_agent: None,
        win_steps: HERMES,
        mac_steps: HERMES,
        win_managed: None,
        mac_managed: None,
        blocked: None,
    },
    Spec {
        key: "gemini-cli",
        family: "other",
        name: TableText::Raw("Gemini CLI"),
        vendor: TableText::Raw("Google"),
        kind: super::AgentKind::Cli,
        official: "https://github.com/google-gemini/gemini-cli",
        needs_confirm: false,
        builtin: Some(AgentId::Gemini),
        cli: &["gemini"],
        win_apps: &[],
        mac_apps: &[],
        win_msix: &[],
        win_config: &[],
        mac_config: &[],
        shared_agent: None,
        win_steps: GEMINI,
        mac_steps: GEMINI,
        win_managed: None,
        mac_managed: None,
        blocked: None,
    },
    Spec {
        key: "manus",
        family: "other",
        name: TableText::Raw("Manus"),
        vendor: TableText::Raw("Manus"),
        kind: super::AgentKind::Cloud,
        official: "https://manus.im/",
        needs_confirm: true,
        builtin: None,
        cli: &[],
        win_apps: &["%APPDATA%/Manus"],
        mac_apps: &["/Applications/Manus.app"],
        win_msix: &[],
        win_config: &[],
        mac_config: &[],
        shared_agent: None,
        win_steps: MANUS_STEPS,
        mac_steps: MANUS_STEPS,
        win_managed: None,
        mac_managed: None,
        blocked: Some(Msg::CatalogBlockedManus),
    },
];
