mod engine;
mod extractor;
mod files;
mod inspector;
mod installer;
mod library;
mod model;
mod origin;
mod panel;
mod preview;
mod settings;
mod urls;
mod watcher;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
#[derive(Default)]
struct DialogState(AtomicBool);

#[tauri::command]
fn set_dialog_active(app: tauri::AppHandle, active: bool) {
    app.state::<DialogState>().0.store(active, Ordering::SeqCst);
    if !active {
        panel::show(&app, None);
    }
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<settings::Settings, String> {
    settings::read(&app)
}
#[tauri::command]
async fn save_settings(
    app: tauri::AppHandle,
    settings: settings::Settings,
) -> Result<settings::Settings, String> {
    let saved = settings::write(&app, settings)?;
    if let Some(tray) = app.tray_by_id("fontcatch") {
        let menu = panel::menu(&app, &saved.language).map_err(|e| e.to_string())?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    app.emit("settings-changed", &saved)
        .map_err(|e| e.to_string())?;
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    let updated = saved.clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::Configure(updated, reply))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(saved)
}
fn configured_url(key: &str) -> Option<String> {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../../src/config.json")).ok()?;
    let url = config.get(key)?.as_str()?;
    let parsed = urls::external(url)?;
    (parsed.scheme() == "https").then(|| parsed.to_string())
}
#[tauri::command]
fn open_link(app: tauri::AppHandle, key: String) -> Result<(), String> {
    if !["coffeeUrl", "websiteUrl", "creatorUrl"].contains(&key.as_str()) {
        return Err("Unknown link".into());
    }
    let url = configured_url(&key).ok_or("This link has not been configured yet.")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn hide_panel(app: tauri::AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or("Panel unavailable")?
        .hide()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_snapshot(app: tauri::AppHandle) -> Result<model::Snapshot, String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || handle.request(engine::Message::Snapshot))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn install_font(
    app: tauri::AppHandle,
    id: String,
    confirm_update: bool,
) -> Result<model::Snapshot, String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::Install(id, confirm_update, reply))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn ignore_font(app: tauri::AppHandle, id: String) -> Result<model::Snapshot, String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::Ignore(id, reply))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn read_font_preview(
    app: tauri::AppHandle,
    postscript: String,
) -> Result<tauri::ipc::Response, String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::Preview(postscript, reply))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}
#[tauri::command]
async fn reveal_font(app: tauri::AppHandle, postscript: String) -> Result<(), String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    let font = tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::Font(postscript, reply))
    })
    .await
    .map_err(|e| e.to_string())??;
    let platform = installer::native()?;
    let path = preview::managed_path(&font, platform.root())?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn open_font_license(app: tauri::AppHandle, postscript: String) -> Result<(), String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    let font = tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::Font(postscript, reply))
    })
    .await
    .map_err(|e| e.to_string())??;
    let url = font.face.license_url.ok_or("license_link_missing")?;
    let parsed = urls::external(&url).ok_or("license_link_invalid")?;
    app.opener()
        .open_url(parsed.to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn plan_uninstall(
    app: tauri::AppHandle,
    family: String,
) -> Result<model::UninstallPlan, String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::UninstallPlan(family, reply))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn uninstall_family(
    app: tauri::AppHandle,
    family: String,
    fingerprint: String,
    confirmed: bool,
) -> Result<model::UninstallOutcome, String> {
    let handle = app.state::<engine::EngineHandle>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle.request(|reply| engine::Message::Uninstall(family, fingerprint, confirmed, reply))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn run() {
    tauri::Builder::default()
        .manage(DialogState::default())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            panel::show(app, None)
        }))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--autostart"])
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_snapshot,
            install_font,
            ignore_font,
            read_font_preview,
            reveal_font,
            open_font_license,
            plan_uninstall,
            uninstall_family,
            save_settings,
            open_link,
            hide_panel,
            set_dialog_active
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let dev_origin = if cfg!(debug_assertions) {
                app.config().build.dev_url.clone()
            } else {
                None
            };
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
                .on_navigation(move |url| urls::local_navigation(url, dev_origin.as_ref()))
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()?;
            #[cfg(target_os = "macos")]
            panel::round_corners(app.handle())?;
            let data = app.path().app_data_dir()?;
            files::private_dir(&data).map_err(std::io::Error::other)?;
            library::initialize(&data.join("library.sqlite"))?;
            // Fail explicitly rather than reset damaged settings silently.
            let settings = settings::read(app.handle()).map_err(std::io::Error::other)?;
            panel::create(app.handle(), &settings.language)?;
            let engine = engine::start(app.handle().clone(), data, settings)
                .map_err(std::io::Error::other)?;
            app.manage(engine);
            if !std::env::args().any(|a| a == "--autostart") {
                panel::show(app.handle(), None);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Focused(false) => {
                if !window.state::<DialogState>().0.load(Ordering::SeqCst) {
                    let _ = window.hide();
                }
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            #[cfg(target_os = "windows")]
            tauri::WindowEvent::ThemeChanged(_) => {
                panel::refresh_icon(window.app_handle());
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("Fontcatch could not start")
        .run(|app, event| {
            // Finder can reopen an already-running accessory app whose panel is hidden.
            #[cfg(target_os = "macos")]
            if matches!(event, tauri::RunEvent::Reopen { .. }) {
                panel::show(app, None);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
