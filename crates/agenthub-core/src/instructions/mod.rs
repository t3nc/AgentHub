pub mod bridge;
pub mod managed_block;

use crate::adapters::AgentAdapter;
use crate::l10n::{Msg, t};
use crate::model::agent::Scope;
use crate::model::instruction::{ChainEntry, InstructionSpec};

/// 某个目录下各 agent 实际按序加载的指令文件链路。
/// 顺序：全局 → 项目；同目录内按 agent 自己的优先级（由适配器声明顺序决定）。
pub fn chain(adapters: &[Box<dyn AgentAdapter>], scope: &Scope) -> Vec<ChainEntry> {
    let mut out = Vec::new();
    for adapter in adapters {
        let specs: Vec<InstructionSpec> = adapter.instruction_specs(scope);
        let any_project = specs.iter().any(|s| s.exists);
        for (idx, spec) in specs.iter().enumerate() {
            let shadowed = !any_project && matches!(scope, Scope::User) && false;
            out.push(ChainEntry {
                agent: spec.agent.clone(),
                order: idx + 1,
                path: spec.path.clone(),
                exists: spec.exists,
                managed: spec.managed,
                shadowed,
                note: if spec.exists {
                    None
                } else {
                    Some(t!(Msg::StatusFileMissing))
                },
            });
        }
    }
    out
}

/// 收集所有存在的指令文件（用于 UI 文件树）。
pub fn collect(adapters: &[Box<dyn AgentAdapter>], scope: &Scope) -> Vec<InstructionSpec> {
    adapters
        .iter()
        .flat_map(|a| a.instruction_specs(scope))
        .collect()
}
