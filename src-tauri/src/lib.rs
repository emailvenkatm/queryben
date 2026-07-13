#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod adapters;
pub mod app;
pub mod core;
pub mod error;
pub mod ipc;
pub mod state;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    let specta = tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        ipc::connection::create_connection,
        ipc::connection::list_connections,
        ipc::connection::delete_connection,
        ipc::connection::update_connection,
        ipc::connection::test_connection,
        ipc::query::execute_query,
        ipc::query::cancel_query,
        ipc::query::get_schema,
        ipc::query::list_tables,
        ipc::query::get_table_metadata,
        ipc::query::execute_transaction,
        ipc::azure::azure_sign_in,
        ipc::azure::azure_sign_out,
        ipc::azure::azure_sign_out_account,
        ipc::azure::azure_current_account,
        ipc::azure::azure_list_accounts,
        ipc::azure::list_azure_subscriptions,
        ipc::azure::list_azure_sql_servers,
        ipc::azure::list_azure_sql_databases,
        ipc::azure::connect_azure_sql,
        ipc::firewall::add_firewall_rule,
        ipc::firewall::can_add_rule_silently,
        ipc::firewall::has_cached_azure_token,
        ipc::theme::read_theme_override_file,
        ipc::ai_assist::ai_new_session,
        ipc::ai_assist::ai_complete,
        ipc::query_plan::get_query_plan,
        ipc::schema_compare::schema_snapshot,
        ipc::schema_compare::schema_diff,
        ipc::schema_compare::schema_diff_ddl,
        ipc::notebook::notebook_list,
        ipc::notebook::notebook_read,
        ipc::notebook::notebook_write,
        ipc::notebook::notebook_rename,
        ipc::notebook::notebook_run_cell,
        ipc::export::export_result_set,
        ipc::queries_repo::save_query,
        ipc::queries_repo::list_saved_queries,
        ipc::queries_repo::delete_saved_query,
        ipc::queries_repo::rename_saved_query,
        ipc::queries_repo::log_query_history,
        ipc::queries_repo::list_query_history,
        ipc::queries_repo::clear_query_history,
        ipc::snippets::read_user_snippets_file,
        ipc::table_designer::load_table_design,
        ipc::table_designer::generate_table_ddl,
        ipc::table_designer::apply_table_ddl,
        ipc::object_scripter::script_object,
        ipc::import::import_preview,
        ipc::import::import_execute,
        ipc::ads_bridge::detect_ads_installation,
        ipc::ads_bridge::import_from_ads,
    ]);

    #[cfg(debug_assertions)]
    if let Err(err) = specta.export(
        specta_typescript::Typescript::default(),
        "../src/shared/api/tauri-bindings.ts",
    ) {
        tracing::warn!("failed to export tauri-bindings.ts: {err}");
    }

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            use tauri::Manager;
            specta.mount_events(app);
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("resolve app_data_dir: {e}"))
            })?;
            let s = state::AppState::new(&app_data_dir).map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("init AppState: {e}"))
            })?;
            app.manage(s);
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        tracing::error!("tauri runtime error: {err}");
    }
}
