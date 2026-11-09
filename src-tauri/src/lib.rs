//! Cœur de Ramure : tout ce qui touche au disque et au réseau passe par ici.
//! L'interface n'appelle que les commandes déclarées dans `invoke_handler`.

mod git;
mod ssh;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ssh::SshState::default())
        .setup(|app| {
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
            ssh::ssh_defaults,
            ssh::ssh_connect,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_close,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Ramure");
}
