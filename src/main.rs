use lang_changer::cli::{parse, Command, USAGE};
use lang_changer::config::{Config, LeaveAlone};
use lang_changer::detector::Detector;
use lang_changer::engine::Engine;
use lang_changer::{launch_agent, menubar, tap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn run() {
    let dir = home().join(".config").join("lang-changer");
    let config = Config::load(&dir);
    let mut engine = Engine::new(Detector::new(config.threshold), LeaveAlone::load(&dir));
    engine.set_enabled(config.enabled);
    let engine = Arc::new(Mutex::new(engine));

    menubar::init_app();
    let _tray = match menubar::install(engine.clone(), config.enabled) {
        Ok(tray) => Some(tray),
        Err(e) => {
            eprintln!("lang-changer: menu bar item unavailable: {e}");
            None
        }
    };
    if !tap::request_permissions() {
        eprintln!("lang-changer needs Accessibility and Input Monitoring (System Settings → Privacy & Security).");
        eprintln!("Grant both, then start it again.");
    }
    if let Err(e) = tap::run(engine, config.hotkey) {
        eprintln!("lang-changer: {e}");
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Command::Run => run(),
        Command::Help => print!("{USAGE}"),
        Command::Unknown(arg) => {
            eprintln!("lang-changer: unknown argument '{arg}'\n\n{USAGE}");
            std::process::exit(2);
        }
        Command::InstallAgent => match launch_agent::install(&home()) {
            Ok(path) => println!("installed {}; grant Accessibility and Input Monitoring to the binary", path.display()),
            Err(e) => {
                eprintln!("lang-changer: {e}");
                std::process::exit(1);
            }
        },
        Command::UninstallAgent => match launch_agent::uninstall(&home()) {
            Ok(()) => println!("removed the LaunchAgent"),
            Err(e) => {
                eprintln!("lang-changer: {e}");
                std::process::exit(1);
            }
        },
    }
}
