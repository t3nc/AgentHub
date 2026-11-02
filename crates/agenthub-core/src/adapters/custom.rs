use std::path::PathBuf;

use super::{AgentAdapter, instruction_spec};
use crate::config::CustomAgentDef;
use crate::model::agent::{AgentId, Capabilities, Scope, SkillTarget};
use crate::model::instruction::{InstructionKind, InstructionSpec};
use crate::model::mcp::McpFile;
use crate::paths;

/// 用户自定义平台（DeepSeek Harness、WPS 灵犀等）：路径由用户填写，不猜测。
pub struct CustomAdapter {
    pub def: CustomAgentDef,
}

impl CustomAdapter {
    pub fn new(def: CustomAgentDef) -> Self {
        Self { def }
    }

    /// 身份用登记 key（`custom_agents[].id`），不是展示名。
    ///
    /// 曾经用 `display_name`：CLI / 前端传的是 key（`trae`），适配器身份却是
    /// `custom:Trae`，`plan_mcp_write` 按 id 过滤时一个都对不上，计划恒为 0 个操作
    /// （2026-11-02 macOS 实测）。展示名由 `custom_agents` 现查，见 `api::settings`。
    fn agent(&self) -> AgentId {
        AgentId::Custom(self.def.id.clone())
    }

    fn expand(&self, raw: &str) -> PathBuf {
        paths::expand(raw)
    }
}

impl AgentAdapter for CustomAdapter {
    fn id(&self) -> AgentId {
        self.agent()
    }

    fn capabilities(&self) -> Capabilities {
        // 用户级与项目级 MCP 只要有一个能写，就算具备 MCP 能力；
        // 但 `config_dir()` 仍只认 skills_dir 的父目录——只有 MCP 的平台不能把
        // `%APPDATA%\Claude` 这种整目录纳入可写白名单。
        Capabilities {
            skills: self.def.skills_dir.is_some(),
            mcp: (self.def.mcp_file.is_some() || self.def.project_mcp_file.is_some())
                && self.def.mcp_format.is_some(),
            instructions: !self.def.instructions.is_empty(),
            project_scope: self.def.project_skills_dir.is_some()
                || self.def.project_mcp_file.is_some(),
            links: true,
        }
    }

    fn config_dir(&self) -> Option<PathBuf> {
        self.def
            .skills_dir
            .as_ref()
            .map(|p| self.expand(p))
            .and_then(|p| p.parent().map(|x| x.to_path_buf()))
    }

    fn cli_names(&self) -> Vec<String> {
        vec![self.def.id.clone()]
    }

    fn skill_targets(&self, scope: &Scope) -> Vec<SkillTarget> {
        match scope {
            Scope::User => self
                .def
                .skills_dir
                .as_ref()
                .map(|p| vec![SkillTarget::new(self.agent(), self.expand(p), Scope::User)])
                .unwrap_or_default(),
            Scope::Project { root } => self
                .def
                .project_skills_dir
                .as_ref()
                .map(|rel| {
                    vec![SkillTarget::new(
                        self.agent(),
                        root.join(rel),
                        scope.clone(),
                    )]
                })
                .unwrap_or_default(),
        }
    }

    fn mcp_files(&self, scope: &Scope) -> Vec<McpFile> {
        let format = match &self.def.mcp_format {
            Some(format) => format.clone(),
            None => return Vec::new(),
        };
        match scope {
            // 项目级：Trae 这类「配置只写在项目里」的产品走 `project_mcp_file`
            Scope::Project { root } => match &self.def.project_mcp_file {
                Some(rel) => {
                    let path = paths::join_rel(root, rel);
                    vec![McpFile {
                        agent: self.agent(),
                        exists: path.is_file(),
                        path,
                        format,
                    }]
                }
                None => Vec::new(),
            },
            Scope::User => match &self.def.mcp_file {
                Some(file) => {
                    let path = self.expand(file);
                    vec![McpFile {
                        agent: self.agent(),
                        exists: path.is_file(),
                        path,
                        format,
                    }]
                }
                None => Vec::new(),
            },
        }
    }

    fn instruction_specs(&self, scope: &Scope) -> Vec<InstructionSpec> {
        if matches!(scope, Scope::Project { .. }) {
            return Vec::new();
        }
        self.def
            .instructions
            .iter()
            .map(|raw| instruction_spec(self.agent(), self.expand(raw), InstructionKind::Rule))
            .collect()
    }
}
