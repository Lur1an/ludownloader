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
async fn quick_download(url: &str) -> Result<String, String> {
    println!("Download started");
    match downloader::quick_download(url).await {
        Ok(_) => Ok(String::from("SUCCESS")),
        Err(err) => Err(format!("Download failed: {:?}", err))
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet, quick_download])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
