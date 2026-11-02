use serde::{Deserialize, Serialize};

use crate::l10n::{Msg, t};
use crate::model::agent::AgentId;

/// Phase 2：agent 一键安装配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallStep {
    pub title: String,
    /// 供用户复核后手动执行；AgentHub 不会静默跑安装脚本。
    /// 桌面类产品的 `command` 是下载页地址，不是 shell 命令。
    pub command: String,
    pub risk: String,
    /// 该步骤对应的官方文档/下载页（可空）。
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    pub agent: AgentId,
    pub installed: bool,
    pub supported: bool,
    #[serde(default)]
    pub steps: Vec<InstallStep>,
    #[serde(default)]
    pub reason: Option<String>,
    /// 步骤对应的平台，如 `windows x86_64`。
    #[serde(default)]
    pub platform: String,
    /// 官方入口；空字符串表示没有可核实的官网。
    #[serde(default)]
    pub docs: String,
    /// 命令或路径尚未被官方资料确证，UI 要显式提示。
    #[serde(default)]
    pub needs_confirm: bool,
}

pub trait Installer: Send + Sync {
    fn id(&self) -> AgentId;
    fn installed(&self) -> bool;
    fn plan(&self) -> InstallPlan;
}

/// 占位实现：明确告诉 UI「该能力未启用」，不伪造可用状态。
pub struct NotReadyInstaller {
    pub agent: AgentId,
}

impl Installer for NotReadyInstaller {
    fn id(&self) -> AgentId {
        self.agent.clone()
    }

    fn installed(&self) -> bool {
        false
    }

    fn plan(&self) -> InstallPlan {
        InstallPlan {
            agent: self.agent.clone(),
            installed: false,
            supported: false,
            steps: Vec::new(),
            reason: Some(t!(Msg::CatalogInstallNoGuide)),
            platform: platform_label(),
            docs: String::new(),
            needs_confirm: true,
        }
    }
}

/// 内置 agent 的安装指引：步骤来自 `ext::catalog` 的静态表。
///
/// 这里不判断「装没装」——检测在 `AgentHub::agent_catalog` 里做，避免同一条信息两处实现。
pub fn plan_for(agent: &AgentId) -> Option<InstallPlan> {
    let spec = crate::ext::catalog::specs()
        .iter()
        .find(|spec| spec.builtin.as_ref() == Some(agent))?;
    let steps = crate::ext::catalog::plan_steps(spec);
    Some(InstallPlan {
        agent: agent.clone(),
        installed: false,
        supported: !steps.is_empty(),
        reason: None,
        steps,
        platform: platform_label(),
        docs: spec.official.to_string(),
        needs_confirm: spec.needs_confirm,
    })
}

fn platform_label() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}
