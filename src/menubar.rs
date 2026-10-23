use crate::engine::Engine;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

/// Makes this process a menu-bar-only app (no Dock icon). Must run on the main thread.
pub fn init_app() {
    let mtm = MainThreadMarker::new().expect("menu bar must be set up on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
}

/// Adds the status item with an "Auto-fix enabled" toggle and Quit. Keep the returned icon alive.
pub fn install(engine: Arc<Mutex<Engine>>, enabled: bool) -> Result<TrayIcon, String> {
    let menu = Menu::new();
    let toggle = CheckMenuItem::new("Auto-fix enabled", true, enabled, None);
    let quit = MenuItem::new("Quit lang-changer", true, None);
    menu.append_items(&[&toggle, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;

    let toggle_id = toggle.id().clone();
    let quit_id = quit.id().clone();
    let state = AtomicBool::new(enabled);
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == toggle_id {
            let now_enabled = !state.fetch_xor(true, Ordering::SeqCst);
            engine.lock().unwrap().set_enabled(now_enabled);
        } else if event.id == quit_id {
            std::process::exit(0);
        }
    }));

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_title("Ru⇄En")
        .with_tooltip("lang-changer")
        .build()
        .map_err(|e| e.to_string())
}
