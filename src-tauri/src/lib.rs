//! Toolbay Flow: free, local, open-source voice dictation for any app.

mod audio;
mod commands;
mod context;
mod db;
mod dictation;
mod engines;
mod hotkeys;
mod llm;
mod paste;
mod settings;
mod sounds;
mod textproc;

use dictation::St;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

/// Interactive rectangles of the floating bar, in logical px relative to its window.
pub static BAR_HIT: Lazy<Mutex<Vec<[f64; 4]>>> = Lazy::new(|| Mutex::new(vec![]));

const BAR_W: f64 = 420.0;
const BAR_H: f64 = 150.0;

pub const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Auto-detect"), ("en", "English"), ("es", "Spanish"), ("fr", "French"), ("de", "German"),
    ("it", "Italian"), ("pt", "Portuguese"), ("nl", "Dutch"), ("pl", "Polish"), ("ru", "Russian"),
    ("uk", "Ukrainian"), ("tr", "Turkish"), ("ar", "Arabic"), ("hi", "Hindi"), ("ja", "Japanese"),
    ("ko", "Korean"), ("zh", "Chinese"), ("vi", "Vietnamese"), ("id", "Indonesian"), ("sv", "Swedish"),
];

pub fn show_hub(app: &AppHandle, page: Option<&str>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(p) = page {
            let _ = app.emit("navigate", p.to_string());
        }
    }
}

fn create_hub(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Toolbay Flow")
        .inner_size(1180.0, 780.0)
        .min_inner_size(900.0, 620.0)
        .center()
        .visible(visible)
        .build()?;
    let w2 = w.clone();
    w.on_window_event(move |ev| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
            // Closing the hub keeps Flow running in the tray.
            api.prevent_close();
            let _ = w2.hide();
        }
    });
    Ok(())
}

fn create_bar(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, "bar", WebviewUrl::App("index.html#bar".into()))
        .title("Toolbay Flow Bar")
        .inner_size(BAR_W, BAR_H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .visible_on_all_workspaces(true)
        .build()?;
    let _ = w.set_ignore_cursor_events(true);
    place_bar(app);
    if visible {
        let _ = w.show();
    }
    Ok(())
}

/// Bottom-centre of the work area of the monitor under the cursor.
fn place_bar(app: &AppHandle) -> Option<()> {
    let w = app.get_webview_window("bar")?;
    let cur = app.cursor_position().ok()?;
    let mon = app.monitor_from_point(cur.x, cur.y).ok().flatten().or_else(|| app.primary_monitor().ok().flatten())?;
    let wa = mon.work_area();
    let scale = mon.scale_factor();
    let size = PhysicalSize::new((BAR_W * scale) as u32, (BAR_H * scale) as u32);
    let x = wa.position.x + (wa.size.width as i32 - size.width as i32) / 2;
    let y = wa.position.y + wa.size.height as i32 - size.height as i32;
    let pos = PhysicalPosition::new(x, y);
    if w.outer_position().ok() != Some(pos) || w.outer_size().ok() != Some(size) {
        let _ = w.set_size(size);
        let _ = w.set_position(pos);
    }
    Some(())
}

/// Keeps the bar on the active monitor and click-through outside the pill.
fn bar_loop(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ignoring = true;
        let mut tick = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(60));
            tick += 1;
            let Some(w) = app.get_webview_window("bar") else { continue };
            if tick % 7 == 0 {
                place_bar(&app);
            }
            let (Ok(cur), Ok(pos), Ok(scale)) = (app.cursor_position(), w.outer_position(), w.scale_factor()) else { continue };
            let lx = (cur.x - pos.x as f64) / scale;
            let ly = (cur.y - pos.y as f64) / scale;
            let inside = BAR_HIT.lock().iter().any(|r| lx >= r[0] && ly >= r[1] && lx <= r[0] + r[2] && ly <= r[1] + r[3]);
            if inside == ignoring {
                ignoring = !inside;
                let _ = w.set_ignore_cursor_events(ignoring);
            }
        }
    });
}

fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let st = app.state::<St>();
    let s = st.settings.read().clone();
    let open = MenuItem::with_id(app, "open", "Open Toolbay Flow", true, None::<&str>)?;
    let paste_last = MenuItem::with_id(app, "paste_last", "Paste last dictation", true, None::<&str>)?;
    let mic = Submenu::with_id(app, "mic", "Microphone", true)?;
    mic.append(&CheckMenuItem::with_id(app, "mic:", "System default", true, s.microphone.is_none(), None::<&str>)?)?;
    for d in audio::list_devices() {
        let checked = s.microphone.as_deref() == Some(d.as_str());
        mic.append(&CheckMenuItem::with_id(app, format!("mic:{d}"), &d, true, checked, None::<&str>)?)?;
    }
    let lang = Submenu::with_id(app, "lang", "Language", true)?;
    for (code, name) in LANGUAGES {
        lang.append(&CheckMenuItem::with_id(app, format!("lang:{code}"), *name, true, s.language == *code, None::<&str>)?)?;
    }
    let shortcuts = MenuItem::with_id(app, "shortcuts", "Shortcuts…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let bar = CheckMenuItem::with_id(app, "toggle_bar", "Show Flow bar", true, s.show_bar, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Toolbay Flow", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &open,
            &paste_last,
            &PredefinedMenuItem::separator(app)?,
            &mic,
            &lang,
            &bar,
            &shortcuts,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

pub fn refresh_tray(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("flow"), tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    let st = app.state::<St>().inner().clone();
    match id {
        "open" => show_hub(app, None),
        "settings" => show_hub(app, Some("settings")),
        "shortcuts" => show_hub(app, Some("shortcuts")),
        "quit" => app.exit(0),
        "paste_last" => {
            if let Ok(Some(t)) = st.db.last_text() {
                let restore = st.settings.read().restore_clipboard;
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(250));
                    let _ = paste::paste(&t, restore);
                });
            }
        }
        "toggle_bar" => {
            let mut s = st.settings.read().clone();
            s.show_bar = !s.show_bar;
            let _ = dictation::apply_settings(&st, s);
            let _ = app.emit("settings-changed", ());
            refresh_tray(app);
        }
        other => {
            let mut s = st.settings.read().clone();
            if let Some(m) = other.strip_prefix("mic:") {
                s.microphone = if m.is_empty() { None } else { Some(m.to_string()) };
            } else if let Some(l) = other.strip_prefix("lang:") {
                s.language = l.to_string();
            } else {
                return;
            }
            let _ = dictation::apply_settings(&st, s);
            let _ = app.emit("settings-changed", ());
            refresh_tray(app);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_hub(app, None);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let st = dictation::State::new(handle.clone(), config_dir, data_dir)?;
            app.manage(st.clone());

            let s = st.settings.read().clone();
            let hidden = std::env::args().any(|a| a == "--hidden") && s.onboarded;
            create_hub(&handle, !hidden)?;
            create_bar(&handle, s.show_bar && s.onboarded)?;
            bar_loop(handle.clone());

            let menu = tray_menu(&handle)?;
            TrayIconBuilder::with_id("flow")
                .icon(app.default_window_icon().cloned().unwrap())
                .tooltip("Toolbay Flow")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| on_menu(app, ev.id().as_ref()))
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        show_hub(tray.app_handle(), None);
                    }
                })
                .build(app)?;

            dictation::start_hotkeys(&st);
            if s.onboarded {
                dictation::prewarm(&st);
            }
            // Daily retention pass.
            let st2 = st.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(3600));
                let p = st2.settings.read().data_policy.clone();
                let _ = st2.db.apply_retention(&p);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::default_shortcuts,
            commands::list_models,
            commands::hardware,
            commands::download_model,
            commands::download_engines,
            commands::delete_model,
            commands::start_engines,
            commands::list_mics,
            commands::mic_test,
            commands::history_list,
            commands::history_delete,
            commands::history_clear,
            commands::history_retry,
            commands::copy_text,
            commands::dict_list,
            commands::dict_save,
            commands::dict_delete,
            commands::undo_learned,
            commands::notes_list,
            commands::note_save,
            commands::note_delete,
            commands::stats,
            commands::record_shortcut,
            commands::preview_cleanup,
            commands::bar_action,
            commands::bar_hit,
            commands::open_hub,
            commands::hotkeys_enabled,
            commands::app_info,
            commands::open_data_dir,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Toolbay Flow")
        .run(|_app, ev| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = ev {
                // Keep running in the tray when the last window hides; real quits pass a code.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
