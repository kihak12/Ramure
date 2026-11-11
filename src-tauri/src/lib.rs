//! Cœur de Ramure : tout ce qui touche au disque et au réseau passe par ici.
//! L'interface n'appelle que les commandes déclarées dans `invoke_handler`.

mod git;
mod local;
mod ssh;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ssh::SshState::default())
        .manage(local::LocalState::default())
        .manage(git::clone::CloneState::default())
        .setup(|app| {
            // Carnet d'hôtes : %APPDATA%\Ramure sous Windows, ~/.config/ramure ailleurs.
            let folder = if cfg!(windows) { "Ramure" } else { "ramure" };
            let store = app
                .path()
                .config_dir()
                .map_err(|e| e.to_string())
                .and_then(|dir| {
                    let dir = dir.join(folder);
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    ssh::store::Store::open(&dir.join("ramure.db"))
                })
                .or_else(|e| {
                    // Sans fichier, le carnet vit en mémoire le temps de la session.
                    log::warn!("carnet d'hôtes non enregistré : {e}");
                    ssh::store::Store::in_memory()
                })?;
            app.manage(store);
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            git::git_log,
            git::work::git_status,
            git::work::git_diff_file,
            git::work::git_commit_detail,
            git::work::git_diff_commit,
            git::work::git_stage,
            git::work::git_unstage,
            git::work::git_discard,
            git::work::git_commit,
            git::work::git_sync,
            git::work::git_apply_lines,
            git::work::git_head_message,
            git::ops::git_refs,
            git::ops::git_remote_relation,
            git::ops::git_switch_blockers,
            git::ops::git_ref_op,
            git::ops::git_stash_show,
            git::ops::git_history,
            git::ops::git_operate,
            git::ops::git_resolve,
            git::ops::git_rebase_plan,
            git::ops::git_rebase_interactive,
            git::clone::git_clone,
            git::clone::git_clone_cancel,
            git::clone::git_init,
            git::conflict::git_conflict_read,
            git::conflict::git_conflict_write,
            ssh::ssh_defaults,
            ssh::ssh_connect,
            ssh::ssh_test,
            ssh::secrets::secrets_available,
            ssh::secrets::secrets_held,
            ssh::secrets::secret_forget,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_close,
            local::local_open,
            local::local_write,
            local::local_resize,
            local::local_close,
            local::local_busy,
            ssh::store::hosts_list,
            ssh::store::host_save,
            ssh::store::host_delete,
            ssh::store::hosts_import,
            ssh::keyfiles::ssh_keys_list,
            ssh::keyfiles::ssh_key_generate,
            ssh::keyfiles::ssh_key_import,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Ramure");
}
