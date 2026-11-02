mod common;

use std::fs;

use agenthub_core::model::agent::Scope;

#[test]
fn project_scope_and_bridge() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let mut hub = common::hub(&home, &data);

    let project = data.join("proj");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("AGENTS.md"), "# 项目规则\n").unwrap();

    hub.init_project(&project).unwrap();
    assert!(project.join(".agenthub/project.json").is_file());

    let dedupe = hub.plan_dedupe(None, None).unwrap();
    hub.apply(&dedupe, &Scope::User).unwrap();

    let cs = hub
        .enable_project_skills(&project, &["cloudflare".to_string()])
        .unwrap();
    let scope = Scope::Project {
        root: project.clone(),
    };
    hub.apply(&cs, &scope).unwrap();

    for rel in [".claude/skills/cloudflare", ".codex/skills/cloudflare"] {
        let path = project.join(rel);
        common::assert_linked(&path);
    }

    let bridge = hub.plan_bridge(&scope).unwrap();
    hub.apply(&bridge, &scope).unwrap();
    let claude = fs::read_to_string(project.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("@AGENTS.md"));

    let again = hub.plan_bridge(&scope).unwrap();
    assert!(again.is_empty(), "桥接必须幂等");
}
