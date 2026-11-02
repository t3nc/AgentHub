mod common;

use std::fs;

use agenthub_core::model::agent::{AgentId, Scope};

#[test]
fn managed_block_keeps_user_content() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);

    let claude_md = home.join(".claude/CLAUDE.md");
    fs::write(&claude_md, "# 我的规则\n\n永远用中文回答。\n").unwrap();

    let plan = hub
        .plan_instruction_inject(&AgentId::Claude, &Scope::User, "共享片段：提交前先跑测试。")
        .unwrap();
    hub.apply(&plan, &Scope::User).unwrap();

    let text = fs::read_to_string(&claude_md).unwrap();
    assert!(text.contains("永远用中文回答。"), "用户内容必须保留");
    assert!(text.contains("<!-- agenthub:begin -->"));
    assert!(text.contains("共享片段：提交前先跑测试。"));

    // 二次注入只更新区块，不重复追加
    let plan2 = hub
        .plan_instruction_inject(&AgentId::Claude, &Scope::User, "共享片段 v2")
        .unwrap();
    hub.apply(&plan2, &Scope::User).unwrap();
    let text2 = fs::read_to_string(&claude_md).unwrap();
    assert_eq!(text2.matches("<!-- agenthub:begin -->").count(), 1);
    assert!(text2.contains("共享片段 v2"));
    assert!(!text2.contains("提交前先跑测试"));
    assert!(text2.contains("永远用中文回答。"));

    let (specs, chain) = hub.instructions_overview(&Scope::User);
    assert!(!specs.is_empty());
    assert!(chain.iter().any(|c| c.managed));
}
