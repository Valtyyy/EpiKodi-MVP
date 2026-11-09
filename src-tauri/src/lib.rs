pub mod player;
pub mod proxy;
pub mod smb;

use player::Player;
use proxy::Proxy;
use serde::Serialize;
use smb::{Connexions, Entry};
use tauri::{Emitter, Manager, State};

struct AppState {
    connexions: Connexions,
    proxy: Proxy,
    player: Player,
}

#[derive(Serialize)]
struct Connected {
    id: String,
    label: String,
}

#[tauri::command]
async fn connect(
    state: State<'_, AppState>,
    host: String,
    share: String,
    username: String,
    password: String,
) -> Result<Connected, String> {
    let (id, label) = smb::connect(&state.connexions, &host, &share, &username, password).await?;
    Ok(Connected { id, label })
}

#[tauri::command]
async fn list_dir(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<Vec<Entry>, String> {
    smb::list_dir(&state.connexions, &connection_id, &path).await
}

#[tauri::command]
async fn play(state: State<'_, AppState>, connection_id: String, path: String) -> Result<(), String> {
    if !smb::has_connexion(&state.connexions, &connection_id) {
        return Err("Connexion inconnue".into());
    }
    state.player.play(state.proxy.url(&connection_id, &path));
    Ok(())
}

#[tauri::command]
async fn disconnect(state: State<'_, AppState>, connection_id: String) -> Result<(), String> {
    smb::disconnect(&state.connexions, &connection_id).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let connexions = Connexions::default();
            let proxy = tauri::async_runtime::block_on(Proxy::start(connexions.clone()))?;
            let handle = app.handle().clone();
            let player = Player::new(move |msg| {
                let _ = handle.emit("playback-error", msg);
            });
            app.manage(AppState {
                connexions,
                proxy,
                player,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![connect, list_dir, play, disconnect])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de EpiKodi");
}
