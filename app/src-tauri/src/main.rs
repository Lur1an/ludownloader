#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]


#[tauri::command]
fn greet(name: &str) -> String {
    let msg = format!("Greetings {}!", name);
    return msg;
    
}

#[tauri::command]
fn direct_download(url: &str) -> Result<String, String> {
    match downloader::direct_download(url) {

    };
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
