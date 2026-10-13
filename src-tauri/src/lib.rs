// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

mod worker;

#[derive(Clone)]
pub struct WorkerControlState {
    pub cpu_limit: Arc<AtomicU32>,
    pub idle_only: Arc<AtomicBool>,
    pub is_idle: Arc<AtomicBool>,
    pub is_running: Arc<AtomicBool>,
    pub stop_signal: Arc<AtomicBool>,
    pub prevent_sleep: Arc<AtomicBool>,
    pub keep_in_menu_bar: Arc<AtomicBool>,
    pub volunteer_pseudo: Arc<tokio::sync::RwLock<Option<String>>>,
    pub caffeinate_child: Arc<std::sync::Mutex<Option<std::process::Child>>>,
}

impl Default for WorkerControlState {
    fn default() -> Self {
        Self {
            cpu_limit: Arc::new(AtomicU32::new(100)),
            idle_only: Arc::new(AtomicBool::new(false)),
            is_idle: Arc::new(AtomicBool::new(true)),
            is_running: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            prevent_sleep: Arc::new(AtomicBool::new(true)),
            keep_in_menu_bar: Arc::new(AtomicBool::new(false)),
            volunteer_pseudo: Arc::new(tokio::sync::RwLock::new(None)),
            caffeinate_child: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

fn enable_prevent_sleep(child_lock: &Arc<std::sync::Mutex<Option<std::process::Child>>>) {
    #[cfg(target_os = "macos")]
    {
        let mut guard = child_lock.lock().unwrap();
        if guard.is_none() {
            if let Ok(child) = std::process::Command::new("caffeinate")
                .args(["-d", "-i"])
                .spawn()
            {
                *guard = Some(child);
            }
        }
    }
}

fn disable_prevent_sleep(child_lock: &Arc<std::sync::Mutex<Option<std::process::Child>>>) {
    #[cfg(target_os = "macos")]
    {
        let mut guard = child_lock.lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub body: Option<String>,
    pub date: Option<String>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct AccountSettingsInfo {
    pub pseudo: Option<String>,
    pub public_key: Option<String>,
    pub storage_dir: String,
    pub pending_submissions: usize,
    pub server_url: String,
    pub prevent_sleep: bool,
    pub keep_in_menu_bar: bool,
    pub is_running: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ServerTestResponse {
    pub ok: bool,
    pub status: u16,
    pub latency_ms: u64,
    pub message: String,
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current_version = app.package_info().version.to_string();
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;

    if let Some(update) = update {
        Ok(UpdateInfo {
            available: true,
            current_version,
            version: Some(update.version),
            body: update.body,
            date: update.date.map(|d| d.to_string()),
        })
    } else {
        Ok(UpdateInfo {
            available: false,
            current_version,
            version: None,
            body: None,
            date: None,
        })
    }
}

#[tauri::command]
async fn install_update(window: tauri::Window) -> Result<(), String> {
    let app = window.app_handle().clone();
    let updater = app.updater().map_err(|e| e.to_string())?;
    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
        let win = window.clone();
        let win_finished = window.clone();
        update
            .download_and_install(
                move |downloaded, total| {
                    let total_bytes = total.unwrap_or(0);
                    let _ = win.emit("update-download-progress", (downloaded, total_bytes));
                },
                move || {
                    let _ = win_finished.emit("update-download-finished", ());
                },
            )
            .await
            .map_err(|e| e.to_string())?;

        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        app.restart();
    }
    Ok(())
}

#[tauri::command]
fn set_resource_allocation(
    state: State<'_, WorkerControlState>,
    cpu_percentage: u32,
    idle_only: bool,
) -> Result<(), String> {
    let clamped = cpu_percentage.clamp(10, 100);
    state.cpu_limit.store(clamped, Ordering::Relaxed);
    state.idle_only.store(idle_only, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_system_idle_state(
    state: State<'_, WorkerControlState>,
    is_idle: bool,
) -> Result<(), String> {
    state.is_idle.store(is_idle, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
async fn set_volunteer_pseudo(
    state: State<'_, WorkerControlState>,
    pseudo: String,
) -> Result<(), String> {
    let mut lock = state.volunteer_pseudo.write().await;
    let trimmed = pseudo.trim();
    if trimmed.is_empty() {
        *lock = None;
    } else {
        *lock = Some(trimmed.to_string());
    }
    Ok(())
}

#[tauri::command]
fn stop_worker(state: State<'_, WorkerControlState>) -> Result<(), String> {
    state.stop_signal.store(true, Ordering::SeqCst);
    state.is_running.store(false, Ordering::SeqCst);
    disable_prevent_sleep(&state.caffeinate_child);
    Ok(())
}

#[tauri::command]
fn is_worker_running(state: State<'_, WorkerControlState>) -> Result<bool, String> {
    Ok(state.is_running.load(Ordering::SeqCst))
}

#[tauri::command]
fn set_prevent_sleep(state: State<'_, WorkerControlState>, enable: bool) -> Result<(), String> {
    state.prevent_sleep.store(enable, Ordering::Relaxed);
    if enable && state.is_running.load(Ordering::Relaxed) {
        enable_prevent_sleep(&state.caffeinate_child);
    } else if !enable {
        disable_prevent_sleep(&state.caffeinate_child);
    }
    Ok(())
}

#[tauri::command]
fn get_prevent_sleep(state: State<'_, WorkerControlState>) -> Result<bool, String> {
    Ok(state.prevent_sleep.load(Ordering::Relaxed))
}

#[tauri::command]
fn set_keep_in_menu_bar(state: State<'_, WorkerControlState>, enable: bool) -> Result<(), String> {
    state.keep_in_menu_bar.store(enable, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn get_keep_in_menu_bar(state: State<'_, WorkerControlState>) -> Result<bool, String> {
    Ok(state.keep_in_menu_bar.load(Ordering::Relaxed))
}

#[tauri::command]
async fn get_account_settings(
    window: tauri::Window,
    state: State<'_, WorkerControlState>,
) -> Result<AccountSettingsInfo, String> {
    let pseudo = state.volunteer_pseudo.read().await.clone();
    let storage_dir = worker::resolve_app_storage_dir(&window);
    let id_path = storage_dir.join("helixgrid_identity.key");
    let public_key = worker::VolunteerIdentity::load_or_create(&id_path)
        .ok()
        .map(|id| id.public_key_hex);
    let buffer_path = storage_dir.join("helixgrid_buffer.db");
    let pending_submissions = worker::count_pending_submissions(&buffer_path).unwrap_or(0);
    let server_url = worker::get_server_url();
    let prevent_sleep = state.prevent_sleep.load(Ordering::Relaxed);
    let keep_in_menu_bar = state.keep_in_menu_bar.load(Ordering::Relaxed);
    let is_running = state.is_running.load(Ordering::Relaxed);

    Ok(AccountSettingsInfo {
        pseudo,
        public_key,
        storage_dir: storage_dir.to_string_lossy().to_string(),
        pending_submissions,
        server_url,
        prevent_sleep,
        keep_in_menu_bar,
        is_running,
    })
}

#[tauri::command]
fn open_storage_dir(window: tauri::Window) -> Result<(), String> {
    let dir = worker::resolve_app_storage_dir(&window);
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&dir).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }
    Ok(())
}

#[tauri::command]
fn set_server_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.is_empty() {
        std::env::set_var("HELIXGRID_SERVER_URL", trimmed);
    }
    Ok(())
}

#[tauri::command]
async fn test_server_connection(url: Option<String>) -> Result<ServerTestResponse, String> {
    let base_url = match url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => worker::get_server_url(),
    };
    let target_url = format!("{}/leaderboard?limit=1", base_url.trim_end_matches('/'));
    let start = std::time::Instant::now();
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Ok(ServerTestResponse {
                ok: false,
                status: 0,
                latency_ms: 0,
                message: format!("Erreur client HTTP : {}", e),
            });
        }
    };

    match client.get(&target_url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let latency_ms = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                Ok(ServerTestResponse {
                    ok: true,
                    status,
                    latency_ms,
                    message: format!("Connecté en {} ms (HTTP 200 OK)", latency_ms),
                })
            } else {
                Ok(ServerTestResponse {
                    ok: false,
                    status,
                    latency_ms,
                    message: format!("Réponse HTTP {} (Erreur serveur)", status),
                })
            }
        }
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(ServerTestResponse {
                ok: false,
                status: 0,
                latency_ms,
                message: format!("Injoignable : {}", e),
            })
        }
    }
}

#[tauri::command]
async fn fetch_leaderboard(
    target: Option<String>,
    url: Option<String>,
) -> Result<serde_json::Value, String> {
    let base_url = match url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => worker::get_server_url(),
    };
    let mut target_url = format!("{}/leaderboard", base_url.trim_end_matches('/'));
    if let Some(t) = target {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            target_url.push_str(&format!("?target={}", trimmed));
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&target_url)
        .send()
        .await
        .map_err(|e| format!("Impossible de contacter le serveur central : {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Erreur serveur HTTP {}", resp.status()));
    }

