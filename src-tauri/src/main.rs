// Keeps a console from opening on Windows in the release build. Do not remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `ramure --sequence-editor <list> <file>`: during an interactive rebase, git runs Ramure
    // again as its “sequence editor”. We replace its list with ours and hand back control,
    // without opening a window.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 4 && args[1] == "--sequence-editor" {
        let code = match std::fs::copy(&args[2], &args[3]) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("ramure: could not write the rebase list: {e}");
                1
            }
        };
        std::process::exit(code);
    }
    ramure_lib::run();
}
