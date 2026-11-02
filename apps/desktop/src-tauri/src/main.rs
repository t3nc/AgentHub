#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod fonts;
mod gateway;
mod terminal;
mod terminal_log;
mod watch;

use std::sync::Mutex;

use agenthub_core::AgentHub;
use tauri::Manager;

pub struct AppState(pub Mutex<AgentHub>);

fn main() {
    let hub = match AgentHub::bootstrap() {
        Ok(hub) => hub,
        Err(err) => {
            eprintln!("AgentHub 初始化失败: {err}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        // 终端页「选择工作目录」用的原生文件夹对话框
        .plugin(tauri_plugin_dialog::init())
        // 右键菜单里的「在文件管理器中显示 / 用默认程序打开」
        .plugin(tauri_plugin_opener::init())
        .manage(AppState(Mutex::new(hub)))
        .manage(terminal::TerminalState::default())
        .manage(gateway::GatewayState::default())
        .setup(|app| {
            // 监听受管目录：外部工具改了东西就通知前端（不自己重扫）
            let handle = app.handle().clone();
            let targets = match app.state::<AppState>().0.lock() {
                Ok(hub) => hub.watch_targets(),
                Err(err) => {
                    eprintln!("[watch] 读取受管目录失败: {err}");
                    Vec::new()
                }
            };
            watch::spawn(handle, targets);

            // 网关默认不自启：只有用户在设置里勾了「随应用启动」才拉起
            let config = match app.state::<AppState>().0.lock() {
                Ok(hub) => hub.config.clone(),
                Err(_) => return Ok(()),
            };
            if config.gateway.auto_start && config.features.provider {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<gateway::GatewayState>();
                    if let Err(err) = state.start(&config).await {
                        eprintln!("[gateway] 自动启动失败：{err}");
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hub_overview,
            commands::hub_instructions,
            commands::hub_projects,
            commands::plan_dedupe,
            commands::plan_repair,
            commands::apply_plan,
            commands::rollback,
            commands::hub_agent_catalog,
            commands::add_managed_agent,
            commands::remove_managed_agent,
            commands::set_skill_strategy,
            commands::set_agent_local_skill,
            commands::mcp_view,
            commands::plan_mcp_write,
            commands::hub_settings,
            commands::save_settings,
            commands::hub_providers,
            commands::save_provider,
            commands::delete_provider,
            commands::bind_provider,
            commands::unbind_provider,
            commands::plan_provider_apply,
            commands::hub_features,
            commands::hub_locale,
            commands::hub_set_locale,
            commands::hub_suppliers,
            commands::probe_provider_models,
            commands::apply_provider_binding,
            commands::add_to_pool,
            commands::remove_from_pool,
            commands::set_default_provider,
            commands::set_pool_mode,
            commands::set_model_slots,
            commands::set_feature,
            commands::hub_gateway,
            commands::save_gateway_config,
            commands::save_gateway_route,
            commands::delete_gateway_route,
            commands::plan_revert_to_direct,
            commands::gateway_status,
            commands::gateway_start,
            commands::gateway_stop,
            commands::gateway_restart,
            commands::gateway_logs,
            commands::clear_gateway_logs,
            commands::hub_usage,
            commands::hub_usage_pricing,
            commands::save_usage_pricing,
            commands::preview_usage_pricing,
            commands::apply_usage_pricing,
            commands::plan_delete,
            commands::market_agents,
            commands::market_search,
            commands::market_detail,
            commands::plan_market_install,
            fonts::terminal_fonts,
            terminal::terminal_info,
            terminal::terminal_open,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            terminal::terminal_history,
            terminal::terminal_export,
            terminal::terminal_save_selection,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 启动失败");
}
