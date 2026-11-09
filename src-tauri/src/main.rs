// Empêche l'ouverture d'une console sous Windows en version release. Ne pas retirer.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ramure_lib::run();
}
