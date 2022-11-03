#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    println!("Arguments of greet: {}", name);
    let msg = format!("Hello, {}! You've been greeted from Rust!", name);
    println!("Greet has been called! Name has been modified!");
    return msg;
}


fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            greet
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
