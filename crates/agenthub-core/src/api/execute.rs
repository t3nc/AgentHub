use crate::l10n::{Msg, t};
use std::path::Path;

use crate::api::AgentHub;
use crate::apply;
use crate::error::Result;
use crate::model::agent::Scope;
use crate::model::change::{ApplyResult, BatchRecord, ChangeSet};
use crate::paths;
use crate::project;

impl AgentHub {
    pub fn apply(&self, change_set: &ChangeSet, scope: &Scope) -> Result<ApplyResult> {
        let roots = self.roots(scope);
        apply::apply(change_set, &roots, &self.config, &self.db)
    }

    pub fn rollback(&self, batch_id: &str) -> Result<Vec<String>> {
        let roots = self.roots(&Scope::User);
        apply::rollback(batch_id, &roots, &self.db)
    }

    pub fn batches(&self, limit: usize) -> Result<Vec<BatchRecord>> {
        self.db.recent_batches(limit)
    }

    /// 应用一个项目初始化计划后，把项目登记到最近项目列表。
    pub fn init_project(&mut self, root: &Path) -> Result<ApplyResult> {
        let scope = Scope::Project {
            root: root.to_path_buf(),
        };
        let cs = self.plan_project_init(root)?;
        let result = self.apply(&cs, &scope)?;
        self.config
            .remember_project(&paths::normalize_lexical(root));
        self.config.save()?;
        Ok(result)
    }

    /// 给项目启用一组技能：写入项目状态 + 建立项目级链接。
    pub fn enable_project_skills(&mut self, root: &Path, skills: &[String]) -> Result<ChangeSet> {
        let scope = Scope::Project {
            root: root.to_path_buf(),
        };
        let store = self.config.store_root_path();
        let mut cs = ChangeSet::new(
            crate::model::change::ChangeKind::ProjectInit,
            t!(Msg::PlanTitleEnableProjectSkills),
        );

        let mut state = project::load(root).unwrap_or_else(|| {
            let name = root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".into());
            project::ProjectState::new(name)
        });

        for name in skills {
            let source = store.join(name);
            if !source.exists() {
                cs.risk(
                    crate::model::change::RiskLevel::Warn,
                    t!(Msg::PlanSkipNotInStore, name = name),
                );
                continue;
            }
            for target in self.skill_targets(&scope) {
                let at = target.dir.join(name);
                if std::fs::symlink_metadata(&at).is_ok() {
                    continue;
                }
                cs.push(crate::model::change::FileOp::create_link(
                    at,
                    source.clone(),
                    self.strategy_for_skill(name, &target.agent),
                ));
            }
            if !state.skills.iter().any(|s| s == name) {
                state.skills.push(name.clone());
            }
        }

        state.updated_at = paths::now_ts();
        let state_path = project::state_path(root);
        let content = serde_json::to_string_pretty(&state)?;
        cs.push(crate::model::change::FileOp::write_file(
            state_path, content, true,
        ));
        Ok(cs)
    }
}
