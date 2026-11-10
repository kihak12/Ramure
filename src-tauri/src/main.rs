// Empêche l'ouverture d'une console sous Windows en version release. Ne pas retirer.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `ramure --sequence-editor <liste> <fichier>` : pendant un rebase interactif, git relance
    // Ramure comme « éditeur de séquence ». On remplace sa liste par la nôtre et on rend la main,
    // sans ouvrir de fenêtre.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 4 && args[1] == "--sequence-editor" {
        let code = match std::fs::copy(&args[2], &args[3]) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("ramure : impossible d'écrire la liste du rebase : {e}");
                1
            }
        };
        std::process::exit(code);
    }
    ramure_lib::run();
}