    let val = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Erreur décodage JSON : {}", e))?;

    Ok(val)
}

#[tauri::command]
async fn fetch_molecule_sdf(smiles_or_id: String) -> Result<String, String> {
    let base_url = worker::get_server_url();
    let target_url = format!("{}/molecules/{}/sdf", base_url.trim_end_matches('/'), smiles_or_id);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&target_url)
        .send()
        .await
        .map_err(|e| format!("Erreur réseau SDF : {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("SDF non trouvé (HTTP {})", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("Erreur lecture SDF : {}", e))?;

    Ok(text)
}

#[tauri::command]
async fn start_worker(
    window: tauri::Window,
    state: State<'_, WorkerControlState>,
    target: Option<String>,
    engine: Option<String>,
    pseudo: Option<String>,
) -> Result<(), String> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err("Le calcul est déjà en cours".to_string());
    }

    if let Some(p) = pseudo {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            let mut lock = state.volunteer_pseudo.write().await;
            *lock = Some(trimmed.to_string());
        }
    }

    let app_handle = window.app_handle().clone();
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    let target_val = target.unwrap_or_else(|| "EGFR_T790M".to_string());
    let engine_val = engine.unwrap_or_else(|| "generative_evolution".to_string());
    let target_cfg = worker::get_target_config(&target_val);

    // Recherche robuste du modèle ONNX correspondant à la cible
    let model_candidates = [
        resource_dir.join(target_cfg.model_file),
        std::path::PathBuf::from(target_cfg.model_file),
        std::path::PathBuf::from("../").join(target_cfg.model_file),
        std::path::PathBuf::from("helixgrid-ui/src-tauri").join(target_cfg.model_file),
    ];

    let model_path = model_candidates
        .iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| resource_dir.join(target_cfg.model_file).to_string_lossy().to_string());

    let win = window.clone();
    let cpu_limit = state.cpu_limit.clone();
    let idle_only = state.idle_only.clone();
    let is_idle = state.is_idle.clone();
    let volunteer_pseudo = state.volunteer_pseudo.clone();

    state.stop_signal.store(false, Ordering::SeqCst);
    state.is_running.store(true, Ordering::SeqCst);
    let stop_signal = state.stop_signal.clone();
    let is_running = state.is_running.clone();

    if state.prevent_sleep.load(Ordering::Relaxed) {
        enable_prevent_sleep(&state.caffeinate_child);
    }

    tauri::async_runtime::spawn(async move {
        worker::run_worker_loop(
            win,
            model_path,
            target_val,
            engine_val,
            cpu_limit,
            idle_only,
            is_idle,
            volunteer_pseudo,
            stop_signal,
            is_running,
        )
        .await;
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(WorkerControlState::default())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let show_i = MenuItem::with_id(app, "show", "Afficher HelixGrid", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quitter HelixGrid", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("HelixGrid — Distributed Drug Discovery")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            let state = app.state::<WorkerControlState>();
                            state.stop_signal.store(true, Ordering::SeqCst);
                            state.is_running.store(false, Ordering::SeqCst);
                            disable_prevent_sleep(&state.caffeinate_child);
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _ = tray_builder.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<WorkerControlState>();
                if state.keep_in_menu_bar.load(Ordering::Relaxed) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_worker,
            stop_worker,
            is_worker_running,
            set_resource_allocation,
            set_system_idle_state,
            set_volunteer_pseudo,
            set_prevent_sleep,
            get_prevent_sleep,
            set_keep_in_menu_bar,
            get_keep_in_menu_bar,
            get_account_settings,
            open_storage_dir,
            set_server_url,
            test_server_connection,
            fetch_leaderboard,
            fetch_molecule_sdf,
            check_update,
            install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
