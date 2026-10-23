use lang_changer::config::{Config, LeaveAlone};
use lang_changer::detector::Detector;
use lang_changer::engine::Engine;
use lang_changer::tap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn config_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".config").join("lang-changer")
}

fn main() {
    let dir = config_dir();
    let config = Config::load(&dir);
    let mut engine = Engine::new(Detector::new(config.threshold), LeaveAlone::load(&dir));
    engine.set_enabled(config.enabled);
    let engine = Arc::new(Mutex::new(engine));

    if !tap::request_permissions() {
        eprintln!("lang-changer needs Accessibility and Input Monitoring (System Settings → Privacy & Security).");
        eprintln!("Grant both, then start it again.");
    }
    if let Err(e) = tap::run(engine, config.hotkey) {
        eprintln!("lang-changer: {e}");
        std::process::exit(1);
    }
}
